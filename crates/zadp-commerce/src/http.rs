use crate::{now, observer::Observer, Config, OrderRequest, Store};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::{Arc, Mutex};
use tower_http::{cors::CorsLayer, set_header::SetResponseHeaderLayer};
#[derive(Clone)]
struct Service {
    store: Arc<Mutex<Store>>,
    observer: Arc<Observer>,
    origin: String,
    endpoint_proof: Option<zadp_protocol::EndpointProof>,
    capability: serde_json::Value,
}
fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({"error":message}))).into_response()
}
fn allowed(headers: &HeaderMap, state: &Service) -> bool {
    headers
        .get(header::ORIGIN)
        .is_none_or(|v| v.to_str().ok() == Some(state.origin.as_str()))
}
fn claim(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
}
pub fn router(config: Config) -> anyhow::Result<Router> {
    router_with_endpoint_proof(config, None)
}
pub fn router_with_endpoint_proof(
    config: Config,
    endpoint_proof: Option<zadp_protocol::EndpointProof>,
) -> anyhow::Result<Router> {
    protect(router_unthrottled(config, endpoint_proof)?)
}
/// For catalog adapters: protect the OUTER router once, across all Offers.
pub fn router_unthrottled(
    config: Config,
    endpoint_proof: Option<zadp_protocol::EndpointProof>,
) -> anyhow::Result<Router> {
    if let Some(proof) = &endpoint_proof {
        let endpoint = &config
            .offer
            .commerce
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing commerce"))?
            .endpoint;
        proof.verify(&config.offer.seller, endpoint, chrono::Utc::now())?;
    }
    let state = Service {
        store: Arc::new(Mutex::new(Store::open(config.clone())?)),
        observer: Arc::new(Observer::new(config.clone())?),
        origin: config.browser_origin.clone(),
        capability: crate::capability::describe(&config.offer.capability, &config.offer.id),
        endpoint_proof,
    };
    let cors = CorsLayer::new()
        .allow_origin(config.browser_origin.parse::<HeaderValue>()?)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::HeaderName::from_static("idempotency-key"),
        ]);
    Ok(Router::new()
        .route("/.well-known/zadp", get(endpoint_proof_document))
        .route("/healthz", get(|| async { "ok" }))
        .route("/capabilities", get(service_capability))
        .route("/orders", post(create))
        .route("/orders/{id}", get(check).delete(cancel))
        .with_state(state)
        .layer(DefaultBodyLimit::max(8192))
        .layer(cors)
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        )))
}
async fn create(
    State(state): State<Service>,
    headers: HeaderMap,
    Json(request): Json<OrderRequest>,
) -> Response {
    if !allowed(&headers, &state) {
        return error(StatusCode::FORBIDDEN, "browser origin not allowed");
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match state.store.lock().unwrap().create(request, key, now()) {
        Ok(response) => Json(response).into_response(),
        Err(e) => {
            if e.to_string() == "offer_closed" {
                return error(StatusCode::GONE, "offer_closed");
            }
            if matches!(
                e.to_string().as_str(),
                "offer_lifecycle_unavailable" | "seller_busy"
            ) {
                return error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "offer_lifecycle_unavailable",
                );
            }
            let code = if e.to_string() == "idempotency conflict" {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            error(
                code,
                "Order unavailable, request invalid or idempotency conflict",
            )
        }
    }
}
async fn check(
    State(state): State<Service>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !allowed(&headers, &state) {
        return error(StatusCode::FORBIDDEN, "browser origin not allowed");
    }
    // Authenticate before expensive RPC; never reveal an unknown order.
    if state
        .store
        .lock()
        .unwrap()
        .authenticated(&id, claim(&headers))
        .is_err()
    {
        return error(
            StatusCode::UNAUTHORIZED,
            "Unknown order or invalid claim token",
        );
    }
    let snapshot = state.observer.snapshot().await;
    match state
        .store
        .lock()
        .unwrap()
        .check(&id, claim(&headers), &snapshot, now())
    {
        Ok(response) => Json(response).into_response(),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "Order check unavailable"),
    }
}
async fn cancel(
    State(state): State<Service>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !allowed(&headers, &state) {
        return error(StatusCode::FORBIDDEN, "browser origin not allowed");
    }
    let mut store = state.store.lock().unwrap();
    if store.authenticated(&id, claim(&headers)).is_err() {
        return error(
            StatusCode::UNAUTHORIZED,
            "Unknown order or invalid claim token",
        );
    }
    match store.cancel(&id, claim(&headers)) {
        Ok(response) => Json(response).into_response(),
        Err(_) => error(StatusCode::CONFLICT, "Order cannot be cancelled"),
    }
}

async fn endpoint_proof_document(State(state): State<Service>) -> Response {
    match &state.endpoint_proof {
        Some(proof) if chrono::Utc::now() < proof.expires_at => Json(proof).into_response(),
        _ => error(StatusCode::NOT_FOUND, "Endpoint proof unavailable"),
    }
}

async fn service_capability(State(state): State<Service>) -> Json<serde_json::Value> {
    let mut descriptor = state.capability;
    descriptor["orders_available"] = state
        .store
        .lock()
        .unwrap()
        .config
        .ensure_offer_open(now())
        .is_ok()
        .into();
    Json(descriptor)
}

/// Shared HTTP protection, before body parsing and catalog serialization.
/// Uses the same trusted-peer IP boundary as discovery, never raw X-Forwarded-For.
pub fn protect(router: Router) -> anyhow::Result<Router> {
    use zadp_api::throttle::{DiscoveryThrottle, ThrottleDefaults};
    let create = DiscoveryThrottle::from_env_with_defaults(
        "ZADP_SELLER_CREATE",
        ThrottleDefaults {
            rate_per_second: 5,
            burst: 20,
            client_rate: 1,
            client_burst: 5,
            client_period_seconds: 20,
        },
    )
    .map_err(anyhow::Error::msg)?;
    let read = DiscoveryThrottle::from_env_with_defaults(
        "ZADP_SELLER_HTTP",
        ThrottleDefaults {
            rate_per_second: 50,
            burst: 100,
            client_rate: 5,
            client_burst: 30,
            client_period_seconds: 1,
        },
    )
    .map_err(anyhow::Error::msg)?;
    let create_layer = axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let budget = create.clone();
            async move {
                if request.method() == Method::POST && request.uri().path() == "/orders" {
                    // Run the existing trusted-client middleware on this request only.
                    use tower::ServiceExt;
                    budget
                        .apply(
                            Router::new()
                                .fallback(move |request| async move { next.run(request).await }),
                        )
                        .oneshot(request)
                        .await
                        .unwrap()
                } else {
                    next.run(request).await
                }
            }
        },
    );
    Ok(read.apply(router.layer(create_layer)))
}

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
    let state = Service {
        store: Arc::new(Mutex::new(Store::open(config.clone())?)),
        observer: Arc::new(Observer::new(config.clone())?),
        origin: config.browser_origin.clone(),
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
        .route("/healthz", get(|| async { "ok" }))
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

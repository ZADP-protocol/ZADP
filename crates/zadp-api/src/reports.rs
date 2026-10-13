//! Public abuse intake and separately bound, authenticated operator interface.
use crate::{boards::HostedState, ApiError};
use axum::{
    extract::Request,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use zadp_indexer::{BoardRegistry, Visibility};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Submission {
    board_id: String,
    offer_id: String,
    seller: String,
    reason: String,
    #[serde(default)]
    explanation: String,
    idempotency_key: String,
}
pub(crate) async fn submit(
    State(s): State<HostedState>,
    Json(r): Json<Submission>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !zadp_indexer::moderation::validate_report(&r.idempotency_key, &r.reason, &r.explanation) {
        return Err(ApiError::bad_request("invalid report"));
    }
    let b = s
        .registry
        .get(&r.board_id)
        .filter(|b| b.status == zadp_indexer::BoardStatus::Active)
        .ok_or_else(ApiError::not_found)?;
    let offer = b
        .repository
        .hosted_offer(&r.seller, &r.offer_id)
        .map_err(|_| ApiError::internal("report lookup failed"))?
        .filter(|o| o.offer.seller.as_str() == r.seller && s.policy.snapshot().permits_record(o))
        .ok_or_else(ApiError::not_found)?;
    let report = b
        .repository
        .submit_report(
            offer.offer.seller.as_str(),
            &r.offer_id,
            &r.idempotency_key,
            &r.reason,
            &r.explanation,
        )
        .map_err(|_| ApiError::bad_request("report rejected or idempotency conflict"))?;
    // Reporter explanations and target content never appear in public receipts.
    Ok(Json(
        serde_json::json!({"report_id":report.report_id,"status":"received"}),
    ))
}
#[derive(Clone)]
struct Admin {
    registry: Arc<BoardRegistry>,
    token: Arc<String>,
}
/// Callers must bind this router ONLY to loopback. Never merge it into public routes.
pub fn moderation_admin_router(
    registry: Arc<BoardRegistry>,
    token: String,
) -> Result<Router, &'static str> {
    if token.len() < 32 || token.len() > 256 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("admin token must be 32-256 printable characters");
    }
    let state = Admin {
        registry,
        token: Arc::new(token),
    };
    Ok(Router::new()
        .route(
            "/",
            get(|| async { axum::response::Html(include_str!("moderation-admin.html")) }),
        )
        .route("/review", get(review))
        .route("/offer", get(related_offer))
        .route("/decisions", post(decide))
        .route("/reports", post(process))
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .layer(middleware::from_fn(admin_headers))
        .with_state(state))
}
async fn authorize(State(s): State<Admin>, request: Request, next: Next) -> Response {
    // The HTML contains no data; all reads and mutations require the explicit bearer token.
    if request.uri().path() == "/" && request.method() == axum::http::Method::GET {
        return next.run(request).await;
    }
    let supplied = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let mut diff = supplied.len() ^ s.token.len();
    for (a, b) in supplied.as_bytes().iter().zip(s.token.as_bytes()) {
        diff |= usize::from(a ^ b);
    }
    if diff != 0 {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"unauthorized"})),
        )
            .into_response();
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
async fn review(State(s): State<Admin>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut boards = Vec::new();
    for b in s.registry.contexts() {
        boards.push(serde_json::json!({"board_id":b.descriptor().board.board_id,"offers":b.repository.moderation_queue().map_err(|_|ApiError::internal("moderation lookup failed"))?,"reports":b.repository.reports().map_err(|_|ApiError::internal("report lookup failed"))?}));
    }
    Ok(Json(serde_json::json!({"boards":boards})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    board_id: String,
    seller: String,
    offer_id: String,
    action: String,
    reason: String,
}
async fn decide(
    State(s): State<Admin>,
    Json(r): Json<Decision>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if r.reason.trim().is_empty() {
        return Err(ApiError::bad_request("decision reason required"));
    }
    let visibility = match r.action.as_str() {
        "approve" | "restore" => Visibility::Visible,
        "hide" => Visibility::Hidden,
        "reject" => Visibility::Quarantined,
        _ => return Err(ApiError::bad_request("invalid action")),
    };
    s.registry
        .get(&r.board_id)
        .ok_or_else(ApiError::not_found)?
        .repository
        .moderate(&r.seller, &r.offer_id, visibility, &r.reason)
        .map_err(|_| ApiError::bad_request("invalid moderation decision"))?;
    Ok(Json(serde_json::json!({"status":"saved"})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Processing {
    board_id: String,
    report_id: String,
    status: String,
    reason: String,
}
async fn process(
    State(s): State<Admin>,
    Json(r): Json<Processing>,
) -> Result<Json<serde_json::Value>, ApiError> {
    s.registry
        .get(&r.board_id)
        .ok_or_else(ApiError::not_found)?
        .repository
        .process_report(&r.report_id, &r.status, &r.reason)
        .map_err(|_| ApiError::bad_request("invalid report decision"))?;
    Ok(Json(serde_json::json!({"status":"saved"})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    board_id: String,
    seller: String,
    offer_id: String,
}
async fn related_offer(
    State(s): State<Admin>,
    axum::extract::Query(r): axum::extract::Query<Target>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let b = s
        .registry
        .get(&r.board_id)
        .ok_or_else(ApiError::not_found)?;
    let offer = b
        .repository
        .historical_offer(&r.seller, &r.offer_id)
        .map_err(|_| ApiError::internal("historical lookup failed"))?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(
        serde_json::json!({"offer":offer,"lifecycle":b.repository.lifecycle(&r.seller,&r.offer_id,chrono::Utc::now()).map_err(|_|ApiError::internal("lifecycle lookup failed"))?}),
    ))
}

async fn admin_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    for (name,value) in [("cache-control","no-store"),("x-frame-options","DENY"),("x-content-type-options","nosniff"),("referrer-policy","no-referrer"),("content-security-policy","default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")] {response.headers_mut().insert(axum::http::HeaderName::from_static(name),axum::http::HeaderValue::from_static(value));}
    response
}

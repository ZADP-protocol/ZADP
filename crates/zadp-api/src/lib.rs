//! Thin HTTP adapters over indexer and seller domain services.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use zadp_indexer::{ChainObservation, Indexer, OfferRepository};
use zadp_protocol::Offer;
use zadp_seller::{CreateInvoiceRequest, Invoice, SellerService, ServiceResult};
use zadp_zcash::ZcashAdapter;

pub struct AppState<A, R>
where
    A: ZcashAdapter + 'static,
    R: OfferRepository + 'static,
{
    pub indexer: Arc<Indexer<A, R>>,
    pub seller: Arc<SellerService<A>>,
}

impl<A, R> Clone for AppState<A, R>
where
    A: ZcashAdapter + 'static,
    R: OfferRepository + 'static,
{
    fn clone(&self) -> Self {
        Self {
            indexer: Arc::clone(&self.indexer),
            seller: Arc::clone(&self.seller),
        }
    }
}

pub fn router<A, R>(state: AppState<A, R>) -> Router
where
    A: ZcashAdapter + 'static,
    R: OfferRepository + 'static,
{
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/offers", get(list_offers::<A, R>))
        .route("/v1/invoices", post(create_invoice::<A, R>))
        .route("/v1/fulfill", post(fulfill::<A, R>))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                // Deliberately omit query strings and bodies from request spans.
                tracing::info_span!("http_request", method = %request.method())
            },
        ))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OffersQuery {
    pub capability: Option<String>,
    pub board_id: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OffersResponse {
    pub offers: Vec<IndexedOffer>,
}

#[derive(Debug, Serialize)]
pub struct IndexedOffer {
    #[serde(flatten)]
    pub offer: Offer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<ChainObservation>,
    pub commerce_verification: CommerceVerification,
}

/// Read-only API for real board discovery. No invoice, fulfillment or wallet routes.
pub struct BoardApiState<R> {
    repository: Arc<R>,
    policy: HostPolicy,
    endpoints: EndpointVerifier,
}
impl<R> Clone for BoardApiState<R> {
    fn clone(&self) -> Self {
        Self {
            repository: Arc::clone(&self.repository),
            policy: self.policy.clone(),
            endpoints: self.endpoints.clone(),
        }
    }
}
pub fn board_router<R: OfferRepository + 'static>(repository: Arc<R>) -> Router {
    board_router_with_policy(repository, HostPolicy::default())
}
pub fn board_router_with_policy<R: OfferRepository + 'static>(
    repository: Arc<R>,
    policy: HostPolicy,
) -> Router {
    board_router_with_verifier(repository, policy, EndpointVerifier::default())
}
/// Dependency injection for offline verification tests; no HTTP endpoint accepts a URL.
pub fn board_router_with_verifier<R: OfferRepository + 'static>(
    repository: Arc<R>,
    policy: HostPolicy,
    endpoints: EndpointVerifier,
) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/offers", get(board_offers::<R>))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                // Deliberately omit query strings and bodies from request spans.
                tracing::info_span!("http_request", method = %request.method())
            },
        ))
        .with_state(BoardApiState {
            repository,
            policy,
            endpoints,
        })
}

async fn board_offers<R: OfferRepository + 'static>(
    State(state): State<BoardApiState<R>>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError> {
    let records = state
        .repository
        .list(query.capability.as_deref())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let policy = state.policy.snapshot();
    Ok(Json(OffersResponse {
        offers: records
            .into_iter()
            .filter(|r| policy.permits_record(r))
            .map(|r| IndexedOffer {
                commerce_verification: state.endpoints.status(&r.offer),
                offer: r.offer,
                chain: r.chain,
            })
            .collect(),
    }))
}

async fn list_offers<A, R>(
    State(state): State<AppState<A, R>>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError>
where
    A: ZcashAdapter + 'static,
    R: OfferRepository + 'static,
{
    let records = state
        .indexer
        .repository()
        .list(query.capability.as_deref())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(OffersResponse {
        offers: records
            .into_iter()
            .map(|r| IndexedOffer {
                commerce_verification: CommerceVerification {
                    status: if r.offer.commerce.is_none() {
                        CommerceStatus::DiscoveryOnly
                    } else {
                        CommerceStatus::Unverified
                    },
                    origin: None,
                    expires_at: None,
                    valid_until: None,
                },
                offer: r.offer,
                chain: r.chain,
            })
            .collect(),
    }))
}

async fn create_invoice<A, R>(
    State(state): State<AppState<A, R>>,
    Json(req): Json<CreateInvoiceRequest>,
) -> Result<Json<Invoice>, ApiError>
where
    A: ZcashAdapter + 'static,
    R: OfferRepository + 'static,
{
    let invoice = state
        .seller
        .create_invoice(req)
        .await
        .map_err(ApiError::from_seller)?;
    Ok(Json(invoice))
}

#[derive(Debug, Deserialize)]
pub struct FulfillRequest {
    pub invoice_id: String,
    pub prompt: String,
}

async fn fulfill<A, R>(
    State(state): State<AppState<A, R>>,
    Json(req): Json<FulfillRequest>,
) -> Result<Json<ServiceResult>, ApiError>
where
    A: ZcashAdapter + 'static,
    R: OfferRepository + 'static,
{
    state
        .seller
        .observe_and_mark_paid(&req.invoice_id)
        .await
        .map_err(ApiError::from_seller)?;
    let result = state
        .seller
        .fulfill_mock_inference(&req.invoice_id, &req.prompt)
        .await
        .map_err(ApiError::from_seller)?;
    Ok(Json(result))
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: "Unknown board".into(),
        }
    }
    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        tracing::warn!(error = %message.into(), "repository request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Internal server error".into(),
        }
    }

    fn from_seller(err: zadp_seller::SellerError) -> Self {
        let status = match &err {
            zadp_seller::SellerError::UnknownOffer(_)
            | zadp_seller::SellerError::UnknownInvoice(_) => StatusCode::NOT_FOUND,
            zadp_seller::SellerError::OfferExpired | zadp_seller::SellerError::Unpaid(_) => {
                StatusCode::CONFLICT
            }
            _ => StatusCode::BAD_REQUEST,
        };
        Self {
            status,
            message: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({"error": self.message})),
        )
            .into_response()
    }
}

mod boards;
pub use boards::{
    multi_board_router, multi_board_router_with_metadata, multi_board_router_with_policy,
    multi_board_router_with_versions, BoardPresentation,
};
mod host_policy;
pub use host_policy::{
    HostPolicy, ManualPolicy, OfferDecision, OfferRule, SellerRule, SellerStatus,
};

mod endpoint_verification;
pub use endpoint_verification::{
    CommerceStatus, CommerceVerification, EndpointVerifier, ProofFetcher,
};

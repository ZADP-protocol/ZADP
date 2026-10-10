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
    pub has_endpoint: bool,
    pub has_fixed_price: bool,
    pub purchase_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<ChainObservation>,
    pub endpoint_verification: EndpointVerification,
}

impl IndexedOffer {
    // Host-derived metadata, never part of the signed Offer. A proof establishes
    // origin control only; the reference buyer also needs a supported capability.
    pub(crate) fn new(
        record: zadp_indexer::OfferRecord,
        verification: EndpointVerification,
    ) -> Self {
        let has_endpoint = record.offer.commerce.is_some();
        let has_fixed_price = record.offer.price.is_some();
        let purchase_available = has_endpoint
            && has_fixed_price
            && verification.status == EndpointStatus::Verified
            && verification
                .expires_at
                .is_some_and(|t| t > chrono::Utc::now())
            && verification
                .valid_until
                .is_some_and(|t| t > chrono::Utc::now())
            && record
                .chain
                .as_ref()
                .is_some_and(|c| c.offer_status == "active")
            && matches!(
                record.offer.capability.as_str(),
                "text.uppercase" | "text.summarize"
            )
            && zadp_protocol::verify_offer(&record.offer, chrono::Utc::now()).is_ok();
        Self {
            offer: record.offer,
            chain: record.chain,
            endpoint_verification: verification,
            has_endpoint,
            has_fixed_price,
            purchase_available,
        }
    }
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
            .map(|r| {
                let verification = state.endpoints.status(&r.offer);
                IndexedOffer::new(r, verification)
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
            .map(|r| {
                let verification = EndpointVerification {
                    status: if r.offer.commerce.is_none() {
                        EndpointStatus::NotApplicable
                    } else {
                        EndpointStatus::Unverified
                    },
                    origin: None,
                    expires_at: None,
                    valid_until: None,
                };
                IndexedOffer::new(r, verification)
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
    EndpointStatus, EndpointVerification, EndpointVerifier, ProofFetcher,
};

#[cfg(test)]
mod endpoint_metadata_tests {
    use super::*;
    use chrono::{Duration, Utc};
    use zadp_protocol::{OfferDraft, Price, SellerKeyPair, ZcashNetwork};

    fn record(price: bool, endpoint: bool) -> zadp_indexer::OfferRecord {
        let keys = SellerKeyPair::from_secret_bytes(&[42; 32]);
        let mut draft = OfferDraft::new_discovery(
            keys.id.clone(),
            "text.uppercase",
            "Service",
            Utc::now() + Duration::hours(1),
            "nonce",
        );
        if endpoint {
            draft = draft.with_endpoint("https://seller.example");
        }
        if price {
            draft = draft.with_price(Price::zec("0.01".parse().unwrap()));
        }
        zadp_indexer::OfferRecord {
            offer: draft.sign(&keys).unwrap(),
            source_txid: zadp_zcash::TxId("fixture".into()),
            indexed_at: Utc::now(),
            chain: Some(ChainObservation {
                network: ZcashNetwork::Mainnet,
                board_id: "hello".into(),
                block_height: 1,
                block_hash: "00".repeat(32),
                transaction_id: "ab".repeat(32),
                pool: "orchard".into(),
                output_index: 0,
                listing_value_zat: 1000,
                scanned_height: 4,
                confirmations: 4,
                confirmation_status: "confirmed".into(),
                offer_status: "active".into(),
            }),
        }
    }
    fn proof(status: EndpointStatus) -> EndpointVerification {
        EndpointVerification {
            status,
            origin: Some("https://seller.example".into()),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            valid_until: Some(Utc::now() + Duration::seconds(60)),
        }
    }
    #[test]
    fn origin_proof_alone_does_not_enable_payment_and_all_buy_gates_apply() {
        let service = IndexedOffer::new(record(false, true), proof(EndpointStatus::Verified));
        assert!(service.has_endpoint);
        assert!(!service.has_fixed_price);
        assert!(!service.purchase_available);
        assert!(
            !IndexedOffer::new(record(false, false), proof(EndpointStatus::NotApplicable))
                .purchase_available
        );
        assert!(
            IndexedOffer::new(record(true, true), proof(EndpointStatus::Verified))
                .purchase_available
        );
        for status in [
            EndpointStatus::Unverified,
            EndpointStatus::Unavailable,
            EndpointStatus::NotApplicable,
        ] {
            assert!(!IndexedOffer::new(record(true, true), proof(status)).purchase_available);
        }
        for expired_field in 0..2 {
            let mut stale = proof(EndpointStatus::Verified);
            if expired_field == 0 {
                stale.expires_at = Some(Utc::now() - Duration::seconds(1));
            } else {
                stale.valid_until = Some(Utc::now() - Duration::seconds(1));
            }
            assert!(!IndexedOffer::new(record(true, true), stale).purchase_available);
        }
        for change in 0..5 {
            let mut r = record(true, true);
            match change {
                0 => r.chain = None,
                1 => r.chain.as_mut().unwrap().offer_status = "expired".into(),
                2 => r.offer.expires_at = Utc::now() - Duration::seconds(1),
                3 => r.offer.title.push_str("tampered"),
                _ => r.offer.capability = "unsupported".into(),
            }
            assert!(!IndexedOffer::new(r, proof(EndpointStatus::Verified)).purchase_available);
        }
    }
}

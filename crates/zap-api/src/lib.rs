//! Thin HTTP adapters over indexer and seller domain services.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use zap_indexer::{Indexer, OfferRepository};
use zap_protocol::Offer;
use zap_seller::{CreateInvoiceRequest, Invoice, SellerService, ServiceResult};
use zap_zcash::ZcashAdapter;

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
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct OffersQuery {
    pub capability: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OffersResponse {
    pub offers: Vec<Offer>,
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
        offers: records.into_iter().map(|r| r.offer).collect(),
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
    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn from_seller(err: zap_seller::SellerError) -> Self {
        let status = match &err {
            zap_seller::SellerError::UnknownOffer(_)
            | zap_seller::SellerError::UnknownInvoice(_) => StatusCode::NOT_FOUND,
            zap_seller::SellerError::OfferExpired | zap_seller::SellerError::Unpaid(_) => {
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

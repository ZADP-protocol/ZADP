//! Public hosted views. No registration, moderation, descriptor or spending HTTP routes.
use crate::{ApiError, IndexedOffer, OffersQuery, OffersResponse};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use zadp_indexer::{BoardRegistry, BoardStatus, BoardSummary, OfferRepository};

pub fn multi_board_router(registry: Arc<BoardRegistry>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/boards", get(boards))
        .route("/boards/{board_id}", get(board))
        .route("/boards/{board_id}/offers", get(board_offers))
        .route("/offers", get(offers))
        .with_state(registry)
}
async fn boards(
    State(registry): State<Arc<BoardRegistry>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let summaries = registry
        .contexts()
        .map(|b| b.summary())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"boards":summaries})))
}
async fn board(
    State(registry): State<Arc<BoardRegistry>>,
    Path(id): Path<String>,
) -> Result<Json<BoardSummary>, ApiError> {
    let context = registry.get(&id).ok_or_else(ApiError::not_found)?;
    Ok(Json(
        context
            .summary()
            .map_err(|e| ApiError::internal(e.to_string()))?,
    ))
}
async fn board_offers(
    State(registry): State<Arc<BoardRegistry>>,
    Path(id): Path<String>,
    Query(mut query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError> {
    if query.board_id.as_ref().is_some_and(|b| b != &id) {
        return Err(ApiError::bad_request("conflicting board filters"));
    }
    query.board_id = Some(id);
    list(&registry, query).await
}
async fn offers(
    State(registry): State<Arc<BoardRegistry>>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError> {
    list(&registry, query).await
}
async fn list(
    registry: &BoardRegistry,
    query: OffersQuery,
) -> Result<Json<OffersResponse>, ApiError> {
    if query.q.as_ref().is_some_and(|q| q.len() > 256) {
        return Err(ApiError::bad_request("search query too long"));
    }
    if query
        .board_id
        .as_ref()
        .is_some_and(|id| registry.get(id).is_none())
    {
        return Err(ApiError::not_found());
    }
    let q = query.q.map(|q| q.to_lowercase());
    let mut offers = Vec::new();
    for board in registry.contexts() {
        if board.status != BoardStatus::Active
            || query
                .board_id
                .as_ref()
                .is_some_and(|id| id != &board.descriptor().board.board_id)
        {
            continue;
        }
        let records = board
            .repository
            .list(query.capability.as_deref())
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        for r in records {
            if q.as_ref().is_some_and(|q| {
                !format!(
                    "{} {} {}",
                    r.offer.title,
                    r.offer.capability,
                    r.offer.description.as_deref().unwrap_or("")
                )
                .to_lowercase()
                .contains(q)
            }) {
                continue;
            }
            offers.push(IndexedOffer {
                offer: r.offer,
                chain: r.chain,
            });
            if offers.len() >= 1000 {
                return Ok(Json(OffersResponse { offers }));
            }
        }
    }
    Ok(Json(OffersResponse { offers }))
}

//! Public hosted views. No registration, moderation, descriptor or spending HTTP routes.
use crate::{ApiError, HostPolicy, IndexedOffer, OffersQuery, OffersResponse};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use zadp_indexer::{BoardRegistry, BoardStatus, BoardSummary, OfferRepository};

#[derive(Clone)]
struct HostedState {
    registry: Arc<BoardRegistry>,
    policy: HostPolicy,
    versions: Option<(String, String)>,
}
pub fn multi_board_router(registry: Arc<BoardRegistry>) -> Router {
    multi_board_router_with_policy(registry, HostPolicy::default())
}
pub fn multi_board_router_with_policy(registry: Arc<BoardRegistry>, policy: HostPolicy) -> Router {
    multi_board_router_with_versions(registry, policy, None)
}
pub fn multi_board_router_with_versions(
    registry: Arc<BoardRegistry>,
    policy: HostPolicy,
    versions: Option<(String, String)>,
) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/boards", get(boards))
        .route("/boards/{board_id}", get(board))
        .route("/boards/{board_id}/offers", get(board_offers))
        .route("/offers", get(offers))
        .route("/offers/{id}", get(offer))
        .with_state(HostedState {
            registry,
            policy,
            versions,
        })
}
async fn health(State(state): State<HostedState>) -> Json<serde_json::Value> {
    let mut status = serde_json::json!({"status":"ok"});
    if let Some((node, transport)) = state.versions {
        status["node_version"] = node.into();
        status["lightwalletd_version"] = transport.into();
    }
    Json(status)
}
async fn boards(State(state): State<HostedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let summaries = state
        .registry
        .contexts()
        .map(|b| b.summary())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"boards":summaries})))
}
async fn board(
    State(state): State<HostedState>,
    Path(id): Path<String>,
) -> Result<Json<BoardSummary>, ApiError> {
    let context = state.registry.get(&id).ok_or_else(ApiError::not_found)?;
    Ok(Json(
        context
            .summary()
            .map_err(|e| ApiError::internal(e.to_string()))?,
    ))
}
async fn board_offers(
    State(state): State<HostedState>,
    Path(id): Path<String>,
    Query(mut query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError> {
    if query.board_id.as_ref().is_some_and(|b| b != &id) {
        return Err(ApiError::bad_request("conflicting board filters"));
    }
    query.board_id = Some(id);
    list(&state.registry, &state.policy, query).await
}
async fn offers(
    State(state): State<HostedState>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError> {
    list(&state.registry, &state.policy, query).await
}
async fn offer(
    State(state): State<HostedState>,
    Path(id): Path<String>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<IndexedOffer>, ApiError> {
    // Reuse the exact active hosted view; knowing a canonical ID cannot bypass it.
    let Json(view) = list(&state.registry, &state.policy, query).await?;
    let mut matches = view.offers.into_iter().filter(|o| o.offer.id == id);
    let first = matches.next().ok_or_else(ApiError::not_found)?;
    if matches.next().is_some() {
        return Err(ApiError::bad_request("select a board for this offer ID"));
    }
    Ok(Json(first))
}
async fn list(
    registry: &BoardRegistry,
    policy: &HostPolicy,
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
    let policy = policy.snapshot();
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
            if !policy.permits_record(&r) {
                continue;
            }
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

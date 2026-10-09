//! Public hosted views. No registration, moderation, descriptor or spending HTTP routes.
use crate::{ApiError, EndpointVerifier, HostPolicy, IndexedOffer, OffersQuery, OffersResponse};
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
    endpoints: EndpointVerifier,
    versions: Option<(String, String)>,
    metadata: std::collections::BTreeMap<String, BoardPresentation>,
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
    multi_board_router_with_metadata(registry, policy, versions, Default::default())
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardPresentation {
    pub display_name: String,
    pub description: String,
}

pub fn multi_board_router_with_metadata(
    registry: Arc<BoardRegistry>,
    policy: HostPolicy,
    versions: Option<(String, String)>,
    metadata: std::collections::BTreeMap<String, BoardPresentation>,
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
            endpoints: EndpointVerifier::default(),
            policy,
            versions,
            metadata,
        })
}
fn summary(
    state: &HostedState,
    context: &zadp_indexer::BoardContext,
) -> Result<BoardSummary, zadp_indexer::IndexerError> {
    let mut summary = context.summary()?;
    if let Some(metadata) = state.metadata.get(&summary.board_id) {
        summary.display_name = Some(metadata.display_name.clone());
        summary.description = Some(metadata.description.clone());
    }
    Ok(summary)
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
        .map(|b| summary(&state, b))
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
        summary(&state, context).map_err(|e| ApiError::internal(e.to_string()))?,
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
    list(
        &state.registry,
        &state.policy,
        &state.endpoints,
        query,
        None,
    )
    .await
}
async fn offers(
    State(state): State<HostedState>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<OffersResponse>, ApiError> {
    list(
        &state.registry,
        &state.policy,
        &state.endpoints,
        query,
        None,
    )
    .await
}
async fn offer(
    State(state): State<HostedState>,
    Path(id): Path<String>,
    Query(query): Query<OffersQuery>,
) -> Result<Json<IndexedOffer>, ApiError> {
    // Reuse the exact active hosted view; knowing a canonical ID cannot bypass it.
    let Json(view) = list(
        &state.registry,
        &state.policy,
        &state.endpoints,
        query,
        Some(&id),
    )
    .await?;
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
    endpoints: &EndpointVerifier,
    query: OffersQuery,
    only_id: Option<&str>,
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
            // Exact lookup must precede list pagination; hidden IDs still use the same policy.
            if only_id.is_some_and(|id| r.offer.id != id) {
                continue;
            }
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
                commerce_verification: endpoints.status(&r.offer),
                offer: r.offer,
                chain: r.chain,
            });
            if offers.len() >= if only_id.is_some() { 2 } else { 1000 } {
                return Ok(Json(OffersResponse { offers }));
            }
        }
    }
    Ok(Json(OffersResponse { offers }))
}

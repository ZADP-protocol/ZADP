use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tower::ServiceExt;
use zadp_indexer::{InMemoryOfferRepository, IndexerError, OfferRecord, OfferRepository};
use zadp_protocol::{OfferDraft, Price, SellerKeyPair};
use zadp_zcash::TxId;

async fn response<R: OfferRepository + 'static>(
    repo: Arc<R>,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let response = zadp_api::board_router(repo)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn empty_capability_expiry_and_missing_detail_routes() {
    let repo = Arc::new(InMemoryOfferRepository::new());
    assert_eq!(
        response(repo.clone(), "/offers").await.1["offers"],
        serde_json::json!([])
    );
    let keys = SellerKeyPair::generate();
    for (capability, ttl) in [("text.shorten", 3600), ("llm", 3600), ("text.shorten", -1)] {
        let offer = OfferDraft::new_ulid(
            keys.id.clone(),
            capability,
            "Fixture",
            Price::zec("0.01".parse().unwrap()),
            "https://example.com",
            Utc::now() + Duration::seconds(ttl),
            "fixture-nonce",
        )
        .sign(&keys)
        .unwrap();
        repo.upsert_first_wins(OfferRecord {
            offer,
            source_txid: TxId("fixture".into()),
            indexed_at: Utc::now(),
            chain: None,
        })
        .await
        .unwrap();
    }
    assert_eq!(
        response(repo.clone(), "/offers").await.1["offers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        response(repo.clone(), "/offers?capability=text.shorten")
            .await
            .1["offers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        response(repo.clone(), "/offers?capability=unknown").await.1["offers"],
        serde_json::json!([])
    );
    assert_eq!(
        response(repo.clone(), "/offers?capability=a&capability=b")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        response(repo.clone(), "/offers/unknown").await.0,
        StatusCode::NOT_FOUND
    );
    let post = zadp_api::board_router(repo)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/invoices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::NOT_FOUND);
}

struct FailingRepository;
#[async_trait]
impl OfferRepository for FailingRepository {
    async fn upsert_first_wins(&self, _: OfferRecord) -> Result<bool, IndexerError> {
        unreachable!("not used by read-only API")
    }
    async fn list(&self, _: Option<&str>) -> Result<Vec<OfferRecord>, IndexerError> {
        Err(IndexerError::State(
            "/private/operator/database internal detail".into(),
        ))
    }
    async fn get(&self, _: &str) -> Result<Option<OfferRecord>, IndexerError> {
        unreachable!("no detail API")
    }
}
#[tokio::test]
async fn internal_repository_details_never_reach_http_clients() {
    let (status, body) = response(Arc::new(FailingRepository), "/offers").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, serde_json::json!({"error":"Internal server error"}));
}

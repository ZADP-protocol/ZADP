use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rust_decimal::Decimal;
use std::sync::Arc;
use tower::ServiceExt;
use zadp_indexer::*;
use zadp_protocol::*;
use zadp_zcash::*;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};
const BIRTHDAY: u32 = 3_000_000;
fn descriptor(policy: BoardPolicy) -> BoardDescriptor {
    let keys = SellerKeyPair::generate();
    let params = ZcashNetwork::Mainnet.parameters();
    let mut seed = [0u8; 64];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let ivk = UnifiedSpendingKey::from_seed(&params, &seed, AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let ua = ivk
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap();
    BoardBody {
        zadp: 1,
        board_id: "ai-services".into(),
        name: "Test board".into(),
        network: ZcashNetwork::Mainnet,
        address: ua.encode(&params),
        incoming_viewing_key: ivk.encode(&params),
        birthday_height: BIRTHDAY,
        policy,
        operator_key: keys.id.clone(),
    }
    .sign(&keys)
    .unwrap()
}
fn offer(keys: &SellerKeyPair, nonce: u8) -> Offer {
    let now = DateTime::from_timestamp(Utc::now().timestamp() - 100, 0).unwrap();
    let draft = OfferDraft::new_ulid(
        keys.id.clone(),
        "llm.inference",
        "test",
        Price::zec(Decimal::new(1, 2)),
        "https://example.com/zadp",
        now + Duration::hours(24),
        B64.encode([nonce; 16]),
    )
    .sign(keys)
    .unwrap();
    bind_offer(draft, "ai-services", now, keys).unwrap()
}
fn block(height: u32, hash: u32, prev: u32, offers: Vec<Offer>) -> BoardBlock {
    BoardBlock {
        header: BlockHeader {
            height,
            hash: format!("{hash:064x}"),
            prev_hash: format!("{prev:064x}"),
            time: DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap(),
        },
        outputs: offers
            .into_iter()
            .enumerate()
            .map(|(i, o)| BoardOutput {
                txid: TxId(format!("{hash:060x}{i:04x}")),
                transaction_index: i as u64,
                output_index: 0,
                pool: "orchard".into(),
                value_zat: 1000,
                memo: encode_offer_memo(&o).unwrap().to_vec(),
            })
            .collect(),
    }
}
async fn call(
    app: Router,
    path: &str,
    body: Option<serde_json::Value>,
    token: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder()
        .uri(path)
        .method(if body.is_some() { "POST" } else { "GET" });
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if body.is_some() {
        req = req.header("content-type", "application/json");
    }
    let response = app
        .oneshot(
            req.body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1000000).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or_default())
}
#[tokio::test]
async fn public_reports_admin_auth_and_all_discovery_paths() {
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let o = offer(&seller, 1);
    let repo = Arc::new(SqliteOfferRepository::open(":memory:", board).unwrap());
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![o.clone()]))
        .unwrap();
    let mut registry = BoardRegistry::new(ZcashNetwork::Mainnet);
    registry
        .register(
            BoardContext {
                repository: repo.clone(),
                source_type: BoardSourceType::Hosted,
                status: BoardStatus::Active,
            },
            &HostedBoardPolicy::default(),
            true,
        )
        .unwrap();
    let registry = Arc::new(registry);
    let public = zadp_api::multi_board_router(registry.clone());
    let token = "01234567890123456789012345678901";
    let admin = zadp_api::moderation_admin_router(registry, token.into()).unwrap();
    assert_eq!(
        call(admin.clone(), "/review", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(admin.clone(), "/review", None, Some("incorrect"))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(public.clone(), "/review", None, Some(token)).await.0,
        StatusCode::NOT_FOUND
    );
    let report = serde_json::json!({"board_id":"ai-services","seller":seller.id.as_str(),"offer_id":o.id,"reason":"spam","explanation":"Possible duplicates","idempotency_key":"abcdefghijklmnop"});
    for _ in 0..2 {
        let (status, receipt) = call(public.clone(), "/reports", Some(report.clone()), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(receipt["status"], "received");
        assert!(receipt.get("explanation").is_none());
    }
    assert_eq!(repo.reports().unwrap().len(), 1);
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    let decision = serde_json::json!({"board_id":"ai-services","seller":seller.id.as_str(),"offer_id":o.id,"action":"hide","reason":"Concrete risk reviewed"});
    assert_eq!(
        call(admin.clone(), "/decisions", Some(decision.clone()), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    assert_eq!(
        call(
            admin.clone(),
            "/decisions",
            Some(decision.clone()),
            Some(token)
        )
        .await
        .0,
        StatusCode::OK
    );
    for path in [
        "/offers",
        "/boards/ai-services/offers",
        "/offers?board_id=ai-services",
        "/offers?q=test",
    ] {
        let (status, view) = call(public.clone(), path, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["offers"], serde_json::json!([]));
    }
    assert_eq!(
        call(
            public.clone(),
            &format!("/offers/{}?board_id=ai-services", o.id),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(public.clone(), "/reports", Some(report.clone()), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let mut restore = decision;
    restore["action"] = "restore".into();
    assert_eq!(
        call(admin.clone(), "/decisions", Some(restore), Some(token))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(call(admin.clone(),"/reports",Some(serde_json::json!({"board_id":"ai-services","report_id":"abcdefghijklmnop","status":"dismissed","reason":"Reviewed"})),Some(token)).await.0,StatusCode::OK);
    let view = call(admin, "/review", None, Some(token)).await.1;
    assert_eq!(view["boards"][0]["reports"][0]["status"], "dismissed");
    // Dedicated intake throttle remains independent of discovery reads.
    for _ in 0..10 {
        call(public.clone(), "/reports", Some(report.clone()), None).await;
    }
    assert_eq!(
        call(public, "/reports", Some(report), None).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
}
#[tokio::test]
async fn report_target_includes_seller_and_unknown_target_is_not_persisted() {
    let a = SellerKeyPair::generate();
    let b = SellerKeyPair::generate();
    let first = offer(&a, 1);
    let mut second = offer(&b, 2);
    second.id = first.id.clone();
    let issued = second.issued_at.unwrap();
    let second = bind_offer(second, "ai-services", issued, &b).unwrap();
    let repo = Arc::new(
        SqliteOfferRepository::open(":memory:", descriptor(BoardPolicy::default())).unwrap(),
    );
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![first, second.clone()]))
        .unwrap();
    let mut registry = BoardRegistry::new(ZcashNetwork::Mainnet);
    registry
        .register(
            BoardContext {
                repository: repo.clone(),
                source_type: BoardSourceType::Hosted,
                status: BoardStatus::Active,
            },
            &HostedBoardPolicy::default(),
            true,
        )
        .unwrap();
    let app = zadp_api::multi_board_router(Arc::new(registry));
    let report = serde_json::json!({"board_id":"ai-services","offer_id":second.id,"seller":b.id.as_str(),"reason":"spam","idempotency_key":"abcdefghijklmnop"});
    assert_eq!(
        call(app.clone(), "/reports", Some(report.clone()), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(repo.reports().unwrap()[0].seller, b.id.as_str());
    let mut invalid = report;
    invalid["offer_id"] = "unknown".into();
    assert_eq!(
        call(app, "/reports", Some(invalid), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(repo.reports().unwrap().len(), 1);
}

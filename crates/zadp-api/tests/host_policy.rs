use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tower::ServiceExt;
use zadp_api::{board_router_with_policy, HostPolicy};
use zadp_indexer::{InMemoryOfferRepository, OfferRecord, OfferRepository};
use zadp_protocol::{OfferDraft, Price, SellerKeyPair};
use zadp_zcash::TxId;

#[test]
fn strict_modes_and_fail_closed_configuration() {
    let key = SellerKeyPair::generate();
    assert!(HostPolicy::from_config(None, None)
        .unwrap()
        .permits(&key.id));
    for entries in [None, Some(""), Some("[]")] {
        assert!(!HostPolicy::from_config(Some("allowlist"), entries)
            .unwrap()
            .permits(&key.id));
    }
    for mode in ["", "unknown", "ALLOWLIST"] {
        assert!(HostPolicy::from_config(Some(mode), None).is_err());
    }
    for entries in ["not-json", r#"["ed25519:invalid"]"#, "{}"] {
        assert!(HostPolicy::from_config(Some("allowlist"), Some(entries)).is_err());
    }
    let entries = serde_json::to_string(&vec![key.id.clone(), key.id.clone()]).unwrap();
    assert!(HostPolicy::from_config(Some("allowlist"), Some(&entries))
        .unwrap()
        .permits(&key.id));
}

#[tokio::test]
async fn hosted_visibility_does_not_change_canonical_storage_or_geo() {
    let repo = Arc::new(InMemoryOfferRepository::new());
    let allowed = SellerKeyPair::generate();
    let hidden = SellerKeyPair::generate();
    for key in [&allowed, &hidden] {
        let offer = OfferDraft::new_ulid(
            key.id.clone(),
            "text.uppercase",
            "Fixture",
            Price::zec("0.01".parse().unwrap()),
            "https://service.example",
            Utc::now() + Duration::hours(1),
            "nonce",
        )
        .with_geo(zadp_protocol::GeoArea {
            lat_e5: 3777500,
            lon_e5: -12241900,
            radius_m: 25000,
        })
        .sign(key)
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
    let entries = serde_json::to_string(&vec![allowed.id.clone()]).unwrap();
    for (policy, count) in [
        (HostPolicy::default(), 2),
        (HostPolicy::from_config(Some("allowlist"), None).unwrap(), 0),
        (
            HostPolicy::from_config(Some("allowlist"), Some(&entries)).unwrap(),
            1,
        ),
    ] {
        let app = board_router_with_policy(repo.clone(), policy);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/offers?capability=text.uppercase")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = to_bytes(response.into_body(), 1_048_576).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["offers"].as_array().unwrap().len(), count);
        if count == 1 {
            assert_eq!(body["offers"][0]["seller"], allowed.id.as_str());
            assert_eq!(body["offers"][0]["geo"]["radius_m"], 25000);
        }
    }
    assert_eq!(repo.list(None).await.unwrap().len(), 2);
}

use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use zadp_api::{CommerceStatus, EndpointVerifier, ProofFetcher};
use zadp_protocol::{EndpointProof, Offer, OfferDraft, Price, SellerKeyPair};
struct Fake {
    result: Result<EndpointProof, CommerceStatus>,
    calls: AtomicUsize,
}
#[async_trait]
impl ProofFetcher for Fake {
    async fn fetch(&self, origin: &str) -> Result<EndpointProof, CommerceStatus> {
        assert_eq!(origin, "https://seller.example");
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}
fn offer(key: &SellerKeyPair, endpoint: &str) -> Offer {
    OfferDraft::new_ulid(
        key.id.clone(),
        "text.uppercase",
        "Test",
        Price::zec("0.01".parse().unwrap()),
        endpoint,
        Utc::now() + Duration::hours(1),
        "nonce",
    )
    .sign(key)
    .unwrap()
}
async fn drain() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}
fn proof(key: &SellerKeyPair) -> EndpointProof {
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp() - 1, 0).unwrap();
    EndpointProof::sign(key, "https://seller.example", now, now + Duration::days(1)).unwrap()
}
#[tokio::test]
async fn matching_proof_verified_but_other_seller_or_modified_offer_cannot_reuse_cache() {
    let key = SellerKeyPair::generate();
    let o = offer(&key, "https://seller.example");
    let fake = Arc::new(Fake {
        result: Ok(proof(&key)),
        calls: AtomicUsize::new(0),
    });
    let verifier = EndpointVerifier::new(fake.clone());
    assert_eq!(verifier.status(&o).status, CommerceStatus::Unverified);
    drain().await;
    assert_eq!(verifier.status(&o).status, CommerceStatus::Verified);
    let other = offer(&SellerKeyPair::generate(), "https://seller.example");
    assert_eq!(verifier.status(&other).status, CommerceStatus::Unverified);
    let mut tampered = o.clone();
    tampered.title = "Changed".into();
    assert_eq!(
        verifier.status(&tampered).status,
        CommerceStatus::Unverified
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    let whole_now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    let v2 = zadp_protocol::bind_offer(
        OfferDraft::new_ulid(
            key.id.clone(),
            "text.uppercase",
            "Geo v2",
            Price::zec("0.01".parse().unwrap()),
            "https://seller.example",
            whole_now + Duration::hours(1),
            "AAAAAAAAAAAAAAAAAAAAAA==",
        )
        .with_geo(zadp_protocol::GeoArea {
            lat_e5: 3777500,
            lon_e5: -12241900,
            radius_m: 25000,
        })
        .sign(&key)
        .unwrap(),
        "demo",
        whole_now,
        &key,
    )
    .unwrap();
    assert_eq!(v2.zadp, 2);
    let before = zadp_protocol::encode_offer_memo(&v2).unwrap();
    assert_eq!(verifier.status(&v2).status, CommerceStatus::Verified);
    assert_eq!(zadp_protocol::encode_offer_memo(&v2).unwrap(), before);
}
#[tokio::test]
async fn failed_proofs_fail_closed_without_altering_signed_offers() {
    let key = SellerKeyPair::generate();
    let o = offer(&key, "https://seller.example");
    let original = serde_json::to_value(&o).unwrap();
    let mut modified = proof(&key);
    modified.expires_at += Duration::days(1);
    let mut wrong_origin = proof(&key);
    wrong_origin.origin = "https://other.example".into();
    let old = chrono::DateTime::from_timestamp(Utc::now().timestamp() - 100, 0).unwrap();
    let expired = EndpointProof::sign(
        &key,
        "https://seller.example",
        old,
        old + Duration::seconds(50),
    )
    .unwrap();
    for (result, status) in [
        (
            Err(CommerceStatus::Unavailable),
            CommerceStatus::Unavailable,
        ),
        (Err(CommerceStatus::Unverified), CommerceStatus::Unverified),
        (Ok(modified), CommerceStatus::Unverified),
        (Ok(wrong_origin), CommerceStatus::Unverified),
        (Ok(expired), CommerceStatus::Unverified),
        (
            Ok(proof(&SellerKeyPair::generate())),
            CommerceStatus::Unverified,
        ),
    ] {
        let fake = Arc::new(Fake {
            result,
            calls: AtomicUsize::new(0),
        });
        let verifier = EndpointVerifier::new(fake);
        verifier.status(&o);
        drain().await;
        assert_eq!(verifier.status(&o).status, status);
        assert_eq!(serde_json::to_value(&o).unwrap(), original);
    }
}
#[tokio::test]
async fn discovery_only_and_unsafe_endpoints_never_fetch() {
    let key = SellerKeyPair::generate();
    let fake = Arc::new(Fake {
        result: Err(CommerceStatus::Unavailable),
        calls: AtomicUsize::new(0),
    });
    let verifier = EndpointVerifier::new(fake.clone());
    let o = OfferDraft::new_discovery(
        key.id.clone(),
        "local.service",
        "Discovery",
        Utc::now() + Duration::hours(1),
        "nonce",
    )
    .sign(&key)
    .unwrap();
    assert_eq!(verifier.status(&o).status, CommerceStatus::DiscoveryOnly);
    for url in [
        "http://seller.example",
        "https://user:pass@seller.example",
        "https://localhost",
        "https://10.0.0.1",
        "https://169.254.169.254",
        "https://[::1]",
    ] {
        assert_eq!(
            verifier.status(&offer(&key, url)).status,
            CommerceStatus::Unverified
        );
    }
    drain().await;
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn unavailable_endpoint_keeps_offer_in_read_only_hosted_api_and_repository() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    use zadp_indexer::{InMemoryOfferRepository, OfferRecord, OfferRepository};
    let key = SellerKeyPair::generate();
    let o = offer(&key, "https://seller.example");
    let original = o.clone();
    let repo = Arc::new(InMemoryOfferRepository::new());
    repo.upsert_first_wins(OfferRecord {
        offer: o,
        source_txid: zadp_zcash::TxId("fixture".into()),
        indexed_at: Utc::now(),
        chain: None,
    })
    .await
    .unwrap();
    let verifier = EndpointVerifier::new(Arc::new(Fake {
        result: Err(CommerceStatus::Unavailable),
        calls: AtomicUsize::new(0),
    }));
    let app = zadp_api::board_router_with_verifier(
        repo.clone(),
        zadp_api::HostPolicy::default(),
        verifier,
    );
    for expected in ["unverified", "unavailable"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/offers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let mut data: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(
            data["offers"][0]["commerce_verification"]["status"],
            expected
        );
        data["offers"][0]
            .as_object_mut()
            .unwrap()
            .remove("commerce_verification");
        data["offers"][0]
            .as_object_mut()
            .unwrap()
            .remove("purchase_mode");
        assert_eq!(data["offers"][0], serde_json::to_value(&original).unwrap());
        drain().await;
    }
    assert_eq!(repo.list(None).await.unwrap()[0].offer, original);
    for route in [
        "/orders",
        "/wallet",
        "/endpoint-proof?url=https://seller.example",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(Request::builder().uri(route).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            404
        );
    }
}

#[tokio::test]
async fn v3_modes_are_explicit_in_api_and_manual_never_fetches_endpoint() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    use zadp_indexer::{InMemoryOfferRepository, OfferRecord, OfferRepository};
    let k = SellerKeyPair::generate();
    let repo = Arc::new(InMemoryOfferRepository::new());
    for mode in 0..3 {
        let mut draft = OfferDraft::new_discovery(
            k.id.clone(),
            "text.uppercase",
            "Mode",
            Utc::now() + Duration::hours(1),
            "nonce",
        )
        .with_version(3);
        if mode > 0 {
            draft.price = Some(Price::zec("0.02".parse().unwrap()));
        }
        if mode == 2 {
            draft.commerce = Some(zadp_protocol::Commerce {
                endpoint: "https://seller.example".into(),
            });
        }
        repo.upsert_first_wins(OfferRecord {
            offer: draft.sign(&k).unwrap(),
            source_txid: zadp_zcash::TxId("fixture".into()),
            indexed_at: Utc::now(),
            chain: None,
        })
        .await
        .unwrap();
    }
    let fake = Arc::new(Fake {
        result: Ok(proof(&k)),
        calls: AtomicUsize::new(0),
    });
    let verifier = EndpointVerifier::new(fake.clone());
    let records = repo.list(None).await.unwrap();
    for r in records.iter().filter(|r| r.offer.commerce.is_none()) {
        verifier.status(&r.offer);
    }
    drain().await;
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    let app = zadp_api::board_router_with_verifier(
        repo.clone(),
        zadp_api::HostPolicy::default(),
        verifier.clone(),
    );
    let res = app
        .oneshot(
            Request::builder()
                .uri("/offers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let data: serde_json::Value =
        serde_json::from_slice(&to_bytes(res.into_body(), 65536).await.unwrap()).unwrap();
    for value in data["offers"].as_array().unwrap() {
        match value["purchase_mode"].as_str().unwrap() {
            "discovery" => {
                assert!(value.get("price").is_none());
                assert!(value.get("commerce").is_none());
            }
            "manual" => {
                assert_eq!(value["price"]["amount"], "0.02");
                assert!(value.get("commerce").is_none());
                assert_eq!(value["commerce_verification"]["status"], "not_required");
            }
            "automated" => {
                assert_eq!(value["price"]["amount"], "0.02");
                assert_eq!(value["commerce"]["endpoint"], "https://seller.example");
                assert!(value["commerce"].get("price").is_none());
            }
            _ => panic!("unknown mode"),
        }
    }
    drain().await;
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    for r in records.iter().filter(|r| r.offer.commerce.is_some()) {
        assert_eq!(verifier.status(&r.offer).status, CommerceStatus::Verified);
    }
}

use chrono::{Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use std::{fs, os::unix::fs::PermissionsExt};
use zadp_commerce::{
    random_hex, Config, ObserverConfig, OrderRequest, OrderResponse, PaymentEvidence, Snapshot,
    Store, TextRequest,
};
use zadp_protocol::{OfferDraft, Price, SellerKeyPair, ZcashNetwork};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};

fn fixture() -> (tempfile::TempDir, Config, i64) {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let params = ZcashNetwork::Testnet.parameters();
    let mut seed = [0u8; 64];
    OsRng.fill_bytes(&mut seed);
    let address = UnifiedSpendingKey::from_seed(&params, &seed, AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key()
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap()
        .encode(&params);
    let keys = SellerKeyPair::generate();
    let now = Utc::now().timestamp();
    let offer = OfferDraft::new_ulid(
        keys.id.clone(),
        "text.uppercase",
        "Known local offer",
        Price::zec(rust_decimal::Decimal::new(1, 2)),
        "http://127.0.0.1:8094",
        Utc::now() + Duration::hours(24),
        "bG9jYWw=",
    )
    .sign(&keys)
    .unwrap();
    let config = Config {
        offer,
        network: ZcashNetwork::Testnet,
        recipient: address,
        seller_account: "seller-only".into(),
        database: dir.path().join("orders.sqlite"),
        confirmations: 3,
        order_lifetime_seconds: 600,
        browser_origin: "http://127.0.0.1:8095".into(),
        observer: ObserverConfig::Mock {
            snapshot_file: dir.path().join("snapshot.json"),
        },
    };
    (dir, config, now)
}
fn request(c: &Config) -> OrderRequest {
    OrderRequest {
        offer_id: c.offer.id.clone(),
        request: TextRequest {
            text: "hello world ß".into(),
        },
    }
}
fn evidence(c: &Config, r: &OrderResponse, now: i64) -> Snapshot {
    Snapshot {
        synchronized: true,
        tip: 12,
        payments: vec![PaymentEvidence {
            txid: "a".repeat(64),
            pool: "orchard".into(),
            output_index: 0,
            account: c.seller_account.clone(),
            recipient: r.recipient.clone(),
            amount_zat: r.amount_zat,
            memo: r.memo.clone(),
            is_change: false,
            mined_height: 10,
            block_time: now + 1,
        }],
    }
}
fn settle(s: &mut Store, r: &OrderResponse, p: &Snapshot, now: i64) -> OrderResponse {
    let token = r.claim_token.as_ref().unwrap();
    assert_eq!(s.check(&r.order_id, token, p, now).unwrap().status, "paid");
    assert_eq!(
        s.check(&r.order_id, token, p, now).unwrap().status,
        "processing"
    );
    s.check(&r.order_id, token, p, now).unwrap()
}
#[test]
fn vertical_slice_zip321_exactly_once_and_restart() {
    let (_dir, c, now) = fixture();
    let mut s = Store::open(c.clone()).unwrap();
    let key = random_hex(32);
    let r = s.create(request(&c), &key, now).unwrap();
    let uri = zip321::TransactionRequest::from_uri(&r.payment_request).unwrap();
    let payment = uri.payments().values().next().unwrap();
    assert_eq!(payment.recipient_address().to_string(), r.recipient);
    assert_eq!(u64::from(payment.amount().unwrap()), r.amount_zat);
    assert_eq!(
        payment.memo().unwrap().as_array()[..r.memo.len()],
        *r.memo.as_bytes()
    );
    assert!(r.payment_qr_svg.contains("<svg"));
    assert!(s
        .check(&r.order_id, "wrong", &Snapshot::default(), now)
        .is_err());
    let p = evidence(&c, &r, now);
    let complete = settle(&mut s, &r, &p, now + 5);
    assert_eq!(complete.result.as_deref(), Some("HELLO WORLD SS"));
    assert_eq!(
        complete.transitions,
        vec![
            "created",
            "waiting_payment",
            "paid",
            "processing",
            "completed"
        ]
    );
    drop(s);
    let mut s = Store::open(c.clone()).unwrap();
    for _ in 0..5 {
        assert_eq!(
            s.check(&r.order_id, r.claim_token.as_ref().unwrap(), &p, now + 7)
                .unwrap()
                .result,
            complete.result
        );
    }
    let db = rusqlite::Connection::open(&c.database).unwrap();
    let count: u64 = db
        .query_row("SELECT execution_count FROM orders", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let recovered = s.create(request(&c), &key, now + 9).unwrap();
    assert_eq!(recovered.order_id, r.order_id);
    assert_eq!(recovered.claim_token, r.claim_token);
    assert!(recovered.result.is_none());
    let stored: String = db
        .query_row("SELECT token_hash FROM orders", [], |r| r.get(0))
        .unwrap();
    assert_ne!(stored, r.claim_token.unwrap());
}
#[test]
fn rejects_wrong_payment_fields_and_late_mining() {
    let (_dir, c, now) = fixture();
    let mut s = Store::open(c.clone()).unwrap();
    let r = s.create(request(&c), &random_hex(32), now).unwrap();
    for field in 0..11 {
        let mut p = evidence(&c, &r, now);
        let e = &mut p.payments[0];
        match field {
            0 => e.amount_zat -= 1,
            1 => e.amount_zat += 1,
            2 => e.memo.push('x'),
            3 => e.account = "other".into(),
            4 => e.recipient = "wrong".into(),
            5 => e.is_change = true,
            6 => e.pool = "transparent".into(),
            7 => e.txid = "invalid".into(),
            8 => e.mined_height = 0,
            9 => e.block_time = r.expires_at + 1,
            _ => e.block_time = now - 1,
        }
        let result = s
            .check(&r.order_id, r.claim_token.as_ref().unwrap(), &p, now + 2)
            .unwrap();
        assert_eq!(result.status, "waiting_payment");
        assert!(result.result.is_none());
    }
    let complete = settle(&mut s, &r, &evidence(&c, &r, now), now + 3);
    assert_eq!(complete.status, "completed");
}
#[test]
fn confirmation_policy_reorg_and_observer_fail_closed() {
    let (_dir, c, now) = fixture();
    let mut s = Store::open(c.clone()).unwrap();
    let r = s.create(request(&c), &random_hex(32), now).unwrap();
    let mut p = evidence(&c, &r, now);
    p.tip = 10;
    assert_eq!(
        s.check(&r.order_id, r.claim_token.as_ref().unwrap(), &p, now + 700)
            .unwrap()
            .payment_check,
        "confirming"
    );
    p.tip = 12;
    settle(&mut s, &r, &p, now + 701);
    p.synchronized = false;
    assert!(s
        .check(&r.order_id, r.claim_token.as_ref().unwrap(), &p, now + 702)
        .unwrap()
        .result
        .is_none());
    p.synchronized = true;
    p.payments.clear();
    let result = s
        .check(&r.order_id, r.claim_token.as_ref().unwrap(), &p, now + 703)
        .unwrap();
    assert_eq!(result.payment_check, "payment_evidence_changed");
    assert!(result.result.is_none());
}
#[test]
fn expiry_cancellation_idempotency_and_request_bounds() {
    let (_dir, c, now) = fixture();
    let mut s = Store::open(c.clone()).unwrap();
    let key = random_hex(32);
    let r = s.create(request(&c), &key, now).unwrap();
    let mut changed = request(&c);
    changed.request.text = "different".into();
    assert!(s.create(changed, &key, now).is_err());
    for text in ["".into(), " ".into(), "x".repeat(4001)] {
        let mut req = request(&c);
        req.request.text = text;
        assert!(s.create(req, &random_hex(32), now).is_err());
    }
    let mut req = request(&c);
    req.offer_id = "unsupported".into();
    assert!(s.create(req, &random_hex(32), now).is_err());
    assert!(s
        .create(
            request(&c),
            &random_hex(32),
            c.offer.expires_at.timestamp() + 1
        )
        .is_err());
    let empty = Snapshot {
        synchronized: true,
        tip: 12,
        payments: vec![],
    };
    assert_eq!(
        s.check(
            &r.order_id,
            r.claim_token.as_ref().unwrap(),
            &empty,
            now + 700
        )
        .unwrap()
        .status,
        "expired"
    );
    assert!(s
        .cancel(&r.order_id, r.claim_token.as_ref().unwrap())
        .is_err());
    let r = s.create(request(&c), &random_hex(32), now).unwrap();
    assert_eq!(
        s.cancel(&r.order_id, r.claim_token.as_ref().unwrap())
            .unwrap()
            .status,
        "cancelled"
    );
    assert_eq!(
        s.check(
            &r.order_id,
            r.claim_token.as_ref().unwrap(),
            &evidence(&c, &r, now),
            now + 4
        )
        .unwrap()
        .status,
        "cancelled"
    );
    assert!(s
        .check("unknown", r.claim_token.as_ref().unwrap(), &empty, now)
        .is_err());
}
#[test]
fn configuration_and_database_boundaries() {
    let (_dir, c, _) = fixture();
    Store::open(c.clone()).unwrap();
    let mut changed = c.clone();
    changed.confirmations += 1;
    assert!(Store::open(changed).is_err());
    let mut changed = c.clone();
    changed.network = ZcashNetwork::Mainnet;
    assert!(changed.validate().is_err());
    fs::set_permissions(&c.database, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Store::open(c).is_err());
}
#[test]
fn observer_requires_explicit_non_change_and_correct_account() {
    let mut tx = serde_json::json!({"account_uuid":"a","mined_height":10,"block_time":100,"txid":"a".repeat(64),"outputs":[{"to_account":"a","to_address":"u","pool":"orchard","value":100,"memo":"m","output_index":0}]});
    assert!(
        zadp_commerce::observer::snapshot_from_transactions(&[tx.clone()], 12)
            .payments
            .is_empty()
    );
    tx["outputs"][0]["is_change"] = false.into();
    assert_eq!(
        zadp_commerce::observer::snapshot_from_transactions(&[tx.clone()], 12)
            .payments
            .len(),
        1
    );
    tx["outputs"][0]["to_account"] = "other".into();
    assert!(
        zadp_commerce::observer::snapshot_from_transactions(&[tx], 12)
            .payments
            .is_empty()
    );
}
#[tokio::test]
async fn http_boundaries_and_direct_client_contract() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (_dir, c, _) = fixture();
    let app = zadp_commerce::http::router(c.clone()).unwrap();
    let body = serde_json::to_string(&request(&c)).unwrap();
    let key = random_hex(32);
    let create = || {
        Request::builder()
            .method("POST")
            .uri("/orders")
            .header("content-type", "application/json")
            .header("idempotency-key", &key)
            .header("origin", &c.browser_origin)
            .body(Body::from(body.clone()))
            .unwrap()
    };
    let response = app.clone().oneshot(create()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let r: OrderResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
    let response = app.clone().oneshot(create()).await.unwrap();
    let repeat: OrderResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
    assert_eq!(r.order_id, repeat.order_id);
    for (route, method) in [
        (format!("/orders/{}", r.order_id), "GET"),
        ("/orders/unknown".into(), "GET"),
        ("/pay".into(), "POST"),
        ("/wallet".into(), "GET"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(route)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::NOT_FOUND
        ));
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .header("origin", "https://untrusted.example")
                .header("idempotency-key", &key)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from("x".repeat(9000)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[test]
fn one_output_cannot_pay_two_orders_and_processing_recovers() {
    let (_dir, c, now) = fixture();
    let mut store = Store::open(c.clone()).unwrap();
    let first = store.create(request(&c), &random_hex(32), now).unwrap();
    let second = store.create(request(&c), &random_hex(32), now).unwrap();
    let p = evidence(&c, &first, now);
    assert_eq!(
        store
            .check(
                &first.order_id,
                first.claim_token.as_ref().unwrap(),
                &p,
                now + 3
            )
            .unwrap()
            .status,
        "paid"
    );
    drop(store);
    let mut store = Store::open(c.clone()).unwrap();
    assert_eq!(
        store
            .check(
                &first.order_id,
                first.claim_token.as_ref().unwrap(),
                &p,
                now + 4
            )
            .unwrap()
            .status,
        "processing"
    );
    drop(store);
    let mut store = Store::open(c.clone()).unwrap();
    assert_eq!(
        store
            .check(
                &first.order_id,
                first.claim_token.as_ref().unwrap(),
                &p,
                now + 5
            )
            .unwrap()
            .status,
        "completed"
    );
    let mut contradictory = p;
    contradictory.payments[0].memo = second.memo.clone();
    let result = store
        .check(
            &second.order_id,
            second.claim_token.as_ref().unwrap(),
            &contradictory,
            now + 6,
        )
        .unwrap();
    assert_eq!(result.payment_check, "payment_already_used");
    assert_eq!(result.status, "waiting_payment");
    assert!(result.result.is_none());
}

#[tokio::test]
async fn http_full_slice_persists_only_in_seller_database() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let (_dir, c, now) = fixture();
    let app = zadp_commerce::http::router(c.clone()).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .header("idempotency-key", random_hex(32))
                .body(Body::from(serde_json::to_vec(&request(&c)).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let order: OrderResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
    let ObserverConfig::Mock { snapshot_file } = &c.observer else {
        panic!()
    };
    fs::write(
        snapshot_file,
        serde_json::to_vec(&evidence(&c, &order, now)).unwrap(),
    )
    .unwrap();
    fs::set_permissions(snapshot_file, fs::Permissions::from_mode(0o600)).unwrap();
    for status in ["paid", "processing", "completed", "completed"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/orders/{}", order.order_id))
                    .header(
                        "authorization",
                        format!("Bearer {}", order.claim_token.as_ref().unwrap()),
                    )
                    .header("origin", &c.browser_origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            c.browser_origin
        );
        let result: OrderResponse =
            serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
        assert_eq!(result.status, status);
        if status == "completed" {
            assert_eq!(result.result.as_deref(), Some("HELLO WORLD SS"));
        }
    }
    let db = rusqlite::Connection::open(&c.database).unwrap();
    let tables: Vec<String> = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(tables, vec!["commerce_meta", "orders"]);
}

#[test]
fn refuses_existing_board_database_without_modifying_it() {
    let (_dir, c, _) = fixture();
    let db = rusqlite::Connection::open(&c.database).unwrap();
    db.execute("CREATE TABLE offers(id TEXT PRIMARY KEY)", [])
        .unwrap();
    drop(db);
    fs::set_permissions(&c.database, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Store::open(c.clone()).is_err());
    let db = rusqlite::Connection::open(&c.database).unwrap();
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn static_well_known_proof_is_served_without_a_signing_key() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let (_dir, mut c, _) = fixture();
    let key = SellerKeyPair::from_secret_bytes(&[9; 32]);
    c.offer = OfferDraft::new_ulid(
        key.id.clone(),
        "text.uppercase",
        "Public endpoint fixture",
        Price::zec("0.01".parse().unwrap()),
        "https://seller.example",
        Utc::now() + Duration::days(1),
        "nonce",
    )
    .sign(&key)
    .unwrap();
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    let proof = zadp_protocol::EndpointProof::sign(
        &key,
        "https://seller.example",
        now,
        now + Duration::days(1),
    )
    .unwrap();
    let document = serde_json::to_value(&proof).unwrap();
    drop(key); // The server receives only Config and public signed JSON.
    let app = zadp_commerce::http::router_with_endpoint_proof(c.clone(), Some(proof)).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/zadp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        document
    );
    let other = SellerKeyPair::generate();
    let wrong = zadp_protocol::EndpointProof::sign(
        &other,
        "https://seller.example",
        now,
        now + Duration::days(1),
    )
    .unwrap();
    assert!(zadp_commerce::http::router_with_endpoint_proof(c.clone(), Some(wrong)).is_err());
    let app = zadp_commerce::http::router(c).unwrap();
    assert_eq!(
        app.oneshot(
            Request::builder()
                .uri("/.well-known/zadp")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap()
        .status(),
        404
    );
}

#[tokio::test]
async fn summarize_capability_description_and_idempotent_execution() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let (_dir, mut c, now) = fixture();
    let keys = SellerKeyPair::generate();
    c.offer = OfferDraft::new_ulid(
        keys.id.clone(),
        "text.summarize",
        "Extractive text summary",
        Price::zec(rust_decimal::Decimal::new(1, 2)),
        "http://127.0.0.1:8094",
        Utc::now() + Duration::hours(24),
        "bG9jYWw=",
    )
    .sign(&keys)
    .unwrap();
    let app = zadp_commerce::http::router(c.clone()).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let descriptor: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    assert_eq!(descriptor["offer_id"], c.offer.id);
    assert_eq!(descriptor["capability"], "text.summarize");
    assert_eq!(descriptor["response_schema"]["type"], "string");
    let mut store = Store::open(c.clone()).unwrap();
    let req = OrderRequest {
        offer_id: c.offer.id.clone(),
        request: TextRequest {
            text: "One. Two! Three? Four.".into(),
        },
    };
    let order = store.create(req, &random_hex(32), now).unwrap();
    let snapshot = evidence(&c, &order, now);
    let completed = settle(&mut store, &order, &snapshot, now + 5);
    assert_eq!(completed.result.as_deref(), Some("One. Two! Three?"));
    for _ in 0..3 {
        assert_eq!(
            store
                .check(
                    &order.order_id,
                    order.claim_token.as_ref().unwrap(),
                    &snapshot,
                    now + 7
                )
                .unwrap()
                .result,
            completed.result
        );
    }
    let db = rusqlite::Connection::open(c.database).unwrap();
    let count: i64 = db
        .query_row("SELECT execution_count FROM orders", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

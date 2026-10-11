use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration};
use serde_json::json;
use sha2::{Digest, Sha256};
use zadp_protocol::*;
use zadp_seller::revoke::validate_approval;
#[test]
fn exact_revoke_review_binds_every_cost_target_and_proposal() {
    let params = ZcashNetwork::Testnet.parameters();
    let ivk =
        zcash_keys::keys::UnifiedSpendingKey::from_seed(&params, &[8; 64], zip32::AccountId::ZERO)
            .unwrap()
            .to_unified_full_viewing_key()
            .to_unified_incoming_viewing_key();
    let key = SellerKeyPair::from_secret_bytes(&[42; 32]);
    let time = DateTime::from_timestamp(1893456000, 0).unwrap();
    let board = BoardBody {
        zadp: 1,
        board_id: "hello".into(),
        name: "Fixture".into(),
        network: ZcashNetwork::Testnet,
        address: ivk
            .address(
                zip32::DiversifierIndex::from(0u32),
                zcash_keys::keys::UnifiedAddressRequest::ORCHARD,
            )
            .unwrap()
            .encode(&params),
        incoming_viewing_key: ivk.encode(&params),
        birthday_height: 3_000_000,
        policy: BoardPolicy::default(),
        operator_key: key.id.clone(),
    }
    .sign(&key)
    .unwrap();
    let r = OfferRevoke::sign(
        "hello".into(),
        "target".into(),
        time,
        B64.encode([7; 16]),
        &key,
    )
    .unwrap();
    let preview = json!({"kind":"unsigned-revocation-preview","broadcast":false,"zcash_transaction_signed":false,"offer_revoke":r,"board_id":"hello","board_fingerprint":board.fingerprint().unwrap(),"board_recipient":board.board.address,"network":"testnet","listing_value_zat":1000,"network_fee_zat":20000,"total_cost_zat":21000,"proposal_fingerprint":hex::encode(Sha256::digest(b"synthetic")),"preview_expires_at":time.timestamp()+900});
    validate_approval(&preview, &r, &board, 1000, 20000, "synthetic", time).unwrap();
    for (field, value) in [
        ("kind", json!("unsigned-publication-preview")),
        ("board_id", json!("wrong")),
        ("board_fingerprint", json!("wrong")),
        ("board_recipient", json!("wrong")),
        ("network", json!("mainnet")),
        ("listing_value_zat", json!(1001)),
        ("network_fee_zat", json!(1)),
        ("total_cost_zat", json!(1)),
        ("proposal_fingerprint", json!("wrong")),
        ("broadcast", json!(true)),
        ("zcash_transaction_signed", json!(true)),
    ] {
        let mut bad = preview.clone();
        bad[field] = value;
        assert!(validate_approval(&bad, &r, &board, 1000, 20000, "synthetic", time).is_err());
    }
    assert!(validate_approval(&preview, &r, &board, 1000, 20000, "changed", time).is_err());
    assert!(validate_approval(
        &preview,
        &r,
        &board,
        1000,
        20000,
        "synthetic",
        time + Duration::seconds(900)
    )
    .is_err());
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("missing.sqlite");
    assert!(zadp_seller::revoke::resolve_target(&path, &board, &key.id, "target").is_err());
    assert!(!path.exists());
    let offer = bind_offer(
        OfferDraft::new_discovery(
            key.id.clone(),
            "local.guide",
            "Test",
            time + Duration::days(1),
            B64.encode([1; 16]),
        )
        .sign(&key)
        .unwrap(),
        "hello",
        time,
        &key,
    )
    .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT); CREATE TABLE offers(seller TEXT,id TEXT,json TEXT); CREATE TABLE offer_revocations(seller TEXT,offer_id TEXT);").unwrap();
    db.execute(
        "INSERT INTO metadata VALUES('board_fingerprint',?1)",
        [board.fingerprint().unwrap()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO offers VALUES(?1,?2,?3)",
        rusqlite::params![
            key.id.as_str(),
            offer.id,
            serde_json::to_string(&offer).unwrap()
        ],
    )
    .unwrap();
    assert_eq!(
        zadp_seller::revoke::resolve_target(&path, &board, &key.id, &offer.id).unwrap(),
        offer
    );
    let other = SellerKeyPair::from_secret_bytes(&[43; 32]);
    assert!(zadp_seller::revoke::resolve_target(&path, &board, &other.id, &offer.id).is_err());
    db.execute(
        "INSERT INTO offer_revocations VALUES(?1,?2)",
        rusqlite::params![key.id.as_str(), offer.id],
    )
    .unwrap();
    assert!(zadp_seller::revoke::resolve_target(&path, &board, &key.id, &offer.id).is_err());
}

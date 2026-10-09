use chrono::{Duration, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use zadp_protocol::*;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};
#[test]
fn exact_proposal_approval_rejects_mutations_and_expiry() {
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    let params = ZcashNetwork::Mainnet.parameters();
    let ivk = UnifiedSpendingKey::from_seed(&params, &[42; 64], AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let address = ivk
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap()
        .encode(&params);
    let key = SellerKeyPair::from_secret_bytes(&[13; 32]);
    let board = BoardBody {
        zadp: 1,
        board_id: "hello".into(),
        name: "Hello".into(),
        network: ZcashNetwork::Mainnet,
        address,
        incoming_viewing_key: ivk.encode(&params),
        birthday_height: 3_000_000,
        policy: BoardPolicy::default(),
        operator_key: key.id.clone(),
    }
    .sign(&key)
    .unwrap();
    let offer = bind_offer(
        OfferDraft::new_discovery(
            key.id.clone(),
            "hello",
            "Hello",
            now + Duration::days(1),
            "MDEyMzQ1Njc4OWFiY2RlZg==",
        )
        .sign(&key)
        .unwrap(),
        "hello",
        now,
        &key,
    )
    .unwrap();
    let pczt = "synthetic-never-broadcast";
    let p = json!({"kind":"unsigned-publication-preview","broadcast":false,"zcash_transaction_signed":false,"offer":offer,"board_id":"hello","board_fingerprint":board.fingerprint().unwrap(),"board_recipient":board.board.address,"network":"mainnet","listing_value_zat":1000,"network_fee_zat":10000,"total_cost_zat":11000,"proposal_fingerprint":hex::encode(Sha256::digest(pczt.as_bytes())),"preview_expires_at":now.timestamp()+900});
    zadp_seller::publication::validate_approval(&p, &offer, &board, 1000, 10000, pczt, now)
        .unwrap();
    for field in [
        "board_recipient",
        "board_fingerprint",
        "network",
        "offer",
        "listing_value_zat",
        "network_fee_zat",
        "total_cost_zat",
        "proposal_fingerprint",
    ] {
        let mut bad = p.clone();
        bad[field] = json!("changed");
        assert!(
            zadp_seller::publication::validate_approval(
                &bad, &offer, &board, 1000, 10000, pczt, now
            )
            .is_err(),
            "{field}"
        );
    }
    assert!(zadp_seller::publication::validate_approval(
        &p,
        &offer,
        &board,
        1000,
        10000,
        "different",
        now
    )
    .is_err());
    assert!(zadp_seller::publication::validate_approval(
        &p,
        &offer,
        &board,
        1000,
        10000,
        pczt,
        now + Duration::seconds(900)
    )
    .is_err());
}

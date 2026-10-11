use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration};
use zadp_protocol::*;
fn revoke() -> OfferRevoke {
    OfferRevoke::sign(
        "hello".into(),
        "target".into(),
        DateTime::from_timestamp(1893456000, 0).unwrap(),
        B64.encode([7; 16]),
        &SellerKeyPair::from_secret_bytes(&[42; 32]),
    )
    .unwrap()
}
#[test]
fn signed_fields_and_separate_domain() {
    let r = revoke();
    r.verify().unwrap();
    assert!(r
        .signing_bytes()
        .unwrap()
        .starts_with(REVOKE_SIGNING_DOMAIN));
    let mut changes = vec![];
    let mut x = r.clone();
    x.offer_id = "other".into();
    changes.push(x);
    let mut x = r.clone();
    x.board_id = "other".into();
    changes.push(x);
    let mut x = r.clone();
    x.seller = SellerKeyPair::generate().id;
    changes.push(x);
    let mut x = r.clone();
    x.issued_at += Duration::seconds(1);
    changes.push(x);
    let mut x = r.clone();
    x.nonce = B64.encode([8; 16]);
    changes.push(x);
    let mut x = r.clone();
    x.zadp = 3;
    changes.push(x);
    let mut x = r.clone();
    x.message_type = "offer".into();
    changes.push(x);
    for x in changes {
        assert!(x.verify().is_err());
    }
    for nonce in ["", "bad", "AA=="] {
        let mut x = r.clone();
        x.nonce = nonce.into();
        assert!(x.verify().is_err());
    }
    let mut x = r.clone();
    x.issued_at = DateTime::from_timestamp(-1, 0).unwrap();
    assert!(x.verify().is_err());
}
#[test]
fn exact_additive_framing_and_strict_roundtrip() {
    let r = revoke();
    let memo = encode_revoke_memo(&r).unwrap();
    assert_eq!(&memo[..6], b"\xf5ZADP\x81");
    assert_eq!(memo[8], 0x87);
    assert!(u16::from_be_bytes([memo[6], memo[7]]) <= 504);
    assert_eq!(
        decode_zadp_memo(&memo).unwrap(),
        Some(ZadpMessage::OfferRevoke(r))
    );
    // Older Offer-only readers fail closed on new control messages, never reinterpret them.
    assert!(decode_offer_memo(&memo).is_err());
    for n in 5..512 {
        assert!(decode_zadp_memo(&memo[..n]).is_err());
    }
    let mut bad = memo;
    bad[511] = 1;
    assert!(decode_zadp_memo(&bad).is_err());
    let mut bad = memo;
    bad[5] = 3;
    assert!(decode_zadp_memo(&bad).is_err());
    assert!(decode_zadp_memo(&[0; 512]).unwrap().is_none());
}
#[test]
fn generic_decoder_preserves_offer_golden_bytes() {
    let mut fixtures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/offer-legacy-commercial-golden.json")).unwrap();
    fixtures
        .push(serde_json::from_str(include_str!("fixtures/offer-v2-mainnet-golden.json")).unwrap());
    fixtures.push(serde_json::from_str(include_str!("fixtures/offer-v3-golden.json")).unwrap());
    for f in fixtures {
        let o: Offer = serde_json::from_value(f["offer"].clone()).unwrap();
        let memo = encode_offer_memo(&o).unwrap();
        let hex: String = memo.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, f["memo_hex"]);
        assert_eq!(
            decode_zadp_memo(&memo).unwrap(),
            Some(ZadpMessage::Offer(o))
        );
    }
}
#[test]
fn maximum_control_payload_and_policy_bounds() {
    let key = SellerKeyPair::from_secret_bytes(&[42; 32]);
    let time = DateTime::from_timestamp(i64::from(u32::MAX), 0).unwrap();
    let r = OfferRevoke::sign(
        "b".repeat(64),
        "i".repeat(64),
        time,
        B64.encode([3; 32]),
        &key,
    )
    .unwrap();
    let memo = encode_revoke_memo(&r).unwrap();
    assert!(u16::from_be_bytes([memo[6], memo[7]]) < 300);
    assert_eq!(
        decode_zadp_memo(&memo).unwrap(),
        Some(ZadpMessage::OfferRevoke(r))
    );
    for (board, id, nonce) in [
        ("b".repeat(65), "id".into(), B64.encode([3; 16])),
        ("b".into(), "i".repeat(65), B64.encode([3; 16])),
        ("b".into(), "id".into(), B64.encode([3; 33])),
    ] {
        assert!(OfferRevoke::sign(board, id, time, nonce, &key).is_err());
    }
}

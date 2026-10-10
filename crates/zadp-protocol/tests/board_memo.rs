use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{Duration, Utc};
use rand::RngCore;
use rust_decimal::Decimal;
use zadp_protocol::*;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};

fn board(keys: &SellerKeyPair) -> BoardDescriptor {
    let params = ZcashNetwork::Mainnet.parameters();
    let mut seed = [0u8; 64];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let usk = UnifiedSpendingKey::from_seed(&params, &seed, AccountId::ZERO).unwrap();
    let ivk = usk
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let ua = ivk
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap();
    BoardBody {
        zadp: 1,
        board_id: "ai-services".into(),
        name: "ZADP AI Services".into(),
        network: ZcashNetwork::Mainnet,
        address: ua.encode(&params),
        incoming_viewing_key: ivk.encode(&params),
        birthday_height: 3_000_000,
        policy: BoardPolicy::default(),
        operator_key: keys.id.clone(),
    }
    .sign(keys)
    .unwrap()
}
fn offer(keys: &SellerKeyPair) -> Offer {
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp() - 1, 0).unwrap();
    let draft = OfferDraft::new_ulid(
        keys.id.clone(),
        "llm.inference",
        "ZADP Demo Inference",
        Price::zec(Decimal::new(1, 2)),
        "https://example.com/zadp",
        now + Duration::hours(24),
        B64.encode([7u8; 16]),
    )
    .sign(keys)
    .unwrap();
    bind_offer(draft, "ai-services", now, keys).unwrap()
}
#[test]
fn portable_descriptor_validates_and_network_mismatch_fails() {
    let keys = SellerKeyPair::generate();
    let b = board(&keys);
    b.validate(ZcashNetwork::Mainnet).unwrap();
    assert!(b.validate(ZcashNetwork::Testnet).is_err());
    let json = serde_json::to_string(&b).unwrap();
    let b2: BoardDescriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(b.fingerprint().unwrap(), b2.fingerprint().unwrap());
}
#[test]
fn operator_signature_and_address_key_binding_are_enforced() {
    let keys = SellerKeyPair::generate();
    let mut b = board(&keys);
    b.board.name = "tampered".into();
    assert!(b.validate(ZcashNetwork::Mainnet).is_err());
    let other = board(&keys);
    b.board.address = other.board.address;
    assert!(b.board.validate().is_err());
}
#[test]
fn memo_roundtrip_and_seller_signature() {
    let keys = SellerKeyPair::generate();
    let o = offer(&keys);
    let memo = encode_offer_memo(&o).unwrap();
    let mut decoded = decode_offer_memo(&memo).unwrap().unwrap();
    verify_offer(&decoded, Utc::now()).unwrap();
    assert_eq!(o, decoded);
    decoded.title = "tampered".into();
    assert!(verify_offer(&decoded, Utc::now()).is_err());
    decoded.signature = SellerKeyPair::generate().sign(b"wrong");
    assert!(verify_offer(&decoded, Utc::now()).is_err());
}
#[test]
fn authorized_policy_ttl_and_listing_value() {
    let operator = SellerKeyPair::generate();
    let seller = SellerKeyPair::generate();
    let mut b = board(&operator);
    b.board.policy.admission = Admission::Authorized;
    let o = offer(&seller);
    assert!(b.validate_offer(&o, 1000, Utc::now()).is_err());
    b.board.policy.authorized_sellers.push(seller.id.clone());
    b = b.board.sign(&operator).unwrap();
    b.validate_offer(&o, 1000, Utc::now()).unwrap();
    assert!(b.validate_offer(&o, 999, Utc::now()).is_err());
    b.board.policy.max_offer_ttl_seconds = 1;
    assert!(b.validate_offer(&o, 1000, Utc::now()).is_err());
    assert!(b.validate_offer(&o, 1000, o.expires_at).is_err());
}
#[test]
fn unrelated_malformed_and_unsupported_memos() {
    assert!(decode_offer_memo(&[0u8; 512]).unwrap().is_none());
    assert!(decode_offer_memo(b"hello").unwrap().is_none());
    assert!(decode_offer_memo(b"\xf5ZADP").is_err());
    let mut memo = encode_offer_memo(&offer(&SellerKeyPair::generate())).unwrap();
    memo[5] = 2;
    assert!(decode_offer_memo(&memo).is_err());
    memo[5] = 1;
    memo[6] = 255;
    assert!(decode_offer_memo(&memo).is_err());
    let mut o = offer(&SellerKeyPair::generate());
    o.zadp = 4;
    assert!(verify_offer(&o, Utc::now()).is_err());
}
#[test]
fn oversized_offer_and_fractional_zatoshi_are_rejected() {
    let mut o = offer(&SellerKeyPair::generate());
    o.description = Some("x".repeat(512));
    assert!(encode_offer_memo(&o).is_err());
    assert!(zec_to_zat(Decimal::new(1, 9)).is_err());
}

#[test]
fn memo_padding_truncation_and_trailing_cbor_are_rejected() {
    let original = encode_offer_memo(&offer(&SellerKeyPair::generate())).unwrap();
    let length = usize::from(u16::from_be_bytes([original[6], original[7]]));
    assert_eq!(original[8], 0x8e); // canonical wire-3 definite 14-field array
    assert!(original[8 + length..].iter().all(|byte| *byte == 0));
    for end in [5, 6, 7, 8, 64, 511] {
        assert!(decode_offer_memo(&original[..end]).is_err());
    }
    let mut padding = original;
    padding[511] = 1;
    assert!(decode_offer_memo(&padding).is_err());
    let mut trailing = original;
    trailing[6..8].copy_from_slice(&((length + 1) as u16).to_be_bytes());
    assert!(decode_offer_memo(&trailing).is_err());
    let mut malformed = original;
    malformed[8] = 0xff;
    assert!(decode_offer_memo(&malformed).is_err());
}

#[test]
fn exact_payload_boundary_and_multibyte_field_bounds() {
    let keys = SellerKeyPair::generate();
    let mut o = offer(&keys);
    let mut found = false;
    for n in 0..=504 {
        o.description = Some("x".repeat(n));
        if let Ok(memo) = encode_offer_memo(&o) {
            if u16::from_be_bytes([memo[6], memo[7]]) == 504 {
                let rebound =
                    bind_offer(o.clone(), "ai-services", o.issued_at.unwrap(), &keys).unwrap();
                verify_offer(
                    &decode_offer_memo(&encode_offer_memo(&rebound).unwrap())
                        .unwrap()
                        .unwrap(),
                    Utc::now(),
                )
                .unwrap();
                o.description = Some("x".repeat(n + 1));
                assert!(encode_offer_memo(&o).is_err());
                found = true;
                break;
            }
        }
    }
    assert!(found, "fixture must exercise exact 504-byte budget");
    o = offer(&keys);
    o.title = "😀".repeat(32);
    assert!(encode_offer_memo(&o).is_ok());
    o.title.push('a');
    assert!(encode_offer_memo(&o).is_err());
}

#[test]
fn nonce_lengths_precision_and_wrong_board_are_checked_at_the_right_boundary() {
    let keys = SellerKeyPair::generate();
    for length in [15, 16, 32, 33] {
        let mut o = offer(&keys);
        o.nonce = B64.encode(vec![7u8; length]);
        assert_eq!(encode_offer_memo(&o).is_ok(), (16..=32).contains(&length));
    }
    let mut o = offer(&keys);
    o.expires_at += Duration::nanoseconds(1);
    assert!(encode_offer_memo(&o).is_err());
    let o = bind_offer(
        offer(&keys),
        "another-board",
        chrono::DateTime::from_timestamp(Utc::now().timestamp() - 1, 0).unwrap(),
        &keys,
    )
    .unwrap();
    assert!(board(&keys).validate_offer(&o, 1000, Utc::now()).is_err());
    let other = SellerKeyPair::generate();
    assert!(bind_offer(offer(&keys), "ai-services", Utc::now(), &other).is_err());
}

#[test]
fn ninety_day_board_accepts_bounded_v1_v2_lifetimes_without_wire_changes() {
    let operator = SellerKeyPair::generate();
    let seller = SellerKeyPair::generate();
    let old = board(&operator);
    let mut body = old.board.clone();
    body.policy.max_offer_ttl_seconds = 7_776_000;
    let new = body.sign(&operator).unwrap();
    new.validate(ZcashNetwork::Mainnet).unwrap();
    assert_ne!(old.fingerprint().unwrap(), new.fingerprint().unwrap());
    assert_eq!(
        old.incoming_scope_fingerprint().unwrap(),
        new.incoming_scope_fingerprint().unwrap()
    );
    for version in [OFFER_VERSION_V1, OFFER_VERSION_V2] {
        for days in [30, 60, 90] {
            let mut o = offer(&seller);
            o.zadp = version;
            if version == OFFER_VERSION_V2 {
                o.commerce = None;
                o.price = None;
            }
            let issued = o.issued_at.unwrap();
            o.expires_at = issued + Duration::days(days);
            o.signature = seller.sign(&signing_bytes(&o).unwrap());
            assert!(old.validate_offer(&o, 1000, issued).is_err());
            new.validate_offer(&o, 1000, issued).unwrap();
            let decoded = decode_offer_memo(&encode_offer_memo(&o).unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(decoded, o);
            new.validate_offer(&o, 1000, o.expires_at - Duration::seconds(1))
                .unwrap();
            assert!(new.validate_offer(&o, 1000, o.expires_at).is_err());
            assert!(new
                .validate_offer(&o, 1000, o.expires_at + Duration::seconds(1))
                .is_err());
            o.expires_at = issued + Duration::days(90) + Duration::seconds(1);
            o.signature = seller.sign(&signing_bytes(&o).unwrap());
            assert!(new.validate_offer(&o, 1000, issued).is_err());
        }
    }
    let legacy = offer(&seller);
    old.validate_offer(&legacy, 1000, legacy.issued_at.unwrap())
        .unwrap();
    new.validate_offer(&legacy, 1000, legacy.issued_at.unwrap())
        .unwrap();
}

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::DateTime;
use zadp_protocol::*;
fn baseline() -> (Offer, SellerKeyPair) {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/offer-v1-golden.json")).unwrap();
    (
        serde_json::from_value(data["offer"].clone()).unwrap(),
        SellerKeyPair::from_secret_bytes(&[42; 32]),
    )
}
fn area() -> GeoArea {
    GeoArea {
        lat_e5: 3777493,
        lon_e5: -12241942,
        radius_m: 50000,
    }
}
fn geo_offer(geo: Option<GeoArea>) -> Offer {
    let (mut o, k) = baseline();
    o.zadp = OFFER_VERSION_V2;
    o.geo = geo;
    o.signature = k.sign(&signing_bytes(&o).unwrap());
    o
}
fn now() -> DateTime<chrono::Utc> {
    DateTime::from_timestamp(1791228001, 0).unwrap()
}
#[test]
fn captured_v1_bytes_json_and_signature_remain_exact() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/offer-v1-golden.json")).unwrap();
    let (offer, _) = baseline();
    assert_eq!(serde_json::to_value(&offer).unwrap(), data["offer"]);
    assert_eq!(
        B64.encode(signing_bytes(&offer).unwrap()),
        data["signing_b64"]
    );
    assert_eq!(
        B64.encode(encode_offer_memo(&offer).unwrap()),
        data["memo_b64"]
    );
    assert_eq!(
        decode_offer_memo(&B64.decode(data["memo_b64"].as_str().unwrap()).unwrap())
            .unwrap()
            .unwrap(),
        offer
    );
    verify_offer(&offer, now()).unwrap();
}
#[test]
fn geo_ranges_complete_integer_model() {
    for lat in [-9000000, 0, 9000000] {
        for lon in [-18000000, 0, 18000000] {
            for radius in [1, 500000] {
                GeoArea {
                    lat_e5: lat,
                    lon_e5: lon,
                    radius_m: radius,
                }
                .validate()
                .unwrap();
            }
        }
    }
    for g in [
        GeoArea {
            lat_e5: 9000001,
            ..area()
        },
        GeoArea {
            lat_e5: -9000001,
            ..area()
        },
        GeoArea {
            lon_e5: 18000001,
            ..area()
        },
        GeoArea {
            lon_e5: -18000001,
            ..area()
        },
        GeoArea {
            radius_m: 0,
            ..area()
        },
        GeoArea {
            radius_m: 500001,
            ..area()
        },
    ] {
        assert!(g.validate().is_err());
        let mut o = geo_offer(Some(area()));
        o.geo = Some(g);
        assert!(verify_offer(&o, now()).is_err());
        assert!(encode_offer_memo(&o).is_err());
    }
    for json in [
        r#"{"lat_e5":1,"radius_m":1}"#,
        r#"{"lat_e5":1,"lon_e5":2}"#,
        r#"{"lat_e5":1.5,"lon_e5":2,"radius_m":1}"#,
        r#"{"lat_e5":1,"lon_e5":2,"radius_m":-1}"#,
        r#"{"lat_e5":1,"lon_e5":2,"radius_m":1,"extra":0}"#,
    ] {
        assert!(serde_json::from_str::<GeoArea>(json).is_err());
    }
}
#[test]
fn signed_geo_dual_wire_roundtrip_and_tampering() {
    for geo in [None, Some(area())] {
        let offer = geo_offer(geo);
        let memo = encode_offer_memo(&offer).unwrap();
        assert_eq!(memo[5], 2);
        assert_eq!(memo[8], 0x8e);
        let decoded = decode_offer_memo(&memo).unwrap().unwrap();
        assert_eq!(decoded, offer);
        verify_offer(&decoded, now()).unwrap();
        assert!(signing_bytes(&offer)
            .unwrap()
            .starts_with(b"ZADP2\0offer\0"));
    }
    let o = geo_offer(Some(area()));
    for g in [
        GeoArea {
            lat_e5: 3777494,
            ..area()
        },
        GeoArea {
            lon_e5: -12241941,
            ..area()
        },
        GeoArea {
            radius_m: 50001,
            ..area()
        },
    ] {
        let mut changed = o.clone();
        changed.geo = Some(g);
        assert!(verify_offer(&changed, now()).is_err());
    }
    let mut o = geo_offer(Some(area()));
    o.zadp = 1;
    assert!(encode_offer_memo(&o).is_err());
    assert!(signing_bytes(&o).is_err());
    let mut memo = encode_offer_memo(&geo_offer(Some(area()))).unwrap();
    memo[5] = 1;
    assert!(decode_offer_memo(&memo).is_err());
    let mut json = serde_json::to_value(geo_offer(Some(area()))).unwrap();
    json["geo"].as_object_mut().unwrap().remove("radius_m");
    assert!(serde_json::from_value::<Offer>(json).is_err());
}
#[test]
fn exact_geo_budget_and_oversize_rejection() {
    let (base, k) = baseline();
    let plain = encode_offer_memo(&base).unwrap();
    let used = usize::from(u16::from_be_bytes([plain[6], plain[7]]));
    for (g, extra) in [
        (None, 1),
        (
            Some(GeoArea {
                lat_e5: -9000000,
                lon_e5: -18000000,
                radius_m: 500000,
            }),
            16,
        ),
    ] {
        let mut o = geo_offer(g);
        let m = encode_offer_memo(&o).unwrap();
        assert_eq!(usize::from(u16::from_be_bytes([m[6], m[7]])), used + extra);
        let mut boundary = false;
        for n in 0..504 {
            o.description = Some("x".repeat(n));
            o.signature = k.sign(&signing_bytes(&o).unwrap());
            if let Ok(m) = encode_offer_memo(&o) {
                if u16::from_be_bytes([m[6], m[7]]) == 504 {
                    boundary = true;
                    o.description.as_mut().unwrap().push('x');
                    o.signature = k.sign(&signing_bytes(&o).unwrap());
                    assert!(encode_offer_memo(&o).is_err());
                    break;
                }
            }
        }
        assert!(boundary);
    }
}
#[test]
fn geo_draft_selects_v2_and_absent_geo_keeps_v1() {
    let (o, k) = baseline();
    let draft = OfferDraft::new_ulid(
        k.id.clone(),
        "cleaning",
        "Cabin cleaning",
        o.price.clone(),
        o.endpoint.clone(),
        o.expires_at,
        o.nonce.clone(),
    );
    assert_eq!(draft.clone().sign(&k).unwrap().zadp, 1);
    assert_eq!(draft.with_geo(area()).sign(&k).unwrap().zadp, 2);
}

#[test]
fn malformed_wire_geo_is_rejected_before_signature_acceptance() {
    use ciborium::value::Value;
    let memo = encode_offer_memo(&geo_offer(Some(area()))).unwrap();
    let len = u16::from_be_bytes([memo[6], memo[7]]) as usize;
    let value: Value = ciborium::from_reader(&memo[8..8 + len]).unwrap();
    let Value::Array(fields) = value else {
        panic!("tuple expected")
    };
    for invalid in [
        Value::Array(vec![Value::Integer(1.into()), Value::Integer(2.into())]),
        Value::Array(vec![
            Value::Integer(9000001.into()),
            Value::Integer(0.into()),
            Value::Integer(50000.into()),
        ]),
        Value::Array(vec![
            Value::Integer(0.into()),
            Value::Integer(18000001.into()),
            Value::Integer(50000.into()),
        ]),
        Value::Array(vec![
            Value::Integer(0.into()),
            Value::Integer(0.into()),
            Value::Integer(0.into()),
        ]),
        Value::Array(vec![
            Value::Float(1.5),
            Value::Integer(0.into()),
            Value::Integer(50000.into()),
        ]),
    ] {
        let mut modified = fields.clone();
        modified[13] = invalid;
        let mut bytes = vec![];
        ciborium::into_writer(&Value::Array(modified), &mut bytes).unwrap();
        let mut framed = [0u8; 512];
        framed[..6].copy_from_slice(&memo[..6]);
        framed[6..8].copy_from_slice(&(bytes.len() as u16).to_be_bytes());
        framed[8..8 + bytes.len()].copy_from_slice(&bytes);
        assert!(decode_offer_memo(&framed).is_err());
    }
}

#[test]
fn legacy_field_limits_stay_available_with_max_geo_and_total_budget_still_applies() {
    let (_, k) = baseline();
    let base = geo_offer(Some(GeoArea {
        lat_e5: 9000000,
        lon_e5: -18000000,
        radius_m: 500000,
    }));
    for field in ["id", "board", "capability", "title", "nonce"] {
        let mut offer = base.clone();
        match field {
            "id" => offer.id = "i".repeat(64),
            "board" => offer.board_id = Some("b".repeat(64)),
            "capability" => offer.capability = "c".repeat(64),
            "title" => offer.title = "t".repeat(128),
            "nonce" => offer.nonce = B64.encode([7; 32]),
            _ => unreachable!(),
        }
        offer.signature = k.sign(&signing_bytes(&offer).unwrap());
        encode_offer_memo(&offer).unwrap();
        match field {
            "id" => offer.id.push('i'),
            "board" => offer.board_id.as_mut().unwrap().push('b'),
            "capability" => offer.capability.push('c'),
            "title" => offer.title.push('t'),
            "nonce" => offer.nonce = B64.encode([7; 33]),
            _ => unreachable!(),
        }
        offer.signature = k.sign(&signing_bytes(&offer).unwrap());
        assert!(encode_offer_memo(&offer).is_err());
    }
    let mut offer = base;
    offer.id = "i".repeat(64);
    offer.board_id = Some("b".repeat(64));
    offer.capability = "c".repeat(64);
    offer.title = "t".repeat(128);
    offer.nonce = B64.encode([7; 32]);
    offer.signature = k.sign(&signing_bytes(&offer).unwrap());
    assert!(
        encode_offer_memo(&offer).is_err(),
        "independent field maxima do not waive total 504-byte budget"
    );
}

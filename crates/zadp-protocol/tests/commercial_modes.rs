use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration};
use zadp_protocol::*;
fn hex(b: impl AsRef<[u8]>) -> String {
    b.as_ref().iter().map(|x| format!("{x:02x}")).collect()
}
fn sample(mode: usize, geo: bool) -> (Offer, SellerKeyPair) {
    let k = SellerKeyPair::from_secret_bytes(&[42; 32]);
    let now = DateTime::from_timestamp(1893456000, 0).unwrap();
    let mut d = OfferDraft::new_discovery(
        k.id.clone(),
        "local.service",
        "Bike repair",
        now + Duration::days(30),
        B64.encode([1; 16]),
    )
    .with_version(3);
    if mode > 0 {
        d.price = Some(Price::zec("0.02".parse().unwrap()));
    }
    if mode == 2 {
        d.commerce = Some(Commerce {
            endpoint: "https://seller.example".into(),
        });
    }
    if geo {
        d = d.with_geo(GeoArea {
            lat_e5: 3777500,
            lon_e5: -12241900,
            radius_m: 25000,
        });
    }
    (
        bind_offer(d.sign(&k).unwrap(), "local-services", now, &k).unwrap(),
        k,
    )
}
#[test]
fn all_three_modes_roundtrip_without_protocol_migration() {
    for mode in 0..3 {
        for geo in [false, true] {
            let (o, _) = sample(mode, geo);
            verify_offer(&o, o.issued_at.unwrap()).unwrap();
            assert_eq!(
                o.purchase_mode(),
                [
                    PurchaseMode::Discovery,
                    PurchaseMode::Manual,
                    PurchaseMode::Automated
                ][mode]
            );
            let json = serde_json::to_value(&o).unwrap();
            assert_eq!(json.get("price").is_some(), mode > 0);
            assert_eq!(json.get("commerce").is_some(), mode == 2);
            if mode == 2 {
                assert!(json["commerce"].get("price").is_none());
            }
            assert_eq!(serde_json::from_value::<Offer>(json).unwrap(), o);
            let memo = encode_offer_memo(&o).unwrap();
            assert_eq!(memo[5], 3);
            assert_eq!(memo[8], 0x8e);
            assert_eq!(decode_offer_memo(&memo).unwrap().unwrap(), o);
            assert!(signing_bytes(&o).unwrap().starts_with(b"ZADP3\0offer\0"));
        }
    }
}
#[test]
fn legacy_and_actual_mainnet_geo_bytes_are_unchanged() {
    let mut fixtures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/offer-legacy-commercial-golden.json")).unwrap();
    fixtures
        .push(serde_json::from_str(include_str!("fixtures/offer-v2-mainnet-golden.json")).unwrap());
    for f in fixtures {
        let o: Offer = serde_json::from_value(f["offer"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&o).unwrap(), f["offer"]);
        assert_eq!(hex(signing_bytes(&o).unwrap()), f["signing_hex"]);
        let memo = encode_offer_memo(&o).unwrap();
        assert_eq!(hex(memo), f["memo_hex"]);
        assert_eq!(decode_offer_memo(&memo).unwrap().unwrap(), o);
        verify_offer(&o, o.issued_at.unwrap()).unwrap();
    }
}
#[test]
fn price_endpoint_geo_and_absence_are_signed() {
    let (o, _) = sample(2, true);
    let mut changes = vec![];
    let mut p = o.clone();
    p.price.as_mut().unwrap().amount = "0.03".parse().unwrap();
    changes.push(p);
    let mut p = o.clone();
    p.commerce = None;
    changes.push(p);
    let mut p = o.clone();
    p.commerce.as_mut().unwrap().endpoint = "https://other.example".into();
    changes.push(p);
    for field in 0..3 {
        let mut p = o.clone();
        let g = p.geo.as_mut().unwrap();
        match field {
            0 => g.lat_e5 += 1,
            1 => g.lon_e5 += 1,
            _ => g.radius_m += 1,
        };
        changes.push(p);
    }
    for p in changes {
        assert!(verify_offer(&p, o.issued_at.unwrap()).is_err());
    }
    let (o, _) = sample(1, false);
    let mut p = o.clone();
    p.price = None;
    assert!(verify_offer(&p, o.issued_at.unwrap()).is_err());
}
#[test]
fn invalid_shapes_prices_and_version_mixing_fail_closed() {
    let (o, k) = sample(2, true);
    let now = o.issued_at.unwrap();
    let mut p = o.clone();
    p.price = None;
    assert!(verify_offer(&p, now).is_err());
    for amount in ["0", "-1", "0.000000001", "21000000.00000001"] {
        let mut p = o.clone();
        p.price.as_mut().unwrap().amount = amount.parse().unwrap();
        assert!(verify_offer(&p, now).is_err());
    }
    let mut p = o.clone();
    p.price.as_mut().unwrap().currency = "USD".into();
    assert!(verify_offer(&p, now).is_err());
    for endpoint in [
        "",
        "bad",
        "http://seller.example",
        "https://user:password@seller.example",
    ] {
        let mut p = o.clone();
        p.commerce.as_mut().unwrap().endpoint = endpoint.into();
        p.signature = k.sign(&signing_bytes(&p).unwrap());
        assert!(verify_offer(&p, now).is_err());
    }
    let mut json = serde_json::to_value(&o).unwrap();
    json["commerce"]["price"] = json["price"].clone();
    assert!(serde_json::from_value::<Offer>(json).is_err());
    let mut json = serde_json::to_value(&o).unwrap();
    json["zadp"] = serde_json::json!(2);
    assert!(serde_json::from_value::<Offer>(json).is_err());
    let mut memo = encode_offer_memo(&o).unwrap();
    memo[5] = 2;
    assert!(decode_offer_memo(&memo).is_err());
    let (mut manual, _) = sample(1, false);
    manual.zadp = 2;
    assert!(encode_offer_memo(&manual).is_err());
}
#[test]
fn all_modes_and_geo_combinations_enforce_exact_504_byte_payload() {
    for mode in 0..3 {
        for geo in [false, true] {
            let (base, k) = sample(mode, geo);
            let mut at_limit = None;
            for len in 0..505 {
                let mut o = base.clone();
                o.description = Some("x".repeat(len));
                o.signature = k.sign(&signing_bytes(&o).unwrap());
                if let Ok(memo) = encode_offer_memo(&o) {
                    let size = u16::from_be_bytes([memo[6], memo[7]]);
                    assert!(size <= 504);
                    if size == 504 {
                        at_limit = Some(o);
                        break;
                    }
                }
            }
            let mut o = at_limit.expect("exact boundary fixture");
            assert_eq!(encode_offer_memo(&o).unwrap().len(), 512);
            o.description.as_mut().unwrap().push('x');
            o.signature = k.sign(&signing_bytes(&o).unwrap());
            assert!(encode_offer_memo(&o).is_err());
        }
    }
}

#[test]
fn protocol_1_defaults_only_to_wire3_and_rejects_legacy_publication() {
    assert_eq!(PROTOCOL_RELEASE, "1.0");
    assert_eq!(PROTOCOL_VERSION, 3);
    let (_, k) = sample(0, false);
    let now = DateTime::from_timestamp(1893456000, 0).unwrap();
    for geo in [false, true] {
        for mode in 0..3 {
            let mut d = OfferDraft::new_discovery(
                k.id.clone(),
                "local.service",
                "Canonical",
                now + Duration::days(30),
                B64.encode([1; 16]),
            );
            if mode > 0 {
                d.price = Some(Price::zec("0.02".parse().unwrap()));
            }
            if mode == 2 {
                d.commerce = Some(Commerce {
                    endpoint: "https://seller.example".into(),
                });
            }
            if geo {
                d.geo = Some(GeoArea {
                    lat_e5: 3777500,
                    lon_e5: -12241900,
                    radius_m: 25000,
                });
            }
            assert_eq!(d.clone().sign(&k).unwrap().zadp, 3);
            for legacy in [1, 2] {
                assert!(d.clone().with_version(legacy).sign(&k).is_err());
            }
            let o = bind_offer(d.sign(&k).unwrap(), "local-services", now, &k).unwrap();
            verify_offer_for_publication(&o, now).unwrap();
            assert_eq!(encode_offer_memo(&o).unwrap()[5], 3);
        }
    }
    let fixtures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/offer-legacy-commercial-golden.json")).unwrap();
    for value in fixtures {
        let o: Offer = serde_json::from_value(value["offer"].clone()).unwrap();
        let now = o.issued_at.unwrap();
        verify_offer(&o, now).unwrap();
        assert!(verify_offer_for_publication(&o, now).is_err());
        assert_eq!(hex(encode_offer_memo(&o).unwrap()), value["memo_hex"]);
    }
}

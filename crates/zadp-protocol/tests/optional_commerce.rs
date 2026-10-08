use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use zadp_protocol::*;
fn offer(commerce: bool, geo: bool) -> (Offer, SellerKeyPair) {
    let k = SellerKeyPair::generate();
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    let mut d = OfferDraft::new_ulid(
        k.id.clone(),
        "local.service",
        "Bike repair — San Francisco area",
        Price::zec(Decimal::new(1, 2)),
        "https://seller.example",
        now + Duration::days(1),
        B64.encode([1; 16]),
    );
    if !commerce {
        d = d.discovery_only();
    }
    if geo {
        d = d.with_geo(GeoArea {
            lat_e5: 3777500,
            lon_e5: -12241900,
            radius_m: 25000,
        });
    }
    let mut o = d.sign(&k).unwrap();
    o.zadp = 2;
    o.signature = k.sign(&signing_bytes(&o).unwrap());
    (bind_offer(o, "ai-services", now, &k).unwrap(), k)
}
#[test]
fn optional_commerce_roundtrip_and_exact_wire_state() {
    for commerce in [false, true] {
        for geo in [false, true] {
            let (o, _) = offer(commerce, geo);
            verify_offer(&o, Utc::now()).unwrap();
            let memo = encode_offer_memo(&o).unwrap();
            assert_eq!(memo[5], 2);
            assert_eq!(memo[8], 0x8d);
            let len = u16::from_be_bytes([memo[6], memo[7]]) as usize;
            let ciborium::value::Value::Array(fields) =
                ciborium::from_reader(&memo[8..8 + len]).unwrap()
            else {
                panic!()
            };
            assert_eq!(fields.len(), 13);
            assert_eq!(fields[7] == ciborium::value::Value::Null, !commerce);
            assert_eq!(decode_offer_memo(&memo).unwrap().unwrap(), o);
            let json = serde_json::to_value(&o).unwrap();
            assert!(json.get("endpoint").is_none());
            assert!(json.get("price").is_none());
            assert_eq!(json.get("commerce").is_some(), commerce);
            assert_eq!(serde_json::from_value::<Offer>(json).unwrap(), o);
        }
    }
}
#[test]
fn commerce_presence_endpoint_price_are_signed() {
    let (o, k) = offer(false, true);
    let complete = Commerce {
        endpoint: "https://seller.example".into(),
        price: Price::zec(Decimal::new(1, 2)),
    };
    let mut changed = o.clone();
    changed.commerce = Some(complete);
    assert!(verify_offer(&changed, Utc::now()).is_err());
    changed.signature = k.sign(&signing_bytes(&changed).unwrap());
    verify_offer(&changed, Utc::now()).unwrap();
    let signed = changed.clone();
    changed.commerce = None;
    assert!(verify_offer(&changed, Utc::now()).is_err());
    for price in [false, true] {
        let mut changed = signed.clone();
        let c = changed.commerce.as_mut().unwrap();
        if price {
            c.price.amount = Decimal::new(2, 2);
        } else {
            c.endpoint = "https://other.example".into();
        }
        assert!(verify_offer(&changed, Utc::now()).is_err());
    }
}
#[test]
fn partial_invalid_commerce_rejected_no_price_required_for_discovery() {
    let (o, k) = offer(false, true);
    let json = serde_json::to_value(&o).unwrap();
    for block in [
        serde_json::json!({"endpoint":"https://seller.example"}),
        serde_json::json!({"price":{"amount":"0.01","currency":"ZEC"}}),
    ] {
        let mut j = json.clone();
        j["commerce"] = block;
        assert!(serde_json::from_value::<Offer>(j).is_err());
    }
    let mut mixed = json.clone();
    mixed["endpoint"] = serde_json::json!("https://seller.example");
    assert!(serde_json::from_value::<Offer>(mixed).is_err());
    for endpoint in [
        "",
        "http://seller.example",
        "https://user:secret@seller.example",
        "bad",
    ] {
        let mut x = o.clone();
        x.commerce = Some(Commerce {
            endpoint: endpoint.into(),
            price: Price::zec(Decimal::new(1, 2)),
        });
        x.signature = k.sign(&signing_bytes(&x).unwrap());
        assert!(verify_offer(&x, Utc::now()).is_err());
    }
    let mut x = o.clone();
    x.commerce = Some(Commerce {
        endpoint: "https://seller.example".into(),
        price: Price::zec(Decimal::ZERO),
    });
    x.signature = k.sign(&signing_bytes(&x).unwrap());
    assert!(verify_offer(&x, Utc::now()).is_err());
    verify_offer(&o, Utc::now()).unwrap();
}

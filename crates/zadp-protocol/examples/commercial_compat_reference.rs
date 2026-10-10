//! Offline compatibility oracle: public signed fixtures only; no wallet or network.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration};
use zadp_protocol::*;
fn hex(b: impl AsRef<[u8]>) -> String {
    b.as_ref().iter().map(|x| format!("{x:02x}")).collect()
}
fn record(o: Offer) -> serde_json::Value {
    serde_json::json!({"offer":o,"memo_hex":hex(encode_offer_memo(&o).unwrap()),"signing_hex":hex(signing_bytes(&o).unwrap())})
}
fn main() {
    if let Some(path) = std::env::args().nth(1) {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        println!(
            "{}",
            record(serde_json::from_value(v["offer"].clone()).unwrap())
        );
        return;
    }
    let k = SellerKeyPair::from_secret_bytes(&[42; 32]);
    let now = DateTime::from_timestamp(1893456000, 0).unwrap();
    let mut records = vec![];
    for commerce in [false, true] {
        for geo in [false, true] {
            let mut d = OfferDraft::new_ulid(
                k.id.clone(),
                "local.service",
                "Bike repair",
                Price::zec("0.02".parse().unwrap()),
                "https://seller.example",
                now + Duration::days(30),
                B64.encode([1; 16]),
            );
            d.id = "01ARZ3NDEKTSV4RRFFQ69G5FAV".into();
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
            records.push(record(
                bind_offer(d.sign(&k).unwrap(), "local-services", now, &k).unwrap(),
            ));
        }
    }
    println!("{}", serde_json::to_string_pretty(&records).unwrap());
}

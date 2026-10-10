//! Offline compatibility oracle: public signed fixtures only; no wallet or network.
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
    let records: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../tests/fixtures/offer-legacy-commercial-golden.json"
    ))
    .unwrap();
    let records: Vec<_> = records
        .into_iter()
        .map(|v| record(serde_json::from_value(v["offer"].clone()).unwrap()))
        .collect();
    println!("{}", serde_json::to_string_pretty(&records).unwrap());
}

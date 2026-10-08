//! Read-only verification of a public prepared offer; never accesses identity or wallet files.
use chrono::Utc;
use zadp_protocol::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: verify_public_offer OFFER_JSON BOARD_JSON".into());
    }
    let o: Offer = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let b: BoardDescriptor = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    b.validate(ZcashNetwork::Mainnet)?;
    b.validate_offer(&o, b.board.policy.min_listing_value_zat, Utc::now())?;
    verify_offer(&o, Utc::now())?;
    let memo = encode_offer_memo(&o)?;
    let mut geo_mutations_rejected = 0;
    if let Some(g) = o.geo {
        for mutated in [
            GeoArea {
                lat_e5: g.lat_e5 + 1,
                ..g
            },
            GeoArea {
                lon_e5: g.lon_e5 + 1,
                ..g
            },
            GeoArea {
                radius_m: g.radius_m + 1,
                ..g
            },
        ] {
            let mut changed = o.clone();
            changed.geo = Some(mutated);
            if verify_offer(&changed, Utc::now()).is_ok() {
                return Err("geo mutation unexpectedly verified".into());
            }
            geo_mutations_rejected += 1;
        }
    }
    println!(
        "{}",
        serde_json::json!({"offer_id":o.id,"network":"mainnet","board_fingerprint":b.fingerprint()?,"offer_version":o.zadp,"wire_version":memo[5],"payload_bytes":u16::from_be_bytes([memo[6],memo[7]]),"framed_bytes":memo.len(),"signature_valid":true,"board_policy_accepted":true,"commerce_present":o.commerce.is_some(),"geo_mutations_rejected":geo_mutations_rejected,"wallet_access":false,"broadcast":false})
    );
    Ok(())
}

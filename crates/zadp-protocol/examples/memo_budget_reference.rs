//! Offline size oracle for UI parity tests. Uses ephemeral Ed25519 identities,
//! never wallet credentials, Zcash signing, network access or publication.
use std::io::{self, BufRead};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration};
use rust_decimal::Decimal;
use serde::Deserialize;
use zadp_protocol::{bind_offer, encode_offer_memo, OfferDraft, Price, SellerKeyPair};

#[derive(Deserialize)]
struct Draft {
    board_id: String,
    title: String,
    capability: String,
    endpoint: Option<String>,
    price: Option<serde_json::Value>,
    commerce: Option<serde_json::Value>,
    offer_version: Option<u16>,
    description: Option<String>,
    issued_at: i64,
    ttl_seconds: Option<u32>,
    geo: Option<zadp_protocol::GeoArea>,
}

fn encoded_size(draft: Draft) -> Result<usize, Box<dyn std::error::Error>> {
    let issued = DateTime::from_timestamp(draft.issued_at, 0).ok_or("invalid issue time")?;
    let keys = SellerKeyPair::generate();
    let endpoint = draft
        .commerce
        .as_ref()
        .and_then(|c| c["endpoint"].as_str())
        .map(String::from)
        .or(draft.endpoint);
    let amount = draft
        .commerce
        .as_ref()
        .and_then(|c| c["price"]["amount"].as_str())
        .map(String::from)
        .or_else(|| {
            draft
                .price
                .as_ref()
                .and_then(|p| p.as_str().or_else(|| p["amount"].as_str()))
                .map(String::from)
        });
    let expiry = issued + Duration::seconds(i64::from(draft.ttl_seconds.unwrap_or(86_400)));
    let mut offer = match (amount, endpoint) {
        (Some(a), Some(e)) => OfferDraft::new_ulid(
            keys.id.clone(),
            draft.capability,
            draft.title,
            Price::zec(a.parse::<Decimal>()?),
            e,
            expiry,
            B64.encode([0; 16]),
        ),
        (Some(a), None) => OfferDraft::new_discovery(
            keys.id.clone(),
            draft.capability,
            draft.title,
            expiry,
            B64.encode([0; 16]),
        )
        .with_price(Price::zec(a.parse::<Decimal>()?)),
        (None, None) => OfferDraft::new_discovery(
            keys.id.clone(),
            draft.capability,
            draft.title,
            expiry,
            B64.encode([0; 16]),
        ),
        _ => return Err("endpoint requires price".into()),
    };
    if let Some(v) = draft.offer_version {
        offer = offer.with_version(v);
    }
    if let Some(geo) = draft.geo {
        offer = offer.with_geo(geo);
    }
    offer.description = draft.description.filter(|s| !s.is_empty());
    let offer = bind_offer(offer.sign(&keys)?, &draft.board_id, issued, &keys)?;
    let memo = encode_offer_memo(&offer)?;
    Ok(u16::from_be_bytes([memo[6], memo[7]]) as usize)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for line in io::stdin().lock().lines() {
        let result = encoded_size(serde_json::from_str(&line?)?);
        let output = match result {
            Ok(used) => serde_json::json!({"fits": true, "used": used}),
            Err(_) => serde_json::json!({"fits": false}),
        };
        println!("{output}");
    }
    Ok(())
}

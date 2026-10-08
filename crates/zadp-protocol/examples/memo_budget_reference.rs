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
    price: Option<String>,
    commerce: Option<zadp_protocol::Commerce>,
    description: Option<String>,
    issued_at: i64,
    ttl_seconds: Option<u32>,
    geo: Option<zadp_protocol::GeoArea>,
}

fn encoded_size(draft: Draft) -> Result<usize, Box<dyn std::error::Error>> {
    let issued = DateTime::from_timestamp(draft.issued_at, 0).ok_or("invalid issue time")?;
    let keys = SellerKeyPair::generate();
    let commerce = draft.commerce.or_else(|| {
        draft.endpoint.map(|endpoint| zadp_protocol::Commerce {
            endpoint,
            price: Price::zec(
                draft
                    .price
                    .as_deref()
                    .unwrap_or("0")
                    .parse::<Decimal>()
                    .unwrap_or_default(),
            ),
        })
    });
    let mut offer = match commerce {
        Some(c) => OfferDraft::new_ulid(
            keys.id.clone(),
            draft.capability,
            draft.title,
            c.price,
            c.endpoint,
            issued + Duration::seconds(i64::from(draft.ttl_seconds.unwrap_or(86_400))),
            B64.encode([0u8; 16]),
        ),
        None => OfferDraft::new_discovery(
            keys.id.clone(),
            draft.capability,
            draft.title,
            issued + Duration::seconds(i64::from(draft.ttl_seconds.unwrap_or(86_400))),
            B64.encode([0u8; 16]),
        ),
    };
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

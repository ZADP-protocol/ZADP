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
    endpoint: String,
    price: String,
    description: Option<String>,
    issued_at: i64,
}

fn encoded_size(draft: Draft) -> Result<usize, Box<dyn std::error::Error>> {
    let issued = DateTime::from_timestamp(draft.issued_at, 0).ok_or("invalid issue time")?;
    let keys = SellerKeyPair::generate();
    let mut offer = OfferDraft::new_ulid(
        keys.id.clone(),
        draft.capability,
        draft.title,
        Price::zec(draft.price.parse::<Decimal>()?),
        draft.endpoint,
        issued + Duration::days(1),
        B64.encode([0u8; 16]),
    );
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

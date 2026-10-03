//! Binary memo framing: 0xf5 (ZIP 302 arbitrary data), ZADP, wire version, u16 length, CBOR array.
use crate::{Offer, Price, ProtocolError, SellerId, SellerKeyPair, Signature};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Utc};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_bytes::ByteBuf;
use std::io::Cursor;

const MAGIC: &[u8] = b"\xf5ZADP";
const WIRE_VERSION: u8 = 1;
type WireOffer = (
    u16,
    String,
    String,
    ByteBuf,
    String,
    String,
    Option<String>,
    u64,
    String,
    i64,
    i64,
    ByteBuf,
    ByteBuf,
);

fn invalid(reason: &str) -> ProtocolError {
    ProtocolError::InvalidOffer(reason.into())
}
pub fn zec_to_zat(amount: Decimal) -> Result<u64, ProtocolError> {
    let scaled = amount
        .checked_mul(Decimal::from(100_000_000u64))
        .ok_or_else(|| invalid("amount overflow"))?;
    if scaled.fract() != Decimal::ZERO {
        return Err(invalid("ZEC precision exceeds 8 decimals"));
    }
    scaled
        .to_u64()
        .filter(|v| *v > 0 && *v <= 2_100_000_000_000_000)
        .ok_or_else(|| invalid("invalid ZEC amount"))
}

/// Bind a draft to a board and issue time before transport encoding. Re-sign all fields.
pub fn bind_offer(
    mut offer: Offer,
    board_id: &str,
    issued_at: DateTime<Utc>,
    keys: &SellerKeyPair,
) -> Result<Offer, ProtocolError> {
    if offer.seller != keys.id {
        return Err(invalid("seller key mismatch"));
    }
    offer.board_id = Some(board_id.into());
    offer.issued_at = Some(issued_at);
    offer.price.amount = offer.price.amount.normalize();
    offer.signature = keys.sign(&crate::signing_bytes(&offer)?);
    Ok(offer)
}
pub fn encode_offer_memo(offer: &Offer) -> Result<[u8; 512], ProtocolError> {
    let issued = offer
        .issued_at
        .ok_or_else(|| invalid("missing issue time"))?;
    if issued.timestamp_subsec_nanos() != 0 || offer.expires_at.timestamp_subsec_nanos() != 0 {
        return Err(invalid("memo timestamps require whole seconds"));
    }
    if offer.message_type != "offer" {
        return Err(ProtocolError::UnsupportedType(offer.message_type.clone()));
    }
    if offer.price.currency != "ZEC" || offer.price.amount != offer.price.amount.normalize() {
        return Err(invalid("invalid price"));
    }
    let wire: WireOffer = (
        offer.zadp,
        offer
            .board_id
            .clone()
            .ok_or_else(|| invalid("missing board binding"))?,
        offer.id.clone(),
        ByteBuf::from(offer.seller.verifying_key()?.to_bytes().to_vec()),
        offer.capability.clone(),
        offer.title.clone(),
        offer.description.clone(),
        zec_to_zat(offer.price.amount)?,
        offer.endpoint.clone(),
        issued.timestamp(),
        offer.expires_at.timestamp(),
        ByteBuf::from(
            B64.decode(&offer.nonce)
                .map_err(|_| invalid("invalid nonce"))?,
        ),
        ByteBuf::from(offer.signature.to_ed25519()?.to_bytes().to_vec()),
    );
    let mut payload = vec![];
    ciborium::ser::into_writer(&wire, &mut payload)
        .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
    if payload.len() > 504 {
        return Err(invalid(
            "offer exceeds 512-byte memo budget; shorten title, endpoint or description",
        ));
    }
    let mut memo = [0u8; 512];
    memo[..5].copy_from_slice(MAGIC);
    memo[5] = WIRE_VERSION;
    memo[6..8].copy_from_slice(&(payload.len() as u16).to_be_bytes());
    memo[8..8 + payload.len()].copy_from_slice(&payload);
    // Decode and verify preserves all signed fields, including decimal and time normalization.
    let roundtrip = decode_offer_memo_inner(&memo)?;
    if roundtrip != *offer {
        return Err(invalid("offer is not normalized for memo transport"));
    }
    Ok(memo)
}
pub fn decode_offer_memo(memo: &[u8]) -> Result<Option<Offer>, ProtocolError> {
    if !memo.starts_with(MAGIC) {
        return Ok(None);
    }
    if memo.len() != 512 || memo[5] != WIRE_VERSION {
        return Err(invalid("unsupported or malformed ZADP memo framing"));
    }
    decode_offer_memo_inner(memo).map(Some)
}
fn decode_offer_memo_inner(memo: &[u8]) -> Result<Offer, ProtocolError> {
    let len = usize::from(u16::from_be_bytes([memo[6], memo[7]]));
    if len == 0 || len > 504 || memo[8 + len..].iter().any(|b| *b != 0) {
        return Err(invalid("invalid memo length or padding"));
    }
    let mut reader = Cursor::new(&memo[8..8 + len]);
    let (
        zadp,
        board_id,
        id,
        seller,
        capability,
        title,
        description,
        amount,
        endpoint,
        issued,
        expiry,
        nonce,
        sig,
    ): WireOffer =
        ciborium::de::from_reader(&mut reader).map_err(|_| invalid("malformed ZADP CBOR"))?;
    if reader.position() != len as u64
        || seller.len() != 32
        || sig.len() != 64
        || nonce.len() < 16
        || nonce.len() > 32
    {
        return Err(invalid("invalid memo field lengths"));
    }
    if id.is_empty()
        || id.len() > 64
        || capability.is_empty()
        || capability.len() > 64
        || title.is_empty()
        || title.len() > 128
        || board_id.len() > 64
    {
        return Err(invalid("invalid offer field bounds"));
    }
    let seller: SellerId =
        serde_json::from_value(serde_json::json!(format!("ed25519:{}", B64.encode(seller))))
            .map_err(|_| invalid("invalid seller"))?;
    let signature: Signature = serde_json::from_value(serde_json::json!(B64.encode(sig)))
        .map_err(|_| invalid("invalid signature"))?;
    Ok(Offer {
        zadp,
        message_type: "offer".into(),
        id,
        seller,
        capability,
        title,
        description,
        price: Price::zec(Decimal::from(amount) / Decimal::from(100_000_000u64)),
        endpoint,
        expires_at: DateTime::from_timestamp(expiry, 0).ok_or_else(|| invalid("invalid expiry"))?,
        issued_at: Some(
            DateTime::from_timestamp(issued, 0).ok_or_else(|| invalid("invalid issued_at"))?,
        ),
        board_id: Some(board_id),
        nonce: B64.encode(nonce),
        signature,
    })
}

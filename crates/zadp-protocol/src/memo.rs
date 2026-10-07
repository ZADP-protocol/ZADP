//! Binary memo framing: 0xf5 (ZIP 302 arbitrary data), ZADP, wire version, u16 length, CBOR array.
use crate::{Offer, Price, ProtocolError, SellerId, SellerKeyPair, Signature};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Utc};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_bytes::ByteBuf;
use std::io::Cursor;

const MAGIC: &[u8] = b"\xf5ZADP";
pub const WIRE_VERSION_V1: u8 = 1;
pub const WIRE_VERSION_V2: u8 = 2;
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
    offer.validate_geo_version()?;
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
    if offer.zadp == crate::OFFER_VERSION_V2 {
        let ciborium::value::Value::Array(mut fields) =
            ciborium::de::from_reader(payload.as_slice())
                .map_err(|_| invalid("invalid encoder tuple"))?
        else {
            return Err(invalid("invalid encoder tuple"));
        };
        let geo = match offer.geo {
            Some(g) => ciborium::value::Value::Array(vec![
                ciborium::value::Value::Integer(g.lat_e5.into()),
                ciborium::value::Value::Integer(g.lon_e5.into()),
                ciborium::value::Value::Integer(g.radius_m.into()),
            ]),
            None => ciborium::value::Value::Null,
        };
        fields.push(geo);
        payload.clear();
        ciborium::ser::into_writer(&ciborium::value::Value::Array(fields), &mut payload)
            .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
    }
    if payload.len() > 504 {
        return Err(invalid(
            "offer exceeds 512-byte memo budget; shorten title, endpoint or description",
        ));
    }
    let mut memo = [0u8; 512];
    memo[..5].copy_from_slice(MAGIC);
    memo[5] = if offer.zadp == crate::OFFER_VERSION_V2 {
        WIRE_VERSION_V2
    } else {
        WIRE_VERSION_V1
    };
    memo[6..8].copy_from_slice(&(payload.len() as u16).to_be_bytes());
    memo[8..8 + payload.len()].copy_from_slice(&payload);
    // Decode and verify preserves all signed fields, including decimal and time normalization.
    let roundtrip = decode_offer_memo_inner(&memo)?;
    if roundtrip != *offer || crate::signing_bytes(&roundtrip)? != crate::signing_bytes(offer)? {
        return Err(invalid("offer is not normalized for memo transport"));
    }
    Ok(memo)
}
pub fn decode_offer_memo(memo: &[u8]) -> Result<Option<Offer>, ProtocolError> {
    if !memo.starts_with(MAGIC) {
        return Ok(None);
    }
    if memo.len() != 512 || ![WIRE_VERSION_V1, WIRE_VERSION_V2].contains(&memo[5]) {
        return Err(invalid("unsupported or malformed ZADP memo framing"));
    }
    decode_offer_memo_inner(memo).map(Some)
}
fn decode_offer_memo_inner(memo: &[u8]) -> Result<Offer, ProtocolError> {
    let len = usize::from(u16::from_be_bytes([memo[6], memo[7]]));
    if len == 0 || len > 504 || memo[8 + len..].iter().any(|b| *b != 0) {
        return Err(invalid("invalid memo length or padding"));
    }
    // Keep the original typed v1 decoding path over the original payload bytes.
    let (legacy, geo) = if memo[5] == WIRE_VERSION_V1 {
        (memo[8..8 + len].to_vec(), None)
    } else {
        let mut outer = Cursor::new(&memo[8..8 + len]);
        let value: ciborium::value::Value =
            ciborium::de::from_reader(&mut outer).map_err(|_| invalid("malformed ZADP CBOR"))?;
        if outer.position() != len as u64 {
            return Err(invalid("trailing ZADP CBOR"));
        }
        let ciborium::value::Value::Array(mut fields) = value else {
            return Err(invalid("expected OFFER tuple"));
        };
        if fields.len() != 14 {
            return Err(invalid("wire v2 requires 14 fields"));
        }
        let geo = match fields.pop().unwrap() {
            ciborium::value::Value::Null => None,
            value => {
                let mut encoded = Vec::new();
                ciborium::ser::into_writer(&value, &mut encoded)
                    .map_err(|_| invalid("invalid geo"))?;
                let (lat_e5, lon_e5, radius_m): (i32, i32, u32) =
                    ciborium::de::from_reader(encoded.as_slice())
                        .map_err(|_| invalid("invalid geo triple"))?;
                let area = crate::GeoArea {
                    lat_e5,
                    lon_e5,
                    radius_m,
                };
                area.validate()?;
                Some(area)
            }
        };
        let mut legacy = Vec::new();
        ciborium::ser::into_writer(&ciborium::value::Value::Array(fields), &mut legacy)
            .map_err(|_| invalid("malformed tuple"))?;
        (legacy, geo)
    };
    let mut reader = Cursor::new(legacy.as_slice());
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
    if reader.position() != legacy.len() as u64
        || seller.len() != 32
        || sig.len() != 64
        || nonce.len() < 16
        || nonce.len() > 32
    {
        return Err(invalid("invalid memo field lengths"));
    }
    if (memo[5] == WIRE_VERSION_V1 && zadp != crate::OFFER_VERSION_V1)
        || (memo[5] == WIRE_VERSION_V2 && zadp != crate::OFFER_VERSION_V2)
    {
        return Err(invalid("offer/wire version mismatch"));
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
        geo,
        signature,
    })
}

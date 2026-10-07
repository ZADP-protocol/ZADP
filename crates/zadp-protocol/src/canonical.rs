use ciborium::value::Value;

use crate::offer::Offer;
use crate::ProtocolError;

/// ASCII prefix for `zadp1-sign-v1`: `ZADP1\0offer\0`
pub const SIGN_PREFIX: &[u8] = b"ZADP1\0offer\0";

/// Build canonical CBOR bytes for an offer body (excluding `signature`).
///
/// Map keys are ordered by RFC 8949 preferred serialization: sorted by the
/// bytewise representation of the encoded key.
pub fn canonical_offer_cbor(offer: &Offer) -> Result<Vec<u8>, ProtocolError> {
    offer.validate_geo_version()?;
    let mut entries: Vec<(Vec<u8>, String, Value)> = vec![
        entry_text("capability", offer.capability.clone())?,
        entry_text("endpoint", offer.endpoint.clone())?,
        entry_text("expires_at", offer.expires_at.to_rfc3339())?,
        entry_text("id", offer.id.clone())?,
        entry_text("nonce", offer.nonce.clone())?,
        entry_map("price", price_map(offer)?)?,
        entry_text("seller", offer.seller.as_str().to_string())?,
        entry_text("title", offer.title.clone())?,
        entry_text("type", offer.message_type.clone())?,
        (
            encode_text_key("zadp")?,
            "zadp".into(),
            Value::Integer(i64::from(offer.zadp).into()),
        ),
    ];

    if let Some(description) = &offer.description {
        if !description.is_empty() {
            entries.push(entry_text("description", description.clone())?);
        }
    }

    if let Some(board_id) = &offer.board_id {
        entries.push(entry_text("board_id", board_id.clone())?);
    }
    if let Some(issued_at) = offer.issued_at {
        entries.push(entry_text("issued_at", issued_at.to_rfc3339())?);
    }
    if let Some(geo) = offer.geo {
        entries.push((
            encode_text_key("geo")?,
            "geo".into(),
            Value::Array(vec![
                Value::Integer(geo.lat_e5.into()),
                Value::Integer(geo.lon_e5.into()),
                Value::Integer(geo.radius_m.into()),
            ]),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let map: Vec<(Value, Value)> = entries
        .into_iter()
        .map(|(_, key, value)| (Value::Text(key), value))
        .collect();

    let mut buf = Vec::new();
    ciborium::ser::into_writer(&Value::Map(map), &mut buf)
        .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
    Ok(buf)
}

/// Bytes that are actually signed: prefix || canonical CBOR body.
pub fn signing_bytes(offer: &Offer) -> Result<Vec<u8>, ProtocolError> {
    let cbor = canonical_offer_cbor(offer)?;
    let prefix: &[u8] = if offer.zadp == crate::OFFER_VERSION_V2 {
        b"ZADP2\0offer\0"
    } else {
        SIGN_PREFIX
    };
    let mut out = Vec::with_capacity(prefix.len() + cbor.len());
    out.extend_from_slice(prefix);
    out.extend_from_slice(&cbor);
    Ok(out)
}

fn price_map(offer: &Offer) -> Result<Vec<(Value, Value)>, ProtocolError> {
    let mut entries = vec![
        entry_text("amount", offer.price.amount.to_string())?,
        entry_text("currency", offer.price.currency.clone())?,
    ];
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries
        .into_iter()
        .map(|(_, key, value)| (Value::Text(key), value))
        .collect())
}

fn entry_text(key: &str, value: String) -> Result<(Vec<u8>, String, Value), ProtocolError> {
    Ok((encode_text_key(key)?, key.to_string(), Value::Text(value)))
}

fn entry_map(
    key: &str,
    value: Vec<(Value, Value)>,
) -> Result<(Vec<u8>, String, Value), ProtocolError> {
    Ok((encode_text_key(key)?, key.to_string(), Value::Map(value)))
}

fn encode_text_key(key: &str) -> Result<Vec<u8>, ProtocolError> {
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&Value::Text(key.into()), &mut buf)
        .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::SellerKeyPair;
    use crate::offer::{OfferDraft, Price};
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;

    #[test]
    fn canonical_bytes_stable_for_same_offer() {
        let keys = SellerKeyPair::generate();
        let offer = OfferDraft::new_ulid(
            keys.id.clone(),
            "llm.inference",
            "Qwen inference",
            Price::zec(Decimal::new(1, 2)),
            "http://127.0.0.1:8080",
            Utc::now() + Duration::hours(1),
            B64.encode([1u8; 16]),
        )
        .sign(&keys)
        .unwrap();

        let a = signing_bytes(&offer).unwrap();
        let b = signing_bytes(&offer).unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with(SIGN_PREFIX));
    }

    #[test]
    fn keys_sorted_by_encoded_cbor() {
        let keys = SellerKeyPair::generate();
        let offer = OfferDraft::new_ulid(
            keys.id.clone(),
            "llm.inference",
            "t",
            Price::zec(Decimal::new(1, 2)),
            "http://x",
            Utc::now() + Duration::hours(1),
            B64.encode([2u8; 16]),
        )
        .sign(&keys)
        .unwrap();
        let cbor = canonical_offer_cbor(&offer).unwrap();
        let id_enc = encode_text_key("id").unwrap();
        let cap_enc = encode_text_key("capability").unwrap();
        let id_pos = find_subslice(&cbor, &id_enc).unwrap();
        let cap_pos = find_subslice(&cbor, &cap_enc).unwrap();
        assert!(id_pos < cap_pos);
    }

    fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }
}

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::canonical::signing_bytes;
use crate::identity::{SellerId, SellerKeyPair, Signature};
use crate::ProtocolError;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MESSAGE_TYPE_OFFER: &str = "offer";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    /// Decimal ZEC amount as a string on the wire.
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
    pub currency: String,
}

impl Price {
    pub fn zec(amount: Decimal) -> Self {
        Self {
            amount,
            currency: "ZEC".into(),
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.currency != "ZEC" {
            return Err(ProtocolError::InvalidOffer(format!(
                "currency must be ZEC, got {}",
                self.currency
            )));
        }
        if self.amount <= Decimal::ZERO {
            return Err(ProtocolError::InvalidOffer(
                "price amount must be positive".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub zap: u16,
    #[serde(rename = "type")]
    pub message_type: String,
    pub id: String,
    pub seller: SellerId,
    pub capability: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub price: Price,
    pub endpoint: String,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
    pub signature: Signature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferStatus {
    Active,
    Expired,
    Rejected,
}

/// Unsigned offer fields used while constructing a signature.
#[derive(Debug, Clone)]
pub struct OfferDraft {
    pub id: String,
    pub seller: SellerId,
    pub capability: String,
    pub title: String,
    pub description: Option<String>,
    pub price: Price,
    pub endpoint: String,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}

impl OfferDraft {
    pub fn new_ulid(
        seller: SellerId,
        capability: impl Into<String>,
        title: impl Into<String>,
        price: Price,
        endpoint: impl Into<String>,
        expires_at: DateTime<Utc>,
        nonce_b64: impl Into<String>,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            seller,
            capability: capability.into(),
            title: title.into(),
            description: None,
            price,
            endpoint: endpoint.into(),
            expires_at,
            nonce: nonce_b64.into(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn sign(self, keys: &SellerKeyPair) -> Result<Offer, ProtocolError> {
        if keys.id != self.seller {
            return Err(ProtocolError::InvalidOffer(
                "signing key does not match seller id".into(),
            ));
        }
        self.price.validate()?;

        // Placeholder signature so we can reuse Offer for canonicalization.
        let placeholder = Signature::from_bytes(&ed25519_dalek::Signature::from_bytes(&[0u8; 64]));
        let mut offer = Offer {
            zap: PROTOCOL_VERSION,
            message_type: MESSAGE_TYPE_OFFER.into(),
            id: self.id,
            seller: self.seller,
            capability: self.capability,
            title: self.title,
            description: self.description,
            price: self.price,
            endpoint: self.endpoint,
            expires_at: self.expires_at,
            nonce: self.nonce,
            signature: placeholder,
        };
        let bytes = signing_bytes(&offer)?;
        offer.signature = keys.sign(&bytes);
        Ok(offer)
    }
}

pub fn verify_offer(offer: &Offer, now: DateTime<Utc>) -> Result<(), ProtocolError> {
    if offer.zap != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(offer.zap));
    }
    if offer.message_type != MESSAGE_TYPE_OFFER {
        return Err(ProtocolError::UnsupportedType(offer.message_type.clone()));
    }
    if offer.id.is_empty() {
        return Err(ProtocolError::InvalidOffer("empty id".into()));
    }
    if offer.capability.is_empty() {
        return Err(ProtocolError::InvalidOffer("empty capability".into()));
    }
    if offer.title.is_empty() {
        return Err(ProtocolError::InvalidOffer("empty title".into()));
    }
    if offer.endpoint.is_empty() {
        return Err(ProtocolError::InvalidOffer("empty endpoint".into()));
    }
    if offer.nonce.is_empty() {
        return Err(ProtocolError::InvalidOffer("empty nonce".into()));
    }
    offer.price.validate()?;
    if offer.expires_at <= now {
        return Err(ProtocolError::Expired(offer.expires_at.to_rfc3339()));
    }
    let bytes = signing_bytes(offer)?;
    SellerKeyPair::verify(&offer.seller, &bytes, &offer.signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use rust_decimal::Decimal;

    fn sample_offer() -> Offer {
        let keys = SellerKeyPair::generate();
        let nonce = B64.encode([7u8; 16]);
        OfferDraft::new_ulid(
            keys.id.clone(),
            "llm.inference",
            "Qwen inference",
            Price::zec(Decimal::new(1, 2)),
            "http://127.0.0.1:8080",
            Utc::now() + chrono::Duration::hours(1),
            nonce,
        )
        .with_description("Mock LLM inference for demo")
        .sign(&keys)
        .expect("sign")
    }

    #[test]
    fn signed_offer_verifies() {
        verify_offer(&sample_offer(), Utc::now()).expect("verify");
    }

    #[test]
    fn tampered_price_fails() {
        let mut offer = sample_offer();
        offer.price.amount = Decimal::new(2, 2);
        assert!(verify_offer(&offer, Utc::now()).is_err());
    }

    #[test]
    fn expired_offer_fails() {
        let keys = SellerKeyPair::generate();
        let nonce = B64.encode([9u8; 16]);
        let offer = OfferDraft::new_ulid(
            keys.id.clone(),
            "llm.inference",
            "Old",
            Price::zec(Decimal::new(1, 2)),
            "http://127.0.0.1:8080",
            Utc::now() - chrono::Duration::minutes(1),
            nonce,
        )
        .sign(&keys)
        .unwrap();
        assert!(matches!(
            verify_offer(&offer, Utc::now()),
            Err(ProtocolError::Expired(_))
        ));
    }
}

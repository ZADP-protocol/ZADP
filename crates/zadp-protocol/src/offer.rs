use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::canonical::signing_bytes;
use crate::identity::{SellerId, SellerKeyPair, Signature};
use crate::ProtocolError;

/// Public release version, distinct from signed Offer/wire identifiers.
pub const PROTOCOL_RELEASE: &str = "1.0";
/// The sole canonical Offer model for new Protocol 1.0 publications.
pub const PROTOCOL_VERSION: u16 = OFFER_VERSION_V3;
/// Pre-release legacy: historical read/verification only.
pub const OFFER_VERSION_V1: u16 = 1;
/// Pre-release legacy: historical read/verification only.
pub const OFFER_VERSION_V2: u16 = 2;
pub const OFFER_VERSION_V3: u16 = 3;
pub const BOARD_DESCRIPTOR_VERSION: u16 = 1;
pub const MAX_GEO_RADIUS_M: u32 = 500_000;

/// Automated order API endpoint. Advertised price is independent in the typed model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commerce {
    pub endpoint: String,
}
impl Commerce {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.endpoint.is_empty() {
            return Err(ProtocolError::InvalidOffer("empty endpoint".into()));
        }
        Ok(())
    }
}
/// Historical v2 JSON shape; never emitted for v3.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCommerce {
    endpoint: String,
    price: Price,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PurchaseMode {
    Discovery,
    Manual,
    Automated,
}

#[derive(Clone, Serialize, Deserialize)]
struct OfferJson {
    zadp: u16,
    #[serde(rename = "type")]
    message_type: String,
    id: String,
    seller: SellerId,
    capability: String,
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price: Option<Price>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commerce: Option<serde_json::Value>,
    expires_at: DateTime<Utc>,
    nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    board_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    issued_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    geo: Option<GeoArea>,
    signature: Signature,
}
impl TryFrom<OfferJson> for Offer {
    type Error = String;
    fn try_from(o: OfferJson) -> Result<Self, Self::Error> {
        let (price, commerce) = match o.zadp {
            OFFER_VERSION_V1 => {
                if o.commerce.is_some() {
                    return Err("v1 uses legacy commerce fields".into());
                }
                (
                    Some(o.price.ok_or("missing price")?),
                    Some(Commerce {
                        endpoint: o.endpoint.ok_or("missing endpoint")?,
                    }),
                )
            }
            OFFER_VERSION_V2 => {
                if o.endpoint.is_some() || o.price.is_some() {
                    return Err("v2 uses optional commerce block, not legacy price/endpoint".into());
                }
                match o.commerce {
                    Some(v) => {
                        let c: LegacyCommerce =
                            serde_json::from_value(v).map_err(|e| e.to_string())?;
                        (
                            Some(c.price),
                            Some(Commerce {
                                endpoint: c.endpoint,
                            }),
                        )
                    }
                    None => (None, None),
                }
            }
            OFFER_VERSION_V3 => {
                if o.endpoint.is_some() {
                    return Err("v3 uses endpoint-only commerce block".into());
                }
                let commerce = o
                    .commerce
                    .map(serde_json::from_value::<Commerce>)
                    .transpose()
                    .map_err(|e| e.to_string())?;
                if commerce.is_some() && o.price.is_none() {
                    return Err("endpoint requires price".into());
                }
                (o.price, commerce)
            }
            _ => return Err("unsupported offer version".into()),
        };
        Ok(Self {
            price,
            commerce,
            zadp: o.zadp,
            message_type: o.message_type,
            id: o.id,
            seller: o.seller,
            capability: o.capability,
            title: o.title,
            description: o.description,
            expires_at: o.expires_at,
            nonce: o.nonce,
            board_id: o.board_id,
            issued_at: o.issued_at,
            geo: o.geo,
            signature: o.signature,
        })
    }
}
impl From<Offer> for OfferJson {
    fn from(o: Offer) -> Self {
        let (price, endpoint, commerce) = match o.zadp {
            OFFER_VERSION_V1 => (o.price, o.commerce.map(|c| c.endpoint), None),
            OFFER_VERSION_V2 => (
                None,
                None,
                o.commerce.map(|c| {
                    serde_json::to_value(LegacyCommerce {
                        endpoint: c.endpoint,
                        price: o.price.expect("validated v2 price"),
                    })
                    .expect("serializable commerce")
                }),
            ),
            _ => (
                o.price,
                None,
                o.commerce
                    .map(|c| serde_json::to_value(c).expect("serializable commerce")),
            ),
        };
        Self {
            price,
            endpoint,
            commerce,
            zadp: o.zadp,
            message_type: o.message_type,
            id: o.id,
            seller: o.seller,
            capability: o.capability,
            title: o.title,
            description: o.description,
            expires_at: o.expires_at,
            nonce: o.nonce,
            board_id: o.board_id,
            issued_at: o.issued_at,
            geo: o.geo,
            signature: o.signature,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoArea {
    pub lat_e5: i32,
    pub lon_e5: i32,
    pub radius_m: u32,
}
impl GeoArea {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if !(-9_000_000..=9_000_000).contains(&self.lat_e5)
            || !(-18_000_000..=18_000_000).contains(&self.lon_e5)
            || !(1..=MAX_GEO_RADIUS_M).contains(&self.radius_m)
        {
            return Err(ProtocolError::InvalidOffer("invalid offer area".into()));
        }
        Ok(())
    }
}
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
#[serde(try_from = "OfferJson", into = "OfferJson")]
pub struct Offer {
    pub zadp: u16,
    pub message_type: String,
    pub id: String,
    pub seller: SellerId,
    pub capability: String,
    pub title: String,
    pub description: Option<String>,
    pub price: Option<Price>,
    pub commerce: Option<Commerce>,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
    pub board_id: Option<String>,
    pub issued_at: Option<DateTime<Utc>>,
    pub geo: Option<GeoArea>,
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
    pub version: Option<u16>,
    pub id: String,
    pub seller: SellerId,
    pub capability: String,
    pub title: String,
    pub description: Option<String>,
    pub geo: Option<GeoArea>,
    pub price: Option<Price>,
    pub commerce: Option<Commerce>,
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
            version: None,
            id: Ulid::new().to_string(),
            seller,
            capability: capability.into(),
            title: title.into(),
            description: None,
            geo: None,
            price: Some(price),
            commerce: Some(Commerce {
                endpoint: endpoint.into(),
            }),
            expires_at,
            nonce: nonce_b64.into(),
        }
    }

    pub fn new_discovery(
        seller: SellerId,
        capability: impl Into<String>,
        title: impl Into<String>,
        expires_at: DateTime<Utc>,
        nonce_b64: impl Into<String>,
    ) -> Self {
        Self {
            version: None,
            id: Ulid::new().to_string(),
            seller,
            capability: capability.into(),
            title: title.into(),
            description: None,
            geo: None,
            price: None,
            commerce: None,
            expires_at,
            nonce: nonce_b64.into(),
        }
    }

    pub fn discovery_only(mut self) -> Self {
        self.price = None;
        self.commerce = None;
        self
    }

    /// Structured price is signed by the Protocol 1.0 canonical wire-3 model.
    pub fn with_price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self.version = Some(OFFER_VERSION_V3);
        self
    }
    pub fn with_version(mut self, version: u16) -> Self {
        self.version = Some(version);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_geo(mut self, geo: GeoArea) -> Self {
        self.geo = Some(geo);
        self
    }

    pub fn sign(self, keys: &SellerKeyPair) -> Result<Offer, ProtocolError> {
        if self.version.unwrap_or(PROTOCOL_VERSION) != OFFER_VERSION_V3 {
            return Err(ProtocolError::InvalidOffer(
                "wire 1/2 are pre-release legacy; new Offers require Protocol 1.0 / wire 3".into(),
            ));
        }
        if keys.id != self.seller {
            return Err(ProtocolError::InvalidOffer(
                "signing key does not match seller id".into(),
            ));
        }
        if let Some(p) = &self.price {
            p.validate()?;
        }
        if let Some(c) = &self.commerce {
            c.validate()?;
        }

        // Placeholder signature so we can reuse Offer for canonicalization.
        let placeholder = Signature::from_bytes(&ed25519_dalek::Signature::from_bytes(&[0u8; 64]));
        let mut offer = Offer {
            zadp: self.version.unwrap_or(PROTOCOL_VERSION),
            message_type: MESSAGE_TYPE_OFFER.into(),
            id: self.id,
            seller: self.seller,
            capability: self.capability,
            title: self.title,
            description: self.description,
            geo: self.geo,
            price: self.price,
            commerce: self.commerce,
            expires_at: self.expires_at,
            nonce: self.nonce,
            board_id: None,
            issued_at: None,
            signature: placeholder,
        };
        let bytes = signing_bytes(&offer)?;
        offer.signature = keys.sign(&bytes);
        Ok(offer)
    }
}

impl Offer {
    pub fn purchase_mode(&self) -> PurchaseMode {
        if self.commerce.is_some() {
            PurchaseMode::Automated
        } else if self.price.is_some() {
            PurchaseMode::Manual
        } else {
            PurchaseMode::Discovery
        }
    }
    pub fn validate_geo_version(&self) -> Result<(), ProtocolError> {
        if ![OFFER_VERSION_V1, OFFER_VERSION_V2, OFFER_VERSION_V3].contains(&self.zadp) {
            return Err(ProtocolError::UnsupportedVersion(self.zadp));
        }
        if self.commerce.is_some() && self.price.is_none() {
            return Err(ProtocolError::InvalidOffer(
                "endpoint requires price".into(),
            ));
        }
        if self.zadp < OFFER_VERSION_V3 && self.price.is_some() && self.commerce.is_none() {
            return Err(ProtocolError::InvalidOffer(
                "manual pricing requires v3".into(),
            ));
        }
        if self.zadp == OFFER_VERSION_V3 {
            if let Some(p) = &self.price {
                p.validate()?;
                crate::zec_to_zat(p.amount)?;
            }
        }
        if self.zadp == OFFER_VERSION_V1 && self.commerce.is_none() {
            return Err(ProtocolError::InvalidOffer("v1 requires commerce".into()));
        }
        if let Some(geo) = self.geo {
            if self.zadp == OFFER_VERSION_V1 {
                return Err(ProtocolError::InvalidOffer(
                    "geo requires Offer v2 or v3".into(),
                ));
            }
            geo.validate()?;
        }
        Ok(())
    }
}
/// New publication gate, deliberately separate from historical verification.
pub fn verify_offer_for_publication(
    offer: &Offer,
    now: DateTime<Utc>,
) -> Result<(), ProtocolError> {
    if offer.zadp != OFFER_VERSION_V3 {
        return Err(ProtocolError::InvalidOffer(
            "pre-release legacy Offer cannot be newly published; Protocol 1.0 requires wire 3"
                .into(),
        ));
    }
    verify_offer(offer, now)
}

/// Verify current or pre-release legacy signed data without authorizing publication.
pub fn verify_offer(offer: &Offer, now: DateTime<Utc>) -> Result<(), ProtocolError> {
    offer.validate_geo_version()?;
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
    if offer.nonce.is_empty() {
        return Err(ProtocolError::InvalidOffer("empty nonce".into()));
    }
    if let Some(p) = &offer.price {
        p.validate()?;
    }
    if let Some(c) = &offer.commerce {
        c.validate()?;
        if offer.zadp >= OFFER_VERSION_V2 {
            let url = url::Url::parse(&c.endpoint)
                .map_err(|_| ProtocolError::InvalidOffer("invalid endpoint URL".into()))?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(ProtocolError::InvalidOffer(
                    "commerce requires credential-free HTTPS".into(),
                ));
            }
        }
    }
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
            "https://seller.example",
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
        offer.price.as_mut().unwrap().amount = Decimal::new(2, 2);
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
            "https://seller.example",
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

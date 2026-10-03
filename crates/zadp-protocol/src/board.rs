//! Portable signed discovery-board contract. Viewing material here is public.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zcash_keys::{address::Address, keys::UnifiedIncomingViewingKey};
use zcash_protocol::consensus::{Network, NetworkUpgrade, Parameters};

use crate::{Offer, ProtocolError, SellerId, SellerKeyPair, Signature, PROTOCOL_VERSION};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZcashNetwork {
    Mainnet,
    Testnet,
}
impl ZcashNetwork {
    pub fn parameters(self) -> Network {
        match self {
            Self::Mainnet => Network::MainNetwork,
            Self::Testnet => Network::TestNetwork,
        }
    }
    pub fn chain_name(self) -> &'static str {
        match self {
            Self::Mainnet => "main",
            Self::Testnet => "test",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    Open,
    Authorized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardPolicy {
    pub admission: Admission,
    /// Operator-signed allowlist is the MVP authorization mechanism.
    pub authorized_sellers: Vec<SellerId>,
    pub max_offer_ttl_seconds: u32,
    pub min_listing_value_zat: u64,
    pub max_active_offers_per_seller: u32,
    pub max_publications_per_seller_per_window: u32,
    pub rate_window_seconds: u32,
    pub confirmed_after_blocks: u32,
    pub max_clock_skew_seconds: u32,
}
impl Default for BoardPolicy {
    fn default() -> Self {
        Self {
            admission: Admission::Open,
            authorized_sellers: vec![],
            max_offer_ttl_seconds: 604800,
            min_listing_value_zat: 1000,
            max_active_offers_per_seller: 20,
            max_publications_per_seller_per_window: 20,
            rate_window_seconds: 86400,
            confirmed_after_blocks: 3,
            max_clock_skew_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardBody {
    pub zadp: u16,
    pub board_id: String,
    pub name: String,
    pub network: ZcashNetwork,
    /// MVP requires an Orchard-only UA so wallets cannot select an unscanned pool.
    pub address: String,
    pub incoming_viewing_key: String,
    pub birthday_height: u32,
    pub policy: BoardPolicy,
    pub operator_key: SellerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardDescriptor {
    pub board: BoardBody,
    pub signature: Signature,
}

fn invalid(reason: &str) -> ProtocolError {
    ProtocolError::InvalidOffer(reason.into())
}

/// Deterministic CBOR: recursively sort map keys by encoded bytes; no floats.
pub fn canonical_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    fn encoded(value: &ciborium::value::Value) -> Result<Vec<u8>, ProtocolError> {
        let mut out = vec![];
        ciborium::ser::into_writer(value, &mut out)
            .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
        Ok(out)
    }
    fn normalize(value: &mut ciborium::value::Value) -> Result<(), ProtocolError> {
        use ciborium::value::Value;
        match value {
            Value::Map(entries) => {
                for (key, val) in entries.iter_mut() {
                    normalize(key)?;
                    normalize(val)?;
                }
                let mut sorted = entries
                    .drain(..)
                    .map(|(k, v)| Ok((encoded(&k)?, k, v)))
                    .collect::<Result<Vec<_>, ProtocolError>>()?;
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                *entries = sorted.into_iter().map(|(_, k, v)| (k, v)).collect();
            }
            Value::Array(values) => {
                for v in values {
                    normalize(v)?;
                }
            }
            Value::Float(_) | Value::Tag(_, _) => {
                return Err(invalid("floats and tags are forbidden"))
            }
            _ => {}
        }
        Ok(())
    }
    let mut val = ciborium::value::Value::serialized(value)
        .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
    normalize(&mut val)?;
    encoded(&val)
}

impl BoardBody {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.zadp != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(self.zadp));
        }
        if self.board_id.is_empty()
            || self.board_id.len() > 64
            || !self
                .board_id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err(invalid("board_id must be a lowercase slug of 1..64 bytes"));
        }
        if self.name.is_empty() || self.name.len() > 128 || self.name.chars().any(char::is_control)
        {
            return Err(invalid("invalid board name"));
        }
        let p = &self.policy;
        if p.max_offer_ttl_seconds == 0
            || p.min_listing_value_zat == 0
            || p.min_listing_value_zat > 2_100_000_000_000_000
            || p.max_active_offers_per_seller == 0
            || p.max_publications_per_seller_per_window == 0
            || p.rate_window_seconds == 0
            || p.confirmed_after_blocks == 0
            || p.confirmed_after_blocks > 10000
            || p.max_clock_skew_seconds > 3600
        {
            return Err(invalid("invalid board policy limits"));
        }
        if p.authorized_sellers.len() > 10000 {
            return Err(invalid("authorization list too large"));
        }
        for id in &p.authorized_sellers {
            id.verifying_key()?;
        }
        self.operator_key.verifying_key()?;
        let params = self.network.parameters();
        let activation: u32 = params
            .activation_height(NetworkUpgrade::Nu5)
            .ok_or_else(|| invalid("Orchard unavailable"))?
            .into();
        if self.birthday_height < activation {
            return Err(invalid("birthday precedes Orchard activation"));
        }
        let Address::Unified(ua) = Address::decode(&params, &self.address)
            .ok_or_else(|| invalid("invalid address or network mismatch"))?
        else {
            return Err(invalid("board requires a Unified Address"));
        };
        if !ua.has_orchard() || ua.has_sapling() || ua.has_transparent() || !ua.unknown().is_empty()
        {
            return Err(invalid("board requires an Orchard-only Unified Address"));
        }
        let key = UnifiedIncomingViewingKey::decode(&params, &self.incoming_viewing_key)
            .map_err(|_| invalid("invalid incoming viewing key or network mismatch"))?;
        let ivk = key
            .orchard()
            .as_ref()
            .ok_or_else(|| invalid("missing Orchard incoming viewing key"))?;
        if ivk
            .diversifier_index(
                ua.orchard()
                    .ok_or_else(|| invalid("missing Orchard receiver"))?,
            )
            .is_none()
        {
            return Err(invalid("viewing key does not control board receiver"));
        }
        Ok(())
    }
    pub fn sign(self, keys: &SellerKeyPair) -> Result<BoardDescriptor, ProtocolError> {
        self.validate()?;
        if self.operator_key != keys.id {
            return Err(invalid("operator signing key mismatch"));
        }
        let mut data = b"ZADP1\0board\0".to_vec();
        data.extend(canonical_cbor(&self)?);
        let signature = keys.sign(&data);
        Ok(BoardDescriptor {
            board: self,
            signature,
        })
    }
}
impl BoardDescriptor {
    pub fn validate(&self, network: ZcashNetwork) -> Result<(), ProtocolError> {
        if self.board.network != network {
            return Err(invalid("board network mismatch"));
        }
        self.board.validate()?;
        let mut data = b"ZADP1\0board\0".to_vec();
        data.extend(canonical_cbor(&self.board)?);
        SellerKeyPair::verify(&self.board.operator_key, &data, &self.signature)
    }
    pub fn fingerprint(&self) -> Result<String, ProtocolError> {
        let hash = Sha256::digest(canonical_cbor(self)?);
        Ok(hash.iter().map(|b| format!("{b:02x}")).collect())
    }
    /// Stateless admission. Counts and replay protection are transactional repository rules.
    pub fn validate_offer(
        &self,
        offer: &Offer,
        value_zat: u64,
        block_time: DateTime<Utc>,
    ) -> Result<(), ProtocolError> {
        crate::verify_offer(offer, block_time)?;
        if offer.title.chars().any(char::is_control) {
            return Err(invalid("offer title contains control characters"));
        }
        if offer.board_id.as_deref() != Some(&self.board.board_id) {
            return Err(invalid("wrong board binding"));
        }
        let issued = offer
            .issued_at
            .ok_or_else(|| invalid("missing issued_at"))?;
        let ttl = offer.expires_at.signed_duration_since(issued).num_seconds();
        if issued
            > block_time
                + chrono::Duration::seconds(i64::from(self.board.policy.max_clock_skew_seconds))
            || ttl <= 0
            || ttl > i64::from(self.board.policy.max_offer_ttl_seconds)
        {
            return Err(invalid("invalid offer TTL or future issue time"));
        }
        if value_zat < self.board.policy.min_listing_value_zat {
            return Err(invalid("listing value below board minimum"));
        }
        if self.board.policy.admission == Admission::Authorized
            && !self.board.policy.authorized_sellers.contains(&offer.seller)
        {
            return Err(invalid("seller is not authorized"));
        }
        let endpoint =
            url::Url::parse(&offer.endpoint).map_err(|_| invalid("invalid endpoint URL"))?;
        if endpoint.scheme() != "https"
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(invalid(
                "real offer endpoint requires HTTPS without credentials",
            ));
        }
        Ok(())
    }
}

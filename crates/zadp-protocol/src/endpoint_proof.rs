//! Off-chain endpoint ownership proof. Never included in Offer signing or memo encoding.
use crate::{board::canonical_cbor, ProtocolError, SellerId, SellerKeyPair, Signature};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

pub const ENDPOINT_PROOF_PREFIX: &[u8] = b"ZADP-ENDPOINT-PROOF/1\0";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointProof {
    pub zadp: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub seller: SellerId,
    pub origin: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub signature: Signature,
}
fn invalid() -> ProtocolError {
    ProtocolError::InvalidOffer("invalid endpoint ownership proof".into())
}
/// Conservative public-unicast policy; DNS answers must pass this too, before connecting.
pub fn public_endpoint_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_documentation()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0)
                || (a == 198 && (b == 18 || b == 19)))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // Only ordinary global unicast. Exclude translation, transition and special ranges.
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}
/// Normalize only the origin; a commerce endpoint may have a service path.
/// Reject credentials, fragments and ambiguous input before URL normalization.
pub fn commerce_origin(endpoint: &str) -> Result<String, ProtocolError> {
    if endpoint.len() > 2048
        || endpoint
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Err(invalid());
    }
    let u = url::Url::parse(endpoint).map_err(|_| invalid())?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || !endpoint.starts_with("https://")
        || endpoint[8..].split('/').next().unwrap_or("").contains('@')
    {
        return Err(invalid());
    }
    match u.host().ok_or_else(invalid)? {
        url::Host::Domain(d) => {
            if !d.contains('.')
                || d.ends_with('.')
                || d == "localhost"
                || [".localhost", ".local", ".internal", ".home", ".lan"]
                    .iter()
                    .any(|s| d.ends_with(s))
            {
                return Err(invalid());
            }
        }
        url::Host::Ipv4(ip) => {
            if !public_endpoint_ip(ip.into()) {
                return Err(invalid());
            }
        }
        url::Host::Ipv6(ip) => {
            if !public_endpoint_ip(ip.into()) {
                return Err(invalid());
            }
        }
    }
    Ok(u.origin().ascii_serialization())
}
impl EndpointProof {
    pub fn sign(
        keys: &SellerKeyPair,
        origin: &str,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, ProtocolError> {
        let mut proof = Self {
            zadp: 1,
            kind: "commerce-endpoint".into(),
            seller: keys.id.clone(),
            origin: origin.into(),
            issued_at,
            expires_at,
            signature: keys.sign(b""),
        };
        proof.signature = keys.sign(&proof.signing_bytes()?);
        proof.verify(&keys.id, origin, issued_at)?;
        Ok(proof)
    }
    /// Canonical CBOR array [1, type, seller, origin, issued Unix seconds, expiry Unix seconds].
    pub fn signing_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.zadp != 1
            || self.kind != "commerce-endpoint"
            || commerce_origin(&self.origin)? != self.origin
            || self.issued_at.timestamp_subsec_nanos() != 0
            || self.expires_at.timestamp_subsec_nanos() != 0
            || self.expires_at <= self.issued_at
        {
            return Err(invalid());
        }
        let mut bytes = ENDPOINT_PROOF_PREFIX.to_vec();
        bytes.extend(canonical_cbor(&(
            self.zadp,
            &self.kind,
            &self.seller,
            &self.origin,
            self.issued_at.timestamp(),
            self.expires_at.timestamp(),
        ))?);
        Ok(bytes)
    }
    pub fn verify(
        &self,
        seller: &SellerId,
        endpoint: &str,
        now: DateTime<Utc>,
    ) -> Result<(), ProtocolError> {
        if &self.seller != seller
            || self.origin != commerce_origin(endpoint)?
            || now < self.issued_at
            || now >= self.expires_at
        {
            return Err(invalid());
        }
        SellerKeyPair::verify(&self.seller, &self.signing_bytes()?, &self.signature)
    }
}

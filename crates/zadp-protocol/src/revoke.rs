//! Additive append-only control messages. Offer canonicalization is deliberately untouched.
use crate::{BoardDescriptor, ProtocolError, SellerId, SellerKeyPair, Signature};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::io::Cursor;

pub const OFFER_REVOKE_VERSION: u16 = 1;
/// Message discriminator, NOT an Offer wire version.
pub const WIRE_OFFER_REVOKE_V1: u8 = 0x81;
pub const REVOKE_SIGNING_DOMAIN: &[u8] = b"ZADP1\0offer_revoke\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferRevoke {
    pub zadp: u16,
    #[serde(rename = "type")]
    pub message_type: String,
    pub board_id: String,
    pub offer_id: String,
    pub seller: SellerId,
    pub issued_at: DateTime<Utc>,
    pub nonce: String,
    pub signature: Signature,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZadpMessage {
    Offer(crate::Offer),
    OfferRevoke(OfferRevoke),
}
fn invalid(reason: &str) -> ProtocolError {
    ProtocolError::InvalidOffer(reason.into())
}
impl OfferRevoke {
    pub fn sign(
        board_id: String,
        offer_id: String,
        issued_at: DateTime<Utc>,
        nonce: String,
        keys: &SellerKeyPair,
    ) -> Result<Self, ProtocolError> {
        let mut r = Self {
            zadp: OFFER_REVOKE_VERSION,
            message_type: "offer_revoke".into(),
            board_id,
            offer_id,
            seller: keys.id.clone(),
            issued_at,
            nonce,
            signature: keys.sign(b"placeholder"),
        };
        r.signature = keys.sign(&r.signing_bytes()?);
        Ok(r)
    }
    fn validate_fields(&self) -> Result<(), ProtocolError> {
        if self.zadp != OFFER_REVOKE_VERSION {
            return Err(ProtocolError::UnsupportedVersion(self.zadp));
        }
        if self.message_type != "offer_revoke" {
            return Err(ProtocolError::UnsupportedType(self.message_type.clone()));
        }
        for field in [&self.board_id, &self.offer_id] {
            if field.is_empty() || field.len() > 64 || field.chars().any(char::is_control) {
                return Err(invalid("invalid revoke reference"));
            }
        }
        if self.issued_at.timestamp() < 0 || self.issued_at.timestamp_subsec_nanos() != 0 {
            return Err(invalid("invalid revoke issue time"));
        }
        let nonce = B64
            .decode(&self.nonce)
            .map_err(|_| invalid("invalid revoke nonce"))?;
        if !(16..=32).contains(&nonce.len()) || B64.encode(&nonce) != self.nonce {
            return Err(invalid("invalid revoke nonce"));
        }
        self.seller.verifying_key()?;
        Ok(())
    }
    /// Canonical deterministic array; all signed fields including type/version are covered.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate_fields()?;
        let mut bytes = REVOKE_SIGNING_DOMAIN.to_vec();
        ciborium::ser::into_writer(
            &(
                self.zadp,
                &self.message_type,
                &self.board_id,
                &self.offer_id,
                ByteBuf::from(self.seller.verifying_key()?.to_bytes().to_vec()),
                self.issued_at.timestamp(),
                ByteBuf::from(
                    B64.decode(&self.nonce)
                        .map_err(|_| invalid("invalid nonce"))?,
                ),
            ),
            &mut bytes,
        )
        .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
        Ok(bytes)
    }
    pub fn verify(&self) -> Result<(), ProtocolError> {
        SellerKeyPair::verify(&self.seller, &self.signing_bytes()?, &self.signature)
    }
    /// Board v1 policy reuse. Reference existence/order/replay are repository responsibilities.
    pub fn validate_board(
        &self,
        board: &BoardDescriptor,
        value: u64,
        block_time: DateTime<Utc>,
    ) -> Result<(), ProtocolError> {
        self.verify()?;
        if self.board_id != board.board.board_id {
            return Err(invalid("wrong board binding"));
        }
        if self.issued_at
            > block_time
                + chrono::Duration::seconds(i64::from(board.board.policy.max_clock_skew_seconds))
        {
            return Err(invalid("future revoke issue time"));
        }
        if value < board.board.policy.min_listing_value_zat {
            return Err(invalid("listing value below board minimum"));
        }
        if board.board.policy.admission == crate::Admission::Authorized
            && !board.board.policy.authorized_sellers.contains(&self.seller)
        {
            return Err(invalid("seller is not authorized"));
        }
        Ok(())
    }
}
type WireRevoke = (u16, String, String, ByteBuf, i64, ByteBuf, ByteBuf);
pub fn encode_revoke_memo(r: &OfferRevoke) -> Result<[u8; 512], ProtocolError> {
    r.verify()?;
    let wire: WireRevoke = (
        r.zadp,
        r.board_id.clone(),
        r.offer_id.clone(),
        ByteBuf::from(r.seller.verifying_key()?.to_bytes().to_vec()),
        r.issued_at.timestamp(),
        ByteBuf::from(B64.decode(&r.nonce).map_err(|_| invalid("invalid nonce"))?),
        ByteBuf::from(r.signature.to_ed25519()?.to_bytes().to_vec()),
    );
    let mut payload = Vec::new();
    ciborium::ser::into_writer(&wire, &mut payload)
        .map_err(|e| ProtocolError::Canonical(e.to_string()))?;
    if payload.len() > 504 {
        return Err(invalid("revoke memo exceeds 504-byte payload"));
    }
    let mut memo = [0u8; 512];
    memo[..5].copy_from_slice(b"\xf5ZADP");
    memo[5] = WIRE_OFFER_REVOKE_V1;
    memo[6..8].copy_from_slice(&(payload.len() as u16).to_be_bytes());
    memo[8..8 + payload.len()].copy_from_slice(&payload);
    Ok(memo)
}
fn decode_revoke(memo: &[u8]) -> Result<OfferRevoke, ProtocolError> {
    let len = usize::from(u16::from_be_bytes([memo[6], memo[7]]));
    if len == 0 || len > 504 || memo[8 + len..].iter().any(|b| *b != 0) {
        return Err(invalid("invalid revoke memo length/padding"));
    }
    let mut reader = Cursor::new(&memo[8..8 + len]);
    let (v, b, id, s, t, n, sig): WireRevoke =
        ciborium::de::from_reader(&mut reader).map_err(|_| invalid("invalid revoke CBOR"))?;
    if reader.position() != len as u64 || s.len() != 32 || sig.len() != 64 {
        return Err(invalid("invalid revoke field lengths"));
    }
    let r = OfferRevoke {
        zadp: v,
        message_type: "offer_revoke".into(),
        board_id: b,
        offer_id: id,
        seller: serde_json::from_value(serde_json::json!(format!("ed25519:{}", B64.encode(s))))
            .map_err(|_| invalid("invalid seller"))?,
        issued_at: DateTime::from_timestamp(t, 0).ok_or_else(|| invalid("invalid timestamp"))?,
        nonce: B64.encode(n),
        signature: serde_json::from_value(serde_json::json!(B64.encode(sig)))
            .map_err(|_| invalid("invalid signature"))?,
    };
    r.verify()?;
    if encode_revoke_memo(&r)? != memo {
        return Err(invalid("noncanonical revoke encoding"));
    }
    Ok(r)
}
pub fn decode_zadp_memo(memo: &[u8]) -> Result<Option<ZadpMessage>, ProtocolError> {
    if !memo.starts_with(b"\xf5ZADP") {
        return Ok(None);
    }
    if memo.len() != 512 {
        return Err(invalid("malformed ZADP framing"));
    }
    if memo[5] == WIRE_OFFER_REVOKE_V1 {
        return decode_revoke(memo).map(|r| Some(ZadpMessage::OfferRevoke(r)));
    }
    crate::memo::decode_offer_memo(memo).map(|o| o.map(ZadpMessage::Offer))
}

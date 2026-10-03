//! ZADP/1 protocol types and seller identity.
//!
//! JSON is the human-readable interchange format.
//! Canonical CBOR (`zadp1-sign-v1`) is used for signatures and is the design
//! anchor for a future shielded-memo encoding (not production-packed here).

mod canonical;
mod error;
mod identity;
mod offer;

pub use canonical::{canonical_offer_cbor, signing_bytes};
pub use error::ProtocolError;
pub use identity::{SellerId, SellerKeyPair, Signature};
pub use offer::{
    verify_offer, Offer, OfferDraft, OfferStatus, Price, MESSAGE_TYPE_OFFER, PROTOCOL_VERSION,
};

mod board;
mod memo;
pub use board::{Admission, BoardBody, BoardDescriptor, BoardPolicy, ZcashNetwork};
pub use memo::{bind_offer, decode_offer_memo, encode_offer_memo, zec_to_zat};

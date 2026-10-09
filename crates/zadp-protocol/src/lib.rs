//! Backward-compatible ZADP Offer v1/v2 and BoardDescriptor v1 types.
//!
//! JSON is the human-readable interchange format.
//! Domain-separated canonical CBOR signs offers; compact memo wire v1/v2
//! transports them in 512 bytes without changing legacy v1 encoding.

mod canonical;
mod error;
mod identity;
mod offer;

pub use canonical::{canonical_offer_cbor, signing_bytes};
pub use error::ProtocolError;
pub use identity::{SellerId, SellerKeyPair, Signature};
pub use offer::{
    verify_offer, Commerce, GeoArea, Offer, OfferDraft, OfferStatus, Price,
    BOARD_DESCRIPTOR_VERSION, MAX_GEO_RADIUS_M, MESSAGE_TYPE_OFFER, OFFER_VERSION_V1,
    OFFER_VERSION_V2, PROTOCOL_VERSION,
};

mod board;
mod memo;
pub use board::{Admission, BoardBody, BoardDescriptor, BoardPolicy, ZcashNetwork};
pub use memo::{
    bind_offer, decode_offer_memo, encode_offer_memo, zec_to_zat, WIRE_VERSION_V1, WIRE_VERSION_V2,
};

mod endpoint_proof;
pub use endpoint_proof::{
    commerce_origin, public_endpoint_ip, EndpointProof, ENDPOINT_PROOF_PREFIX,
};

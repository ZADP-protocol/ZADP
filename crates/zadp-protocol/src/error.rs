use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid seller identity: {0}")]
    InvalidSellerId(String),
    #[error("invalid signature encoding: {0}")]
    InvalidSignature(String),
    #[error("signature verification failed")]
    SignatureMismatch,
    #[error("invalid offer: {0}")]
    InvalidOffer(String),
    #[error("offer expired at {0}")]
    Expired(String),
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("unsupported message type {0}")]
    UnsupportedType(String),
    #[error("canonical encoding error: {0}")]
    Canonical(String),
    #[error("crypto error: {0}")]
    Crypto(String),
}

use thiserror::Error;
use zap_protocol::ProtocolError;
use zap_zcash::ZcashError;

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Zcash(#[from] ZcashError),
    #[error("conflict: offer id {0} already indexed with different payload")]
    Conflict(String),
    #[error("decode error: {0}")]
    Decode(String),
}

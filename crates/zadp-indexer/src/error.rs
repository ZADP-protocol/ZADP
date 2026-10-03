use thiserror::Error;
use zadp_protocol::ProtocolError;
use zadp_zcash::ZcashError;

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

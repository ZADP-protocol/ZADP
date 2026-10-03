use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZcashError {
    #[error("publish failed: {0}")]
    Publish(String),
    #[error("scan failed: {0}")]
    Scan(String),
    #[error("payment failed: {0}")]
    Payment(String),
    #[error("observe failed: {0}")]
    Observe(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
}

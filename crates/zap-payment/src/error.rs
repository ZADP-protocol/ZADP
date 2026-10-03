use thiserror::Error;
use zap_zcash::ZcashError;

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("policy denied payment: {0}")]
    Denied(String),
    #[error("human approval required for amount {0} ZEC")]
    ApprovalRequired(String),
    #[error("invalid invoice: {0}")]
    InvalidInvoice(String),
    #[error(transparent)]
    Zcash(#[from] ZcashError),
}

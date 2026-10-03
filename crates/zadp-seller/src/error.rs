use thiserror::Error;
use zadp_protocol::ProtocolError;
use zadp_zcash::ZcashError;

#[derive(Debug, Error)]
pub enum SellerError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Zcash(#[from] ZcashError),
    #[error("unknown offer {0}")]
    UnknownOffer(String),
    #[error("unknown invoice {0}")]
    UnknownInvoice(String),
    #[error("invoice not paid: {0}")]
    Unpaid(String),
    #[error("offer expired")]
    OfferExpired,
}

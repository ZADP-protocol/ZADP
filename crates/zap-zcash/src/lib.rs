//! Zcash integration boundary for ZAP.
//!
//! Phase 1 provides [`MockZcashAdapter`] only. Real RPC shapes are intentionally
//! not invented here — see `docs/zcash-integration-todos.md`.

mod error;
mod mock;
mod types;

pub use error::ZcashError;
pub use mock::MockZcashAdapter;
pub use types::{
    ObservedPayment, PaymentFilter, PaymentReceipt, PaymentRequest, ScannedMessage, TxId,
    ZcashAdapter,
};

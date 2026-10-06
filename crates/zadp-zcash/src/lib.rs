//! Zcash integration boundary for ZADP.
//!
//! Mock commerce transport and actual read-only CompactTxStreamer board discovery.

mod error;
mod mock;
mod types;

pub use error::ZcashError;
pub use mock::MockZcashAdapter;
pub use types::{
    ObservedPayment, PaymentFilter, PaymentReceipt, PaymentRequest, ScannedMessage, TxId,
    ZcashAdapter,
};

mod board;
mod lightwalletd;
pub use board::{BlockHeader, BoardBlock, BoardOutput, BoardSource};
pub use lightwalletd::{
    broadcast_transaction, connect_lightwalletd, decrypt_board_outputs, initialize_tls,
    BoardTransport, RealZcashScanner,
};

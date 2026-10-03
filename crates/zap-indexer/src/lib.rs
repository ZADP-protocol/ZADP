//! Offer discovery indexer.
//!
//! Depends on [`zap_zcash::ZcashAdapter`] and [`zap_protocol`] only — not HTTP.

mod error;
mod repo;
mod service;

pub use error::IndexerError;
pub use repo::{InMemoryOfferRepository, OfferRecord, OfferRepository};
pub use service::Indexer;

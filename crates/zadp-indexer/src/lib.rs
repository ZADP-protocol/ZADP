//! Offer discovery indexer.
//!
//! Depends on [`zadp_zcash::ZcashAdapter`] and [`zadp_protocol`] only — not HTTP.

mod error;
mod repo;
mod service;

pub use error::IndexerError;
pub use repo::{InMemoryOfferRepository, OfferRecord, OfferRepository};
pub use service::Indexer;

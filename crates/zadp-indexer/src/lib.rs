//! Offer discovery indexer.
//!
//! Depends on [`zadp_zcash::ZcashAdapter`] and [`zadp_protocol`] only — not HTTP.

mod error;
mod repo;
mod service;

pub use error::IndexerError;
pub use repo::{ChainObservation, InMemoryOfferRepository, OfferRecord, OfferRepository};
pub use service::Indexer;

mod board;
mod persistent;
pub use board::BoardIndexer;
pub use persistent::{PublicationAudit, SqliteOfferRepository};
mod registry;
pub use registry::*;

//! Payment policy and orchestration.
//!
//! The LLM/agent layer may only call [`PaymentController::request_payment`].
//! This crate never exposes Zcash spending key material.

mod controller;
mod error;
mod policy;

pub use controller::{InvoiceView, PaymentController, PaymentOutcome};
pub use error::PaymentError;
pub use policy::{PolicyConfig, PolicyDecision, SpendingPolicy};

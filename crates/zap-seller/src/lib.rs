//! Seller-side commerce logic (no HTTP framework dependency).

mod error;
mod service;

pub use error::SellerError;
pub use service::{CreateInvoiceRequest, Invoice, InvoiceStatus, SellerService, ServiceResult};

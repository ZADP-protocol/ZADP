use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::ZcashError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedMessage {
    pub txid: TxId,
    pub observed_at: DateTime<Utc>,
    /// Opaque transport payload (Phase 1 mock: UTF-8 JSON offer envelope).
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub invoice_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub pay_to: String,
    /// Private order metadata that would ride in a shielded memo.
    pub memo: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentReceipt {
    pub txid: TxId,
    pub invoice_id: String,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct PaymentFilter {
    pub invoice_id: Option<String>,
    pub pay_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedPayment {
    pub txid: TxId,
    pub invoice_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub pay_to: String,
    pub memo: serde_json::Value,
    pub observed_at: DateTime<Utc>,
}

/// Replaceable Zcash boundary. Domain crates depend on this trait only.
#[async_trait]
pub trait ZcashAdapter: Send + Sync {
    async fn publish_message(&self, payload: &[u8]) -> Result<TxId, ZcashError>;
    async fn scan_messages(
        &self,
        after_txid: Option<&TxId>,
    ) -> Result<Vec<ScannedMessage>, ZcashError>;
    async fn create_payment(&self, request: PaymentRequest) -> Result<PaymentReceipt, ZcashError>;
    async fn observe_payments(
        &self,
        filter: PaymentFilter,
    ) -> Result<Vec<ObservedPayment>, ZcashError>;
}

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;
use tracing::info;
use ulid::Ulid;

use crate::types::{
    ObservedPayment, PaymentFilter, PaymentReceipt, PaymentRequest, ScannedMessage, TxId,
    ZcashAdapter,
};
use crate::ZcashError;

#[derive(Default)]
struct MockState {
    board: Vec<ScannedMessage>,
    payments: Vec<ObservedPayment>,
}

/// In-process stand-in for shielded publish/scan/pay/observe.
#[derive(Clone, Default)]
pub struct MockZcashAdapter {
    inner: Arc<Mutex<MockState>>,
}

impl MockZcashAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ZcashAdapter for MockZcashAdapter {
    async fn publish_message(&self, payload: &[u8]) -> Result<TxId, ZcashError> {
        let txid = TxId(format!("mock-tx-{}", Ulid::new()));
        let msg = ScannedMessage {
            txid: txid.clone(),
            observed_at: Utc::now(),
            payload: payload.to_vec(),
        };
        let mut state = self.inner.lock().await;
        state.board.push(msg);
        info!(txid = %txid.0, bytes = payload.len(), "mock shielded message published");
        Ok(txid)
    }

    async fn scan_messages(
        &self,
        after_txid: Option<&TxId>,
    ) -> Result<Vec<ScannedMessage>, ZcashError> {
        let state = self.inner.lock().await;
        let msgs = if let Some(after) = after_txid {
            if let Some(pos) = state.board.iter().position(|m| m.txid == *after) {
                state.board[pos + 1..].to_vec()
            } else {
                state.board.clone()
            }
        } else {
            state.board.clone()
        };
        Ok(msgs)
    }

    async fn create_payment(&self, request: PaymentRequest) -> Result<PaymentReceipt, ZcashError> {
        if request.currency != "ZEC" {
            return Err(ZcashError::Payment("only ZEC supported".into()));
        }
        let txid = TxId(format!("mock-pay-{}", Ulid::new()));
        let observed = ObservedPayment {
            txid: txid.clone(),
            invoice_id: request.invoice_id.clone(),
            amount: request.amount,
            currency: request.currency,
            pay_to: request.pay_to,
            memo: request.memo,
            observed_at: Utc::now(),
        };
        let mut state = self.inner.lock().await;
        state.payments.push(observed);
        info!(
            txid = %txid.0,
            invoice = %request.invoice_id,
            "mock shielded payment submitted"
        );
        Ok(PaymentReceipt {
            txid,
            invoice_id: request.invoice_id,
            submitted_at: Utc::now(),
        })
    }

    async fn observe_payments(
        &self,
        filter: PaymentFilter,
    ) -> Result<Vec<ObservedPayment>, ZcashError> {
        let state = self.inner.lock().await;
        Ok(state
            .payments
            .iter()
            .filter(|p| {
                filter
                    .invoice_id
                    .as_ref()
                    .is_none_or(|id| id == &p.invoice_id)
                    && filter.pay_to.as_ref().is_none_or(|addr| addr == &p.pay_to)
            })
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use serde_json::json;

    #[tokio::test]
    async fn publish_and_scan_roundtrip() {
        let adapter = MockZcashAdapter::new();
        let tx = adapter.publish_message(b"{\"zap\":1}").await.unwrap();
        let msgs = adapter.scan_messages(None).await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].txid, tx);
    }

    #[tokio::test]
    async fn payment_observe_by_invoice() {
        let adapter = MockZcashAdapter::new();
        adapter
            .create_payment(PaymentRequest {
                invoice_id: "inv-1".into(),
                amount: Decimal::new(1, 2),
                currency: "ZEC".into(),
                pay_to: "mock:shielded:seller".into(),
                memo: json!({"order_id": "o1"}),
            })
            .await
            .unwrap();
        let seen = adapter
            .observe_payments(PaymentFilter {
                invoice_id: Some("inv-1".into()),
                pay_to: None,
            })
            .await
            .unwrap();
        assert_eq!(seen.len(), 1);
    }
}

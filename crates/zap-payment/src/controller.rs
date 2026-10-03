use std::sync::Arc;

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;
use zap_zcash::{PaymentReceipt, PaymentRequest, ZcashAdapter};

use crate::policy::{PolicyDecision, SpendingPolicy};
use crate::PaymentError;

/// Agent-safe invoice view — no wallet secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceView {
    pub invoice_id: String,
    pub offer_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub pay_to: String,
    pub memo_hint: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct PaymentOutcome {
    pub decision: PolicyDecision,
    pub receipt: Option<PaymentReceipt>,
}

/// Payment controller: the only path from agent intent to spending.
pub struct PaymentController<A> {
    adapter: Arc<A>,
    policy: Mutex<SpendingPolicy>,
    /// When true, `REQUIRE_APPROVAL` is treated as denied unless explicitly approved.
    auto_approve: bool,
}

impl<A: ZcashAdapter> PaymentController<A> {
    pub fn new(adapter: Arc<A>, policy: SpendingPolicy) -> Self {
        Self {
            adapter,
            policy: Mutex::new(policy),
            auto_approve: false,
        }
    }

    /// Demo helper: treat REQUIRE_APPROVAL as ALLOW (still never exposes keys).
    pub fn with_auto_approve(mut self, auto_approve: bool) -> Self {
        self.auto_approve = auto_approve;
        self
    }

    /// Sole agent-facing payment entrypoint.
    pub async fn request_payment(
        &self,
        invoice: &InvoiceView,
    ) -> Result<PaymentOutcome, PaymentError> {
        if invoice.currency != "ZEC" {
            return Err(PaymentError::InvalidInvoice("currency must be ZEC".into()));
        }

        let now = Utc::now();
        let mut policy = self.policy.lock().await;
        let decision = policy.evaluate(invoice.amount, now);
        info!(
            invoice = %invoice.invoice_id,
            amount = %invoice.amount,
            ?decision,
            "payment policy evaluated"
        );

        match decision {
            PolicyDecision::Deny => Err(PaymentError::Denied(format!(
                "amount {} exceeds policy",
                invoice.amount
            ))),
            PolicyDecision::RequireApproval if !self.auto_approve => {
                Err(PaymentError::ApprovalRequired(invoice.amount.to_string()))
            }
            PolicyDecision::Allow | PolicyDecision::RequireApproval => {
                let receipt = self
                    .adapter
                    .create_payment(PaymentRequest {
                        invoice_id: invoice.invoice_id.clone(),
                        amount: invoice.amount,
                        currency: invoice.currency.clone(),
                        pay_to: invoice.pay_to.clone(),
                        memo: invoice.memo_hint.clone(),
                    })
                    .await?;
                policy.record_spend(invoice.amount, now);
                Ok(PaymentOutcome {
                    decision,
                    receipt: Some(receipt),
                })
            }
        }
    }
}

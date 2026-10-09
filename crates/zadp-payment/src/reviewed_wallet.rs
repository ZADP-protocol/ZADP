//! Adapter for one pre-reviewed local wallet proposal. Never accepts wallet keys from agents.
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, process::Stdio};
use zadp_zcash::{
    ObservedPayment, PaymentFilter, PaymentReceipt, PaymentRequest, ScannedMessage, TxId,
    ZcashAdapter, ZcashError,
};

/// Operator-created binding. Paths are private local deployment configuration,
/// not parameters to the agent-facing request_payment method.
pub struct ReviewedWalletAdapter {
    pub seller_binary: PathBuf,
    pub transfer_config: PathBuf,
    pub preview: PathBuf,
    pub proposal: PathBuf,
    pub journal: PathBuf,
    pub approved_preview_sha256: String,
    pub expected_invoice_id: String,
    pub buyer_account: String,
    pub seller_account: String,
}
fn invalid(message: &str) -> ZcashError {
    ZcashError::InvalidPayload(message.into())
}
impl ReviewedWalletAdapter {
    pub fn validate_request(
        &self,
        request: &PaymentRequest,
        preview: &serde_json::Value,
    ) -> Result<u64, ZcashError> {
        if self.buyer_account.is_empty()
            || self.seller_account.is_empty()
            || self.buyer_account == self.seller_account
        {
            return Err(invalid("separate buyer/seller accounts required"));
        }
        let zat = request.amount * Decimal::from(100_000_000u64);
        let amount = zat
            .to_u64()
            .filter(|_| zat.fract().is_zero())
            .ok_or_else(|| invalid("integer zatoshi amount required"))?;
        if amount == 0
            || request.currency != "ZEC"
            || request.invoice_id != self.expected_invoice_id
            || preview["amount_zat"] != amount
            || preview["recipient"] != request.pay_to
            || preview["memo"] != request.memo
            || preview["broadcast"] != false
            || preview["expires_at"]
                .as_i64()
                .is_none_or(|expiry| expiry <= Utc::now().timestamp())
        {
            return Err(invalid("request differs from reviewed wallet preview"));
        }
        Ok(amount)
    }
}
#[async_trait]
impl ZcashAdapter for ReviewedWalletAdapter {
    async fn publish_message(&self, _: &[u8]) -> Result<TxId, ZcashError> {
        Err(invalid("buyer adapter cannot publish discovery"))
    }
    async fn scan_messages(&self, _: Option<&TxId>) -> Result<Vec<ScannedMessage>, ZcashError> {
        Err(invalid("use independent discovery client"))
    }
    async fn observe_payments(&self, _: PaymentFilter) -> Result<Vec<ObservedPayment>, ZcashError> {
        Err(invalid("seller observes payment independently"))
    }
    async fn create_payment(&self, request: PaymentRequest) -> Result<PaymentReceipt, ZcashError> {
        if self.journal.exists() {
            return Err(invalid(
                "payment already attempted; inspect journal, never retry automatically",
            ));
        }
        let bytes =
            std::fs::read(&self.preview).map_err(|_| invalid("reviewed preview unavailable"))?;
        if bytes.len() > 16384
            || hex::encode(Sha256::digest(&bytes)) != self.approved_preview_sha256
        {
            return Err(invalid("exact human approval required"));
        }
        let preview: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| invalid("invalid preview"))?;
        let amount = self.validate_request(&request, &preview)?;
        // Validate the protected account binding in the child too; model never gets config content.
        let config = std::fs::read(&self.transfer_config)
            .map_err(|_| invalid("local transfer configuration unavailable"))?;
        if config.len() > 16384 {
            return Err(invalid("oversized configuration"));
        }
        let config: serde_json::Value =
            serde_json::from_slice(&config).map_err(|_| invalid("invalid local configuration"))?;
        if config["account_uuid"] != self.buyer_account {
            return Err(invalid("wrong buyer wallet account"));
        }
        let output = tokio::process::Command::new(&self.seller_binary)
            .args([
                "transfer",
                "--execute",
                "--amount-zat",
                &amount.to_string(),
                "--approved-preview-sha256",
                &self.approved_preview_sha256,
            ])
            .arg("--config")
            .arg(&self.transfer_config)
            .arg("--preview")
            .arg(&self.preview)
            .arg("--proposal")
            .arg(&self.proposal)
            .arg("--journal")
            .arg(&self.journal)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|_| invalid("wallet command unavailable; inspect journal before retry"))?;
        if !output.status.success() || output.stdout.len() > 16384 {
            return Err(invalid(
                "payment result uncertain; inspect journal, do not retry",
            ));
        }
        let result: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| invalid("payment receipt unavailable"))?;
        let txid = result["txid"]
            .as_str()
            .filter(|id| id.len() == 64 && id.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| invalid("missing payment txid"))?;
        if result["stage"] != "broadcast_accepted" {
            return Err(invalid("payment not acknowledged"));
        }
        Ok(PaymentReceipt {
            txid: TxId(txid.into()),
            invoice_id: request.invoice_id,
            submitted_at: Utc::now(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approved_binding_and_separate_wallets_fail_closed() {
        let mut adapter = ReviewedWalletAdapter {
            seller_binary: PathBuf::new(),
            transfer_config: PathBuf::new(),
            preview: PathBuf::new(),
            proposal: PathBuf::new(),
            journal: PathBuf::new(),
            approved_preview_sha256: String::new(),
            expected_invoice_id: "order".into(),
            buyer_account: "buyer".into(),
            seller_account: "seller".into(),
        };
        let mut request = PaymentRequest {
            invoice_id: "order".into(),
            amount: Decimal::new(1, 8),
            currency: "ZEC".into(),
            pay_to: "recipient".into(),
            memo: serde_json::json!("exact memo"),
        };
        let preview = serde_json::json!({"amount_zat":1,"recipient":"recipient","memo":"exact memo","broadcast":false,"expires_at":Utc::now().timestamp()+900});
        assert_eq!(adapter.validate_request(&request, &preview).unwrap(), 1);
        request.pay_to = "other".into();
        assert!(adapter.validate_request(&request, &preview).is_err());
        request.pay_to = "recipient".into();
        request.memo = serde_json::json!("changed");
        assert!(adapter.validate_request(&request, &preview).is_err());
        request.memo = serde_json::json!("exact memo");
        request.amount = Decimal::new(1, 9);
        assert!(adapter.validate_request(&request, &preview).is_err());
        request.amount = Decimal::new(1, 8);
        adapter.buyer_account = "seller".into();
        assert!(adapter.validate_request(&request, &preview).is_err());
    }
}

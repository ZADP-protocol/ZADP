use std::collections::HashMap;
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;
use ulid::Ulid;
use zap_protocol::{Offer, OfferDraft, Price, SellerKeyPair};
use zap_zcash::{PaymentFilter, ZcashAdapter};

use crate::SellerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvoiceStatus {
    Open,
    Paid,
    Fulfilled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub invoice_id: String,
    pub offer_id: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
    pub currency: String,
    pub pay_to: String,
    pub memo_hint: serde_json::Value,
    pub expires_at: DateTime<Utc>,
    pub status: InvoiceStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateInvoiceRequest {
    pub offer_id: String,
    pub buyer_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceResult {
    pub invoice_id: String,
    pub output: String,
}

struct State {
    offers: HashMap<String, Offer>,
    invoices: HashMap<String, Invoice>,
}

pub struct SellerService<A> {
    keys: SellerKeyPair,
    adapter: Arc<A>,
    pay_to: String,
    state: Mutex<State>,
}

impl<A: ZcashAdapter> SellerService<A> {
    pub fn new(keys: SellerKeyPair, adapter: Arc<A>) -> Self {
        Self {
            pay_to: format!("mock:shielded:{}", keys.id.as_str()),
            keys,
            adapter,
            state: Mutex::new(State {
                offers: HashMap::new(),
                invoices: HashMap::new(),
            }),
        }
    }

    pub fn seller_id(&self) -> &zap_protocol::SellerId {
        &self.keys.id
    }

    pub async fn create_and_publish_offer(
        &self,
        capability: &str,
        title: &str,
        description: &str,
        amount: Decimal,
        endpoint: &str,
        ttl: Duration,
    ) -> Result<Offer, SellerError> {
        let mut nonce = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut nonce);
        let offer = OfferDraft::new_ulid(
            self.keys.id.clone(),
            capability,
            title,
            Price::zec(amount),
            endpoint,
            Utc::now() + ttl,
            B64.encode(nonce),
        )
        .with_description(description)
        .sign(&self.keys)?;

        let payload = serde_json::to_vec(&offer).map_err(|e| {
            SellerError::Protocol(zap_protocol::ProtocolError::Canonical(e.to_string()))
        })?;
        let txid = self.adapter.publish_message(&payload).await?;
        info!(
            offer_id = %offer.id,
            txid = %txid.0,
            title = %offer.title,
            amount = %offer.price.amount,
            "seller published offer"
        );

        self.state
            .lock()
            .await
            .offers
            .insert(offer.id.clone(), offer.clone());
        Ok(offer)
    }

    pub async fn create_invoice(&self, req: CreateInvoiceRequest) -> Result<Invoice, SellerError> {
        let state = self.state.lock().await;
        let offer = state
            .offers
            .get(&req.offer_id)
            .ok_or_else(|| SellerError::UnknownOffer(req.offer_id.clone()))?;
        if offer.expires_at <= Utc::now() {
            return Err(SellerError::OfferExpired);
        }

        let order_id = Ulid::new().to_string();
        let invoice = Invoice {
            invoice_id: Ulid::new().to_string(),
            offer_id: offer.id.clone(),
            amount: offer.price.amount,
            currency: offer.price.currency.clone(),
            pay_to: self.pay_to.clone(),
            memo_hint: serde_json::json!({
                "order_id": order_id,
                "buyer_ref": req.buyer_ref,
                "offer_id": offer.id,
            }),
            expires_at: Utc::now() + Duration::minutes(30),
            status: InvoiceStatus::Open,
        };
        drop(state);

        self.state
            .lock()
            .await
            .invoices
            .insert(invoice.invoice_id.clone(), invoice.clone());
        Ok(invoice)
    }

    pub async fn observe_and_mark_paid(&self, invoice_id: &str) -> Result<Invoice, SellerError> {
        let payments = self
            .adapter
            .observe_payments(PaymentFilter {
                invoice_id: Some(invoice_id.to_string()),
                pay_to: Some(self.pay_to.clone()),
            })
            .await?;
        if payments.is_empty() {
            return Err(SellerError::Unpaid(invoice_id.into()));
        }

        let mut state = self.state.lock().await;
        let invoice = state
            .invoices
            .get_mut(invoice_id)
            .ok_or_else(|| SellerError::UnknownInvoice(invoice_id.into()))?;
        if invoice.amount != payments[0].amount {
            return Err(SellerError::Unpaid(format!(
                "amount mismatch for {invoice_id}"
            )));
        }
        invoice.status = InvoiceStatus::Paid;
        info!(invoice_id, "payment received");
        Ok(invoice.clone())
    }

    pub async fn fulfill_mock_inference(
        &self,
        invoice_id: &str,
        prompt: &str,
    ) -> Result<ServiceResult, SellerError> {
        let mut state = self.state.lock().await;
        let invoice = state
            .invoices
            .get_mut(invoice_id)
            .ok_or_else(|| SellerError::UnknownInvoice(invoice_id.into()))?;
        if invoice.status != InvoiceStatus::Paid && invoice.status != InvoiceStatus::Fulfilled {
            return Err(SellerError::Unpaid(invoice_id.into()));
        }
        info!(invoice_id, "running mock inference service");
        let output = format!("mock-qwen: {}", prompt.trim());
        invoice.status = InvoiceStatus::Fulfilled;
        Ok(ServiceResult {
            invoice_id: invoice_id.into(),
            output,
        })
    }
}

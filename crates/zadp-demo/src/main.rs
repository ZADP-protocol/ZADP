//! ZADP local end-to-end demo (mock Zcash).

use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;
use tokio::net::TcpListener;
use tracing::info;
use zadp_api::{router, AppState};
use zadp_indexer::{InMemoryOfferRepository, Indexer};
use zadp_payment::{InvoiceView, PaymentController, PolicyConfig, PolicyDecision, SpendingPolicy};
use zadp_protocol::SellerKeyPair;
use zadp_seller::SellerService;
use zadp_zcash::MockZcashAdapter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=warn".into()),
        )
        .without_time()
        .init();

    println!("ZADP demo\n");
    println!(
        "Note: this demo mocks Zcash publish/scan/pay/observe; it does not access a wallet.\n"
    );

    let adapter = Arc::new(MockZcashAdapter::new());
    let repo = Arc::new(InMemoryOfferRepository::new());
    let indexer = Arc::new(Indexer::new(Arc::clone(&adapter), Arc::clone(&repo)));
    let seller_keys = SellerKeyPair::generate();
    let seller = Arc::new(SellerService::new(seller_keys, Arc::clone(&adapter)));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base = format!("http://{addr}");

    let state = AppState {
        indexer: Arc::clone(&indexer),
        seller: Arc::clone(&seller),
    };
    let app = router(state);
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(%error, "local demo server stopped");
        }
    });
    // Brief wait for accept loop.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 1) Seller publishes signed offer via mock shielded channel.
    let offer = seller
        .create_and_publish_offer(
            "llm.inference",
            "Qwen inference",
            "Mock LLM inference for demo",
            Decimal::new(1, 2),
            &base,
            ChronoDuration::hours(1),
        )
        .await?;
    println!("Seller:");
    println!(
        "  Published offer {} @ {} ZEC",
        offer.title,
        offer
            .price
            .as_ref()
            .map(|p| p.amount.to_string())
            .unwrap_or_else(|| "none (discovery only)".into())
    );
    println!("  offer_id={}", offer.id);
    println!();

    // 2) Indexer discovers + validates.
    let n = indexer.sync_once().await?;
    println!("Indexer:");
    println!("  Valid signature");
    println!("  Offer indexed ({n} new)");
    println!();

    // 3) Buyer queries REST.
    let client = reqwest::Client::new();
    let offers_url = format!("{base}/offers?capability=llm.inference");
    let resp = client
        .get(&offers_url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let offers = resp["offers"].as_array().context("offers array missing")?;
    println!("Buyer:");
    println!("  Found {} matching offer(s)", offers.len());
    if offers.is_empty() {
        bail!("expected at least one offer");
    }
    println!();

    // 4) Invoice
    let invoice_resp = client
        .post(format!("{base}/v1/invoices"))
        .json(&serde_json::json!({
            "offer_id": offer.id,
            "buyer_ref": "demo-buyer-1"
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let invoice_id = invoice_resp["invoice_id"]
        .as_str()
        .context("invoice_id")?
        .to_string();
    let amount: Decimal = invoice_resp["amount"].as_str().context("amount")?.parse()?;
    println!("Invoice:");
    println!("  {amount} ZEC");
    println!("  invoice_id={invoice_id}");
    println!();

    // 5) Payment policy (agent may only request_payment).
    let payment = PaymentController::new(
        Arc::clone(&adapter),
        SpendingPolicy::new(PolicyConfig::default()),
    );
    let view = InvoiceView {
        invoice_id: invoice_id.clone(),
        offer_id: offer.id.clone(),
        amount,
        currency: "ZEC".into(),
        pay_to: invoice_resp["pay_to"]
            .as_str()
            .context("invoice pay_to missing")?
            .to_string(),
        memo_hint: invoice_resp["memo_hint"].clone(),
    };
    let outcome = payment.request_payment(&view).await?;
    println!("Policy:");
    println!("  {:?}", outcome.decision);
    if outcome.decision != PolicyDecision::Allow
        && outcome.decision != PolicyDecision::RequireApproval
    {
        bail!("unexpected policy decision");
    }
    println!();

    let receipt = outcome.receipt.context("missing payment receipt")?;
    println!("Payment:");
    println!("  Shielded payment submitted");
    println!("  txid={}", receipt.txid.0);
    println!();

    // 6) Seller observes + fulfills.
    let result = client
        .post(format!("{base}/v1/fulfill"))
        .json(&serde_json::json!({
            "invoice_id": invoice_id,
            "prompt": "Say hello privately."
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    println!("Seller:");
    println!("  Payment received");
    println!("  Running service...");
    println!();
    println!("Result:");
    println!(
        "  \"{}\"",
        result["output"]
            .as_str()
            .context("service output missing")?
    );
    println!();

    info!(%addr, "demo complete");
    Ok(())
}

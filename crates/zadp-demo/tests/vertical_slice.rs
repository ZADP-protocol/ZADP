//! Basic integration test for the Phase 1 vertical slice.

use std::sync::Arc;
use std::time::Duration;

use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;
use tokio::net::TcpListener;
use zadp_api::{router, AppState};
use zadp_indexer::{InMemoryOfferRepository, Indexer};
use zadp_payment::{InvoiceView, PaymentController, PolicyConfig, PolicyDecision, SpendingPolicy};
use zadp_protocol::SellerKeyPair;
use zadp_seller::SellerService;
use zadp_zcash::MockZcashAdapter;

#[tokio::test]
async fn offer_to_fulfillment_slice() {
    let adapter = Arc::new(MockZcashAdapter::new());
    let repo = Arc::new(InMemoryOfferRepository::new());
    let indexer = Arc::new(Indexer::new(Arc::clone(&adapter), Arc::clone(&repo)));
    let seller = Arc::new(SellerService::new(
        SellerKeyPair::generate(),
        Arc::clone(&adapter),
    ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let app = router(AppState {
        indexer: Arc::clone(&indexer),
        seller: Arc::clone(&seller),
    });
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let offer = seller
        .create_and_publish_offer(
            "llm.inference",
            "Qwen inference",
            "integration",
            Decimal::new(1, 2),
            "https://seller.example",
            ChronoDuration::hours(1),
        )
        .await
        .unwrap();
    assert_eq!(indexer.sync_once().await.unwrap(), 1);

    let client = reqwest::Client::new();
    let offers: serde_json::Value = client
        .get(format!("{base}/offers?capability=llm.inference"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(offers["offers"].as_array().unwrap().len(), 1);

    let invoice: serde_json::Value = client
        .post(format!("{base}/v1/invoices"))
        .json(&serde_json::json!({"offer_id": offer.id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let payment = PaymentController::new(
        Arc::clone(&adapter),
        SpendingPolicy::new(PolicyConfig::default()),
    );
    let outcome = payment
        .request_payment(&InvoiceView {
            invoice_id: invoice["invoice_id"].as_str().unwrap().into(),
            offer_id: offer.id,
            amount: invoice["amount"].as_str().unwrap().parse().unwrap(),
            currency: "ZEC".into(),
            pay_to: invoice["pay_to"].as_str().unwrap().into(),
            memo_hint: invoice["memo_hint"].clone(),
        })
        .await
        .unwrap();
    assert_eq!(outcome.decision, PolicyDecision::Allow);

    let result: serde_json::Value = client
        .post(format!("{base}/v1/fulfill"))
        .json(&serde_json::json!({
            "invoice_id": invoice["invoice_id"],
            "prompt": "ping"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(result["output"].as_str().unwrap().contains("ping"));
}

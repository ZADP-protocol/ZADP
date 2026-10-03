use std::sync::Arc;

use chrono::Utc;
use tracing::{info, warn};
use zadp_protocol::{verify_offer, Offer};
use zadp_zcash::{TxId, ZcashAdapter};

use crate::repo::{OfferRecord, OfferRepository};
use crate::IndexerError;

pub struct Indexer<A, R> {
    adapter: Arc<A>,
    repo: Arc<R>,
    last_txid: tokio::sync::Mutex<Option<TxId>>,
}

impl<A, R> Indexer<A, R>
where
    A: ZcashAdapter,
    R: OfferRepository,
{
    pub fn new(adapter: Arc<A>, repo: Arc<R>) -> Self {
        Self {
            adapter,
            repo,
            last_txid: tokio::sync::Mutex::new(None),
        }
    }

    pub fn repository(&self) -> Arc<R> {
        Arc::clone(&self.repo)
    }

    /// Scan board messages, validate offers, store first-wins.
    pub async fn sync_once(&self) -> Result<usize, IndexerError> {
        let after = self.last_txid.lock().await.clone();
        let messages = self.adapter.scan_messages(after.as_ref()).await?;
        let mut indexed = 0usize;
        let mut newest = after;

        for msg in messages {
            newest = Some(msg.txid.clone());
            match decode_offer(&msg.payload) {
                Ok(offer) => match self.ingest(offer, msg.txid).await {
                    Ok(true) => indexed += 1,
                    Ok(false) => {}
                    Err(e) => warn!(error = %e, "offer rejected"),
                },
                Err(e) => warn!(error = %e, "payload skipped"),
            }
        }

        *self.last_txid.lock().await = newest;
        Ok(indexed)
    }

    async fn ingest(&self, offer: Offer, txid: TxId) -> Result<bool, IndexerError> {
        verify_offer(&offer, Utc::now())?;
        info!(
            offer_id = %offer.id,
            seller = %offer.seller,
            "valid signature; indexing offer"
        );
        let inserted = self
            .repo
            .upsert_first_wins(OfferRecord {
                offer,
                source_txid: txid,
                indexed_at: Utc::now(),
                chain: None,
            })
            .await?;
        Ok(inserted)
    }
}

fn decode_offer(payload: &[u8]) -> Result<Offer, IndexerError> {
    serde_json::from_slice(payload).map_err(|e| IndexerError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use chrono::Duration;
    use rust_decimal::Decimal;
    use zadp_protocol::{OfferDraft, Price, SellerKeyPair};
    use zadp_zcash::MockZcashAdapter;

    use crate::InMemoryOfferRepository;

    #[tokio::test]
    async fn indexes_signed_offer_from_mock_board() {
        let adapter = Arc::new(MockZcashAdapter::new());
        let repo = Arc::new(InMemoryOfferRepository::new());
        let indexer = Indexer::new(Arc::clone(&adapter), Arc::clone(&repo));

        let keys = SellerKeyPair::generate();
        let offer = OfferDraft::new_ulid(
            keys.id.clone(),
            "llm.inference",
            "Qwen inference",
            Price::zec(Decimal::new(1, 2)),
            "http://127.0.0.1:8080",
            Utc::now() + Duration::hours(1),
            B64.encode([3u8; 16]),
        )
        .sign(&keys)
        .unwrap();

        let payload = serde_json::to_vec(&offer).unwrap();
        adapter.publish_message(&payload).await.unwrap();

        let n = indexer.sync_once().await.unwrap();
        assert_eq!(n, 1);
        let listed = repo.list(Some("llm.inference")).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].offer.title, "Qwen inference");
    }
}

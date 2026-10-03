use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use zap_protocol::Offer;
use zap_zcash::TxId;

use crate::IndexerError;

#[derive(Debug, Clone)]
pub struct OfferRecord {
    pub offer: Offer,
    pub source_txid: TxId,
    pub indexed_at: DateTime<Utc>,
}

#[async_trait]
pub trait OfferRepository: Send + Sync {
    async fn upsert_first_wins(&self, record: OfferRecord) -> Result<bool, IndexerError>;
    async fn list(&self, capability: Option<&str>) -> Result<Vec<OfferRecord>, IndexerError>;
    async fn get(&self, id: &str) -> Result<Option<OfferRecord>, IndexerError>;
}

#[derive(Default, Clone)]
pub struct InMemoryOfferRepository {
    inner: Arc<RwLock<HashMap<String, OfferRecord>>>,
}

impl InMemoryOfferRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OfferRepository for InMemoryOfferRepository {
    async fn upsert_first_wins(&self, record: OfferRecord) -> Result<bool, IndexerError> {
        let mut map = self.inner.write().await;
        if let Some(existing) = map.get(&record.offer.id) {
            if existing.offer != record.offer {
                return Err(IndexerError::Conflict(record.offer.id));
            }
            return Ok(false);
        }
        map.insert(record.offer.id.clone(), record);
        Ok(true)
    }

    async fn list(&self, capability: Option<&str>) -> Result<Vec<OfferRecord>, IndexerError> {
        let map = self.inner.read().await;
        let mut out: Vec<_> = map
            .values()
            .filter(|r| capability.is_none_or(|c| r.offer.capability == c))
            .cloned()
            .collect();
        out.sort_by(|a, b| a.offer.id.cmp(&b.offer.id));
        Ok(out)
    }

    async fn get(&self, id: &str) -> Result<Option<OfferRecord>, IndexerError> {
        Ok(self.inner.read().await.get(id).cloned())
    }
}

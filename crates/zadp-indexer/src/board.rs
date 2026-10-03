use crate::{IndexerError, SqliteOfferRepository};
use std::sync::Arc;
use zadp_zcash::BoardSource;

pub struct BoardIndexer<S> {
    source: Arc<S>,
    repository: Arc<SqliteOfferRepository>,
    sync_lock: tokio::sync::Mutex<()>,
}
impl<S: BoardSource> BoardIndexer<S> {
    pub fn new(source: Arc<S>, repository: Arc<SqliteOfferRepository>) -> Self {
        Self {
            source,
            repository,
            sync_lock: tokio::sync::Mutex::new(()),
        }
    }
    pub fn repository(&self) -> Arc<SqliteOfferRepository> {
        Arc::clone(&self.repository)
    }
    /// Resume atomically committed blocks, finding the common ancestor on any reorg.
    pub async fn sync_once(&self, max_blocks: u32) -> Result<usize, IndexerError> {
        let _guard = self.sync_lock.lock().await;
        let tip = self.source.tip_height().await?;
        let birthday = self.repository.board().board.birthday_height;
        if tip < birthday {
            return Err(IndexerError::State(
                "chain tip precedes board birthday".into(),
            ));
        }
        if let Some(cursor) = self.repository.cursor()? {
            let mut height = cursor.height.min(tip);
            loop {
                if let Some(stored) = self.repository.header(height)? {
                    if self.source.header(height).await?.hash == stored.hash {
                        self.repository.rewind(Some(height))?;
                        break;
                    }
                }
                if height <= birthday {
                    self.repository.rewind(None)?;
                    break;
                }
                height -= 1;
            }
        }
        let start = self
            .repository
            .cursor()?
            .map_or(birthday, |c| c.height.saturating_add(1));
        let end = tip.min(start.saturating_add(max_blocks.saturating_sub(1)));
        let mut indexed = 0;
        if max_blocks == 0 {
            return Ok(0);
        }
        for height in start..=end {
            indexed += self
                .repository
                .apply_block(self.source.block(height).await?)?;
        }
        Ok(indexed)
    }
}

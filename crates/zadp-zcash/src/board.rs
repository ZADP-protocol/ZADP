//! Read-only discovery transport. This interface has no spending or broadcast method.
use crate::{TxId, ZcashError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u32,
    /// Hex of protocol-order bytes, deliberately not explorer display order.
    pub hash: String,
    pub prev_hash: String,
    pub time: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardOutput {
    pub txid: TxId,
    pub transaction_index: u64,
    pub output_index: u32,
    pub pool: String,
    pub value_zat: u64,
    pub memo: Vec<u8>,
}
#[derive(Debug, Clone)]
pub struct BoardBlock {
    pub header: BlockHeader,
    pub outputs: Vec<BoardOutput>,
}

#[async_trait]
pub trait BoardSource: Send + Sync {
    async fn tip_height(&self) -> Result<u32, ZcashError>;
    async fn header(&self, height: u32) -> Result<BlockHeader, ZcashError>;
    async fn block(&self, height: u32) -> Result<BoardBlock, ZcashError>;
}

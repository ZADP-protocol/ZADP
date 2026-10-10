//! Private incoming-only payment observation using the existing authenticated scanner.
//! The scope descriptor is not a hosted board. No wallet RPC, keys, or broadcasts.
use crate::{Config, PaymentEvidence, Snapshot};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use zadp_protocol::BoardDescriptor;
use zadp_seller::local::read_protected;
use zadp_zcash::{BlockHeader, BoardSource, RealZcashScanner};

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    binding: String,
    headers: Vec<BlockHeader>,
    payments: Vec<PaymentEvidence>,
}

pub async fn load(
    config: &Config,
    url: &str,
    descriptor_path: &Path,
    path: &Path,
) -> Result<Snapshot> {
    // One scan at a time per process; the HTTP observer and warm-up task share it.
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _lock = LOCK.lock().await;
    ensure!(
        path != config.database && path != descriptor_path,
        "separate observation cache required"
    );
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing cache parent"))?;
    let info = fs::symlink_metadata(parent)?;
    let owner = fs::metadata("/proc/self")?.uid();
    ensure!(
        info.is_dir() && info.uid() == owner && info.mode() & 0o077 == 0,
        "protected cache parent required"
    );
    let descriptor: BoardDescriptor = serde_json::from_slice(&read_protected(descriptor_path)?)?;
    descriptor.validate(config.network)?;
    ensure!(
        descriptor.board.address == config.recipient,
        "viewing scope recipient mismatch"
    );
    let binding = format!(
        "{}:{}:{}",
        descriptor.fingerprint()?,
        config.seller_account,
        config.recipient
    );
    let mut cache = if path.exists() {
        let info = fs::symlink_metadata(path)?;
        ensure!(
            info.is_file()
                && info.uid() == owner
                && info.mode() & 0o077 == 0
                && info.len() <= 16 * 1024 * 1024,
            "invalid private cache"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 16 * 1024 * 1024, "cache too large");
        let cache: Cache = serde_json::from_slice(&bytes)?;
        ensure!(cache.binding == binding, "observation scope mismatch");
        cache
    } else {
        Cache {
            binding,
            ..Cache::default()
        }
    };
    let scanner = RealZcashScanner::connect(&descriptor, url).await?;
    let snapshot = scan_batch(
        &scanner,
        &mut cache,
        &config.seller_account,
        &config.recipient,
        descriptor.board.birthday_height,
    )
    .await?;
    let temp = path.with_extension("pending");
    // Interrupted writes never replace the last complete cache. Existing temp fails closed.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    let bytes = serde_json::to_vec(&cache)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "cache capacity reached; operator review required"
    );
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(snapshot)
}

async fn scan_batch(
    source: &dyn BoardSource,
    cache: &mut Cache,
    account: &str,
    recipient: &str,
    birthday: u32,
) -> Result<Snapshot> {
    let tip = source.tip_height().await?;
    ensure!(tip >= birthday, "tip precedes scope birthday");
    let had_headers = !cache.headers.is_empty();
    // Find the shared ancestor before making any payment eligible again.
    while let Some(last) = cache.headers.last() {
        if last.height <= tip && source.header(last.height).await?.hash == last.hash {
            break;
        }
        cache.headers.pop();
    }
    ensure!(
        !had_headers || !cache.headers.is_empty(),
        "no common ancestor in retained cache; manual rebuild required"
    );
    let last = cache.headers.last().map(|h| h.height);
    cache
        .payments
        .retain(|p| last.is_some_and(|h| p.mined_height <= u64::from(h)));
    let start = last.map_or(birthday, |h| h.saturating_add(1));
    let end = tip.min(start.saturating_add(127));
    for height in start..=end {
        let block = source.block(height).await?;
        ensure!(block.header.height == height, "wrong observation height");
        if let Some(previous) = cache.headers.last() {
            ensure!(
                block.header.prev_hash == previous.hash,
                "fork changed during observation"
            );
        }
        for output in block.outputs {
            // Only invoice memos are retained; other incoming data never enters order storage.
            let bytes = output.memo.split(|b| *b == 0).next().unwrap_or_default();
            let Ok(memo) = std::str::from_utf8(bytes) else {
                continue;
            };
            if !memo.starts_with("ZADP-COMMERCE/1:") {
                continue;
            }
            cache.payments.push(PaymentEvidence {
                txid: output.txid.0,
                pool: output.pool,
                output_index: output.output_index,
                account: account.into(),
                recipient: recipient.into(),
                amount_zat: output.value_zat,
                memo: memo.into(),
                is_change: false,
                mined_height: u64::from(height),
                block_time: block.header.time.timestamp(),
            });
        }
        cache.headers.push(block.header);
    }
    if cache.headers.len() > 4096 {
        cache.headers.drain(..cache.headers.len() - 4096);
    }
    let head = cache
        .headers
        .last()
        .ok_or_else(|| anyhow::anyhow!("no scanned header"))?;
    let synchronized = head.height == tip
        && source.tip_height().await? == tip
        && source.header(tip).await?.hash == head.hash;
    Ok(Snapshot {
        synchronized,
        tip: u64::from(tip),
        payments: cache.payments.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zadp_zcash::{BoardBlock, BoardOutput, TxId, ZcashError};
    struct Source {
        fork: bool,
        tip: u32,
    }
    impl Source {
        fn h(&self, height: u32) -> BlockHeader {
            BlockHeader {
                height,
                hash: format!("{}-{height}", self.fork && height >= 2),
                prev_hash: format!("{}-{}", self.fork && height > 2, height - 1),
                time: chrono::Utc::now(),
            }
        }
    }
    #[async_trait::async_trait]
    impl BoardSource for Source {
        async fn tip_height(&self) -> Result<u32, ZcashError> {
            Ok(self.tip)
        }
        async fn header(&self, h: u32) -> Result<BlockHeader, ZcashError> {
            Ok(self.h(h))
        }
        async fn block(&self, h: u32) -> Result<BoardBlock, ZcashError> {
            Ok(BoardBlock {
                header: self.h(h),
                outputs: if h == 2 {
                    vec![BoardOutput {
                        txid: TxId(if self.fork { "bb" } else { "aa" }.repeat(32)),
                        transaction_index: 0,
                        output_index: 0,
                        pool: "orchard".into(),
                        value_zat: 1000,
                        memo: b"ZADP-COMMERCE/1:invoice\0".to_vec(),
                    }]
                } else {
                    vec![]
                },
            })
        }
    }
    // Use the existing independent protected-config fixture construction.
    #[tokio::test]
    async fn bounded_catchup_and_fork_replacement() {
        let mut cache = Cache::default();
        let first = scan_batch(
            &Source {
                fork: false,
                tip: 129,
            },
            &mut cache,
            "seller",
            "recipient",
            1,
        )
        .await
        .unwrap();
        assert!(!first.synchronized);
        assert_eq!(cache.headers.len(), 128);
        let second = scan_batch(
            &Source {
                fork: false,
                tip: 129,
            },
            &mut cache,
            "seller",
            "recipient",
            1,
        )
        .await
        .unwrap();
        assert!(second.synchronized);
        assert_eq!(second.payments.len(), 1);
        let replacement = scan_batch(
            &Source { fork: true, tip: 2 },
            &mut cache,
            "seller",
            "recipient",
            1,
        )
        .await
        .unwrap();
        assert!(replacement.synchronized);
        assert_eq!(replacement.payments.len(), 1);
        assert_eq!(replacement.payments[0].txid, "bb".repeat(32));
    }
}

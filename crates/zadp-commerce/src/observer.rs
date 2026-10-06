//! Trusted observation boundary. No signer, broadcaster, or HTTP payment callback.
use crate::{Config, ObserverConfig, PaymentEvidence, Snapshot};
use anyhow::Result;
use serde_json::{json, Value};
use zadp_seller::{local::read_protected, publication::LocalWallet};

pub struct Observer {
    config: Config,
    wallet: Option<LocalWallet>,
}
impl Observer {
    pub fn new(config: Config) -> Result<Self> {
        let wallet = match &config.observer {
            ObserverConfig::Zallet { rpc_url, auth_file } => {
                Some(LocalWallet::connect(rpc_url, auth_file)?)
            }
            ObserverConfig::Mock { .. } => None,
        };
        Ok(Self { config, wallet })
    }
    pub async fn snapshot(&self) -> Snapshot {
        // Suppress credentials, transport diagnostics and full wallet responses.
        match tokio::time::timeout(std::time::Duration::from_secs(20), self.load()).await {
            Ok(Ok(snapshot)) => snapshot,
            _ => Snapshot::default(),
        }
    }
    async fn read_rpc(&self, method: &str, params: Value) -> Result<Value> {
        anyhow::ensure!(
            ["getwalletstatus", "z_listtransactions"].contains(&method),
            "not a read-only observation method"
        );
        self.wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("observer unavailable"))?
            .rpc(method, params)
            .await
    }
    async fn load(&self) -> Result<Snapshot> {
        if let ObserverConfig::Mock { snapshot_file } = &self.config.observer {
            // Protected local operator input; deliberately never an HTTP route.
            return Ok(serde_json::from_slice(&read_protected(snapshot_file)?)?);
        }
        let first = self.read_rpc("getwalletstatus", json!([])).await?;
        let tip = first["node_tip"]["height"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("no tip"))?;
        anyhow::ensure!(
            first["locked"] == false && first["fully_synced_height"].as_u64() == Some(tip),
            "sync in progress"
        );
        let mut transactions = Vec::new();
        let mut complete = false;
        for offset in (0..1000).step_by(100) {
            let page = self
                .read_rpc(
                    "z_listtransactions",
                    json!([self.config.seller_account, null, null, offset, 100]),
                )
                .await?;
            let page = page
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid page"))?;
            anyhow::ensure!(page.len() <= 100, "invalid pagination");
            transactions.extend(page.iter().cloned());
            if page.len() < 100 {
                complete = true;
                break;
            }
        }
        anyhow::ensure!(complete, "history truncated");
        let last = self.read_rpc("getwalletstatus", json!([])).await?;
        anyhow::ensure!(
            last["locked"] == false
                && last["node_tip"] == first["node_tip"]
                && last["fully_synced_height"].as_u64() == Some(tip),
            "unstable observation"
        );
        Ok(snapshot_from_transactions(&transactions, tip))
    }
}
pub fn snapshot_from_transactions(transactions: &[Value], tip: u64) -> Snapshot {
    let mut payments = Vec::new();
    for tx in transactions {
        let Some(account) = tx["account_uuid"].as_str() else {
            continue;
        };
        let Some(txid) = tx["txid"].as_str() else {
            continue;
        };
        let Some(height) = tx["mined_height"].as_u64() else {
            continue;
        };
        let Some(time) = tx["block_time"].as_i64() else {
            continue;
        };
        let Some(outputs) = tx["outputs"].as_array() else {
            continue;
        };
        for output in outputs {
            let fields = (
                output["to_account"].as_str(),
                output["to_address"].as_str(),
                output["pool"].as_str(),
                output["memo"].as_str(),
                output["value"].as_u64(),
                output["output_index"].as_u64(),
                output["is_change"].as_bool(),
            );
            if let (
                Some(to_account),
                Some(recipient),
                Some(pool),
                Some(memo),
                Some(amount),
                Some(index),
                Some(change),
            ) = fields
            {
                if to_account != account || index > u64::from(u32::MAX) {
                    continue;
                }
                payments.push(PaymentEvidence {
                    txid: txid.into(),
                    pool: pool.into(),
                    output_index: index as u32,
                    account: account.into(),
                    recipient: recipient.into(),
                    amount_zat: amount,
                    memo: memo.into(),
                    is_change: change,
                    mined_height: height,
                    block_time: time,
                });
            }
        }
    }
    Snapshot {
        synchronized: true,
        tip,
        payments,
    }
}

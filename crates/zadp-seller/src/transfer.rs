//! Explicit local shielded transfer. No HTTP route, timer, seed export or implicit approval.
use crate::{
    local::read_protected,
    publication::{inspect_fee, LocalWallet},
};
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use zadp_protocol::ZcashNetwork;
use zcash_client_backend::proto::service::{BlockId, ChainSpec};
use zcash_keys::address::Address;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId};

#[derive(clap::Args)]
pub struct Transfer {
    #[arg(long)]
    pub config: PathBuf,
    /// Transfer amount in integer zatoshis. Includes neither fee nor change.
    #[arg(long)]
    pub amount_zat: u64,
    #[arg(long)]
    pub preview: PathBuf,
    /// Protected unsigned PCZT; never publish this file.
    #[arg(long)]
    pub proposal: PathBuf,
    #[arg(long)]
    pub journal: Option<PathBuf>,
    /// Exact SHA256 of the reviewed public preview; required with execute.
    #[arg(long)]
    pub approved_preview_sha256: Option<String>,
    #[arg(long)]
    pub execute: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub network: ZcashNetwork,
    pub wallet_rpc: String,
    pub wallet_auth_file: PathBuf,
    pub account_uuid: String,
    pub recipient: String,
    pub memo: String,
    pub lightwalletd: String,
    pub max_fee_zat: u64,
}
fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn save(path: &Path, value: &Value, new: bool) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).mode(0o600).custom_flags(libc::O_NOFOLLOW);
    if new {
        o.create_new(true);
    } else {
        o.truncate(true);
    }
    let mut f = o.open(path)?;
    f.write_all(&serde_json::to_vec(value)?)?;
    f.sync_all()?;
    Ok(())
}
fn pczt(v: &Value) -> Result<&str> {
    v["pczt"].as_str().context("missing protected PCZT")
}
pub fn validate_config(c: &Config, amount: u64) -> Result<()> {
    ensure!(
        amount > 0 && amount <= 2_100_000_000_000_000,
        "invalid amount"
    );
    ensure!(
        c.max_fee_zat > 0 && c.max_fee_zat <= 100_000,
        "invalid explicit fee cap"
    );
    ensure!(c.memo.len() <= 512, "memo too long");
    let Some(Address::Unified(ua)) = Address::decode(&c.network.parameters(), &c.recipient) else {
        anyhow::bail!("network-correct Unified Address required")
    };
    ensure!(ua.orchard().is_some(), "shielded Orchard receiver required");
    Ok(())
}
pub async fn transfer(a: Transfer) -> Result<()> {
    let config_bytes = read_protected(&a.config)?;
    let c: Config = serde_json::from_slice(&config_bytes)?;
    validate_config(&c, a.amount_zat)?;
    let wallet = LocalWallet::connect(&c.wallet_rpc, &c.wallet_auth_file)?;
    let mut client = zadp_zcash::connect_lightwalletd(&c.lightwalletd, c.network).await?;
    let status = wallet.rpc("getwalletstatus", json!([])).await?;
    ensure!(
        status["locked"] == false && status["fully_synced_height"] == status["node_tip"]["height"],
        "wallet sync locked"
    );
    let height = status["node_tip"]["height"]
        .as_u64()
        .context("missing wallet tip")?;
    let mut hash = client
        .get_block(BlockId {
            height,
            hash: vec![],
        })
        .await?
        .into_inner()
        .hash;
    hash.reverse();
    ensure!(
        status["node_tip"]["blockhash"].as_str() == Some(hex::encode(hash).as_str()),
        "wallet/network mismatch"
    );
    let balances = wallet.rpc("z_getbalances", json!([3])).await?;
    let balance = balances["accounts"]
        .as_array()
        .context("missing accounts")?
        .iter()
        .find(|b| b["account_uuid"] == c.account_uuid)
        .context("configured account missing")?["total"]["spendable"]["valueZat"]
        .as_u64()
        .unwrap_or(0);
    if !a.execute {
        ensure!(
            a.journal.is_none() && a.approved_preview_sha256.is_none(),
            "approval flags require execute"
        );
        ensure!(
            balance >= a.amount_zat,
            "insufficient confirmed spendable balance"
        );
        let amount: Value = serde_json::from_str(
            &(rust_decimal::Decimal::from(a.amount_zat)
                / rust_decimal::Decimal::from(100_000_000u64))
            .to_string(),
        )?;
        let created=wallet.rpc("pczt_create",json!([c.account_uuid,[{"address":c.recipient,"amount":amount,"memo":hex::encode(c.memo.as_bytes())}],null,"FullPrivacy","orchard"])).await?;
        let inspected = wallet.rpc("pczt_inspect", json!([pczt(&created)?])).await?;
        let fee = inspect_fee(&inspected, &c.recipient, a.amount_zat)?;
        ensure!(
            fee <= c.max_fee_zat
                && a.amount_zat
                    .checked_add(fee)
                    .is_some_and(|total| total <= balance),
            "fee cap or balance exceeded"
        );
        let now = Utc::now().timestamp();
        let preview = json!({"kind":"shielded-transfer-preview","network":c.network,"recipient":c.recipient,"amount_zat":a.amount_zat,"memo":c.memo,"network_fee_zat":fee,"total_zat":a.amount_zat+fee,"confirmed_spendable_zat":balance,"remaining_zat":balance-a.amount_zat-fee,"config_sha256":digest(&config_bytes),"proposal_sha256":digest(pczt(&created)?.as_bytes()),"created_at":now,"expires_at":now+900,"broadcast":false});
        save(&a.proposal, &json!({"pczt":pczt(&created)?}), true)?;
        save(&a.preview, &preview, true)?;
        println!(
            "{}",
            json!({"preview":preview,"approval_sha256":digest(&read_protected(&a.preview)?)})
        );
        return Ok(());
    }
    let journal = a
        .journal
        .context("execution requires new recovery journal")?;
    ensure!(!journal.exists(), "journal exists; no automatic retry");
    let bytes = read_protected(&a.preview)?;
    ensure!(
        a.approved_preview_sha256.as_deref() == Some(digest(&bytes).as_str()),
        "exact preview approval required"
    );
    let p: Value = serde_json::from_slice(&bytes)?;
    let proposal: Value = serde_json::from_slice(&read_protected(&a.proposal)?)?;
    ensure!(
        p["config_sha256"] == digest(&config_bytes)
            && p["proposal_sha256"] == digest(pczt(&proposal)?.as_bytes())
            && p["amount_zat"] == a.amount_zat
            && p["recipient"] == c.recipient
            && p["memo"] == c.memo
            && p["network"] == serde_json::to_value(c.network)?
            && p["broadcast"] == false,
        "approved transfer changed"
    );
    ensure!(
        p["expires_at"]
            .as_i64()
            .is_some_and(|expiry| expiry > Utc::now().timestamp()),
        "preview expired"
    );
    let inspected = wallet
        .rpc("pczt_inspect", json!([pczt(&proposal)?]))
        .await?;
    let fee = inspect_fee(&inspected, &c.recipient, a.amount_zat)?;
    ensure!(
        p["network_fee_zat"] == fee
            && fee <= c.max_fee_zat
            && a.amount_zat
                .checked_add(fee)
                .is_some_and(|total| total <= balance),
        "proposal fee/balance changed"
    );
    let mut j = json!({"stage":"signing_started","preview_sha256":digest(&bytes),"network":c.network,"recipient":c.recipient,"amount_zat":a.amount_zat,"fee_zat":fee});
    save(&journal, &j, true)?;
    let proved = wallet.rpc("pczt_prove", json!([pczt(&proposal)?])).await?;
    let signed = wallet
        .rpc("pczt_sign", json!([pczt(&proved)?, "FullPrivacy", true]))
        .await?;
    let extracted = wallet.rpc("pczt_extract", json!([pczt(&signed)?])).await?;
    ensure!(
        extracted["stored"] == true,
        "wallet must record transaction"
    );
    let raw = hex::decode(extracted["hex"].as_str().context("missing transaction")?)?;
    let tip = client.get_latest_block(ChainSpec {}).await?.into_inner();
    let branch = BranchId::for_height(
        &c.network.parameters(),
        BlockHeight::from_u32(u32::try_from(tip.height)?),
    );
    let mut reader = std::io::Cursor::new(&raw);
    let tx = Transaction::read(&mut reader, branch)?;
    ensure!(
        reader.position() == raw.len() as u64 && tx.consensus_branch_id() == branch,
        "transaction branch changed"
    );
    j["txid"] = tx.txid().to_string().into();
    j["stage"] = "broadcast_attempting".into();
    save(&journal, &j, false)?;
    zadp_zcash::broadcast_transaction(&mut client, raw).await?;
    j["stage"] = "broadcast_accepted".into();
    save(&journal, &j, false)?;
    println!("{}", j);
    Ok(())
}

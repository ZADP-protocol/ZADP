//! Explicit operator-approved self-payment test. No HTTP spending route.
use crate::{
    local::read_protected,
    publication::{inspect_fee, LocalWallet},
};
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use zadp_protocol::ZcashNetwork;
use zcash_client_backend::proto::service::{BlockId, ChainSpec};
use zcash_keys::{address::Address, keys::UnifiedIncomingViewingKey};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId};
use zeroize::Zeroizing;

#[derive(clap::Args)]
pub struct Purchase {
    /// Existing protected local operator configuration. Never copied to receipts.
    #[arg(long)]
    pub operator_config: PathBuf,
    /// Protected invoice JSON from the fixed local service.
    #[arg(long)]
    pub invoice: PathBuf,
    /// New protected recovery journal. Existing journals always block retries.
    #[arg(long)]
    pub journal: PathBuf,
    /// Maximum permitted network fee, independently of the invoice amount.
    #[arg(long)]
    pub max_fee_zat: u64,
    /// Without this flag, build and inspect only; never sign or broadcast.
    #[arg(long)]
    pub execute: bool,
}
#[derive(Deserialize)]
pub struct Invoice {
    pub invoice_id: String,
    pub offer_id: String,
    pub amount_zat: u64,
    pub currency: String,
    pub pay_to: String,
    pub memo: String,
    pub expires_at: i64,
    pub network: String,
    pub status: String,
    pub local_unpublished_test: bool,
}
#[derive(Deserialize)]
pub struct Service {
    pub offer_id: String,
    pub price_zat: u64,
}
#[derive(Deserialize)]
pub struct Config {
    pub network: String,
    pub receive_address: String,
    pub account_uuid: String,
    pub wallet_rpc: String,
    pub wallet_auth_file: PathBuf,
    pub lightwalletd: String,
    pub text_service: Service,
}
impl Invoice {
    pub fn validate(&self, config: &Config, now: i64) -> Result<ZcashNetwork> {
        ensure!(
            self.invoice_id.len() == 32
                && self
                    .invoice_id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "invalid invoice ID"
        );
        ensure!(
            self.network == config.network && self.currency == "ZEC",
            "invoice currency/network mismatch"
        );
        let network = match config.network.as_str() {
            "mainnet" => ZcashNetwork::Mainnet,
            "testnet" => ZcashNetwork::Testnet,
            _ => anyhow::bail!("unsupported network"),
        };
        ensure!(
            self.local_unpublished_test && self.status == "awaiting_payment",
            "only local pending self-payment tests are supported"
        );
        ensure!(
            self.offer_id == config.text_service.offer_id
                && self.amount_zat == config.text_service.price_zat
                && self.amount_zat > 0
                && self.amount_zat <= 2_100_000_000_000_000,
            "invoice offer/amount mismatch"
        );
        ensure!(
            self.pay_to == config.receive_address,
            "self-payment receiver mismatch"
        );
        ensure!(
            self.memo == format!("ZADP-PAY/1:{}", self.invoice_id),
            "invoice memo mismatch"
        );
        ensure!(
            self.expires_at > now + 120 && self.expires_at <= now + 1800,
            "invoice expired or outside permitted time window"
        );
        ensure!(
            config.account_uuid.len() == 36
                && config.account_uuid.bytes().enumerate().all(|(i, c)| {
                    if [8, 13, 18, 23].contains(&i) {
                        c == b'-'
                    } else {
                        c.is_ascii_hexdigit()
                    }
                }),
            "invalid account identifier"
        );
        let Some(Address::Unified(ua)) = Address::decode(&network.parameters(), &self.pay_to)
        else {
            anyhow::bail!("invoice must use a network-correct unified address")
        };
        ensure!(
            ua.orchard().is_some(),
            "invoice must support Orchard-family shielded payments"
        );
        Ok(network)
    }
}
fn pczt(value: &Value) -> Result<&str> {
    value["pczt"].as_str().context("missing wallet PCZT")
}
fn write_journal(path: &Path, value: &Value, new: bool) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if new {
        options.create_new(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .context("cannot write recovery journal; never retry a possibly submitted payment")?;
    file.write_all(serde_json::to_vec(value)?.as_slice())?;
    file.sync_all()?;
    Ok(())
}
pub async fn purchase(args: Purchase) -> Result<()> {
    ensure!(
        !args.journal.exists(),
        "recovery journal already exists; inspect status before any retry"
    );
    let config: Config = serde_json::from_slice(&read_protected(&args.operator_config)?)
        .context("invalid protected operator configuration")?;
    let invoice: Invoice = serde_json::from_slice(&read_protected(&args.invoice)?)
        .context("invalid protected invoice")?;
    let network = invoice.validate(&config, Utc::now().timestamp())?;
    ensure!(
        args.max_fee_zat > 0 && args.max_fee_zat <= 100_000,
        "fee cap must be between 1 and 100000 zat"
    );
    let wallet = LocalWallet::connect(&config.wallet_rpc, &config.wallet_auth_file)?;
    let mut client = zadp_zcash::connect_lightwalletd(&config.lightwalletd, network).await?;
    let status = wallet.rpc("getwalletstatus", json!([])).await?;
    ensure!(
        status["locked"] == false && status["fully_synced_height"] == status["node_tip"]["height"],
        "wallet is not synchronized"
    );
    let height = status["node_tip"]["height"]
        .as_u64()
        .context("missing wallet height")?;
    let remote = client
        .get_block(BlockId {
            height,
            hash: vec![],
        })
        .await?
        .into_inner();
    let mut hash = remote.hash;
    hash.reverse();
    ensure!(
        status["node_tip"]["blockhash"].as_str() == Some(hex::encode(hash).as_str()),
        "wallet and network chain mismatch"
    );
    // Never return the seller's private incoming viewing authority to stdout/receipts.
    let exported = wallet
        .rpc("z_exportviewingkey", json!([invoice.pay_to, true]))
        .await?;
    let encoded = Zeroizing::new(
        exported
            .as_str()
            .context("missing local viewing authority")?
            .to_owned(),
    );
    let ivk = UnifiedIncomingViewingKey::decode(&network.parameters(), &encoded)
        .map_err(|_| anyhow::anyhow!("invalid local viewing authority"))?;
    let Address::Unified(ua) =
        Address::decode(&network.parameters(), &invoice.pay_to).context("invalid recipient")?
    else {
        anyhow::bail!("expected UA")
    };
    let amount: Value = serde_json::from_str(
        &(rust_decimal::Decimal::from(invoice.amount_zat)
            / rust_decimal::Decimal::from(100_000_000u64))
        .to_string(),
    )?;
    let created=wallet.rpc("pczt_create",json!([config.account_uuid,[{"address":invoice.pay_to,"amount":amount,"memo":hex::encode(invoice.memo.as_bytes())}],null,"FullPrivacy","orchard"])).await?;
    let inspected = wallet.rpc("pczt_inspect", json!([pczt(&created)?])).await?;
    let fee = inspect_fee(&inspected, &invoice.pay_to, invoice.amount_zat)?;
    ensure!(fee <= args.max_fee_zat, "wallet fee exceeds authorized cap");
    let total = invoice
        .amount_zat
        .checked_add(fee)
        .filter(|v| *v <= 2_100_000_000_000_000)
        .context("payment total out of range")?;
    println!(
        "{}",
        json!({"kind":"local-self-payment-test","amount_zat":invoice.amount_zat,"network_fee_zat":fee,"total_output_plus_fee_zat":total,"self_payment":true,"dry_run":!args.execute})
    );
    if !args.execute {
        return Ok(());
    }
    invoice.validate(&config, Utc::now().timestamp())?;
    let mut journal = json!({"stage":"signing_started","invoice_id":invoice.invoice_id,"network":config.network,"amount_zat":invoice.amount_zat,"network_fee_zat":fee,"self_payment":true});
    write_journal(&args.journal, &journal, true)?;
    let proved = wallet.rpc("pczt_prove", json!([pczt(&created)?])).await?;
    let signed = wallet
        .rpc("pczt_sign", json!([pczt(&proved)?, "FullPrivacy", true]))
        .await?;
    let signed_info = wallet.rpc("pczt_inspect", json!([pczt(&signed)?])).await?;
    ensure!(
        inspect_fee(&signed_info, &invoice.pay_to, invoice.amount_zat)? == fee,
        "signed proposal changed payment or fee"
    );
    let extracted = wallet.rpc("pczt_extract", json!([pczt(&signed)?])).await?;
    ensure!(
        extracted["stored"] == true,
        "wallet did not store payment; refusing broadcast"
    );
    let bytes = hex::decode(
        extracted["hex"]
            .as_str()
            .context("missing extracted transaction")?,
    )
    .context("invalid extracted transaction")?;
    let tip = client.get_latest_block(ChainSpec {}).await?.into_inner();
    let branch = BranchId::for_height(
        &network.parameters(),
        BlockHeight::from_u32(u32::try_from(tip.height)?),
    );
    let mut reader = std::io::Cursor::new(&bytes);
    let tx = Transaction::read(&mut reader, branch).context("unsupported extracted transaction")?;
    ensure!(
        reader.position() == bytes.len() as u64 && tx.consensus_branch_id() == branch,
        "transaction branch mismatch"
    );
    let outputs = zadp_zcash::decrypt_board_outputs(
        &tx,
        ivk.orchard()
            .as_ref()
            .context("missing Orchard viewing authority")?,
        ua.orchard().context("missing Orchard recipient")?,
        0,
    );
    let matches = outputs
        .iter()
        .filter(|o| {
            o.value_zat == invoice.amount_zat
                && o.memo.starts_with(invoice.memo.as_bytes())
                && o.memo[invoice.memo.len()..].iter().all(|b| *b == 0)
        })
        .count();
    ensure!(
        matches == 1,
        "final transaction does not contain exactly one matching invoice output"
    );
    ensure!(
        tx.transparent_bundle().is_none() && tx.sapling_bundle().is_none(),
        "unexpected transaction pool"
    );
    let balance = tx
        .orchard_bundle()
        .map(|b| *b.value_balance())
        .unwrap_or_else(zcash_protocol::value::ZatBalance::zero)
        + tx.ironwood_bundle()
            .map(|b| *b.value_balance())
            .unwrap_or_else(zcash_protocol::value::ZatBalance::zero);
    ensure!(
        balance.is_some_and(|b| i64::from(b) == fee as i64),
        "final transaction fee changed"
    );
    let txid = tx.txid().to_string();
    ensure!(
        extracted["txid"].as_str() == Some(&txid),
        "inconsistent transaction ID"
    );
    journal["txid"] = json!(txid);
    journal["stage"] = json!("prepared_no_retry");
    write_journal(&args.journal, &journal, false)?;
    // Recheck expiry before the only broadcast call. A timeout never triggers a retry.
    invoice.validate(&config, Utc::now().timestamp())?;
    zadp_zcash::broadcast_transaction(&mut client, bytes).await?;
    journal["stage"] = json!("broadcast_accepted");
    write_journal(&args.journal, &journal, false)?;
    println!(
        "{}",
        json!({"txid":txid,"network_fee_zat":fee,"status":"broadcast_accepted","confirmations_required":3})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
    use zip32::{AccountId, DiversifierIndex};
    fn fixture() -> (Config, Invoice) {
        let params = ZcashNetwork::Mainnet.parameters();
        // Synthetic seed only, never a runtime wallet or funded account.
        let usk = UnifiedSpendingKey::from_seed(&params, &[42; 64], AccountId::ZERO).unwrap();
        let ivk = usk
            .to_unified_full_viewing_key()
            .to_unified_incoming_viewing_key();
        let address = ivk
            .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
            .unwrap()
            .encode(&params);
        let config = Config {
            network: "mainnet".into(),
            receive_address: address.clone(),
            account_uuid: "00000000-0000-0000-0000-000000000000".into(),
            wallet_rpc: "http://127.0.0.1:8232".into(),
            wallet_auth_file: "unused".into(),
            lightwalletd: "unused".into(),
            text_service: Service {
                offer_id: "local-test".into(),
                price_zat: 1_000_000,
            },
        };
        let invoice = Invoice {
            invoice_id: "a".repeat(32),
            offer_id: "local-test".into(),
            amount_zat: 1_000_000,
            currency: "ZEC".into(),
            pay_to: address,
            memo: format!("ZADP-PAY/1:{}", "a".repeat(32)),
            expires_at: 1800,
            network: "mainnet".into(),
            status: "awaiting_payment".into(),
            local_unpublished_test: true,
        };
        (config, invoice)
    }
    #[test]
    fn rejects_mutated_invoice_and_wrong_network_receivers() {
        let (config, invoice) = fixture();
        assert!(invoice.validate(&config, 100).is_ok());
        for field in [
            "amount_zat",
            "offer_id",
            "memo",
            "pay_to",
            "network",
            "currency",
            "status",
            "invoice_id",
            "local_unpublished_test",
        ] {
            let (_, mut bad) = fixture();
            match field {
                "amount_zat" => bad.amount_zat += 1,
                "offer_id" => bad.offer_id = "other".into(),
                "memo" => bad.memo = "other".into(),
                "pay_to" => bad.pay_to = "u1invalid".into(),
                "network" => bad.network = "testnet".into(),
                "currency" => bad.currency = "USD".into(),
                "status" => bad.status = "fulfilled".into(),
                "invoice_id" => bad.invoice_id = "g".repeat(32),
                _ => bad.local_unpublished_test = false,
            }
            assert!(bad.validate(&config, 100).is_err(), "{field}");
        }
        let (mut config, mut invoice) = fixture();
        config.network = "testnet".into();
        invoice.network = "testnet".into();
        assert!(invoice.validate(&config, 100).is_err());
        let (mut config, invoice) = fixture();
        config.receive_address = "u1invalid".into();
        let mut invoice = invoice;
        invoice.pay_to = config.receive_address.clone();
        assert!(invoice.validate(&config, 100).is_err());
    }
    #[test]
    fn rejects_expiry_short_window_and_invalid_account() {
        let (config, mut invoice) = fixture();
        assert!(invoice.validate(&config, 1680).is_err());
        invoice.expires_at = 1901;
        assert!(invoice.validate(&config, 100).is_err());
        let (mut config, invoice) = fixture();
        config.account_uuid = "invalid".into();
        assert!(invoice.validate(&config, 100).is_err());
    }
    #[test]
    fn journal_blocks_reexecution_and_contains_no_invoice_token() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("journal.json");
        let data = json!({"stage":"prepared_no_retry","txid":"a".repeat(64)});
        write_journal(&path, &data, true).unwrap();
        assert!(write_journal(&path, &data, true).is_err());
        assert_eq!(
            serde_json::from_slice::<Value>(&read_protected(&path).unwrap()).unwrap(),
            data
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

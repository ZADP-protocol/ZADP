//! Mainnet publication exists only behind the explicit `publish` CLI command.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand};
use rand::RngCore;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sha2::Digest;
use std::{
    io::{self, Write},
    path::PathBuf,
};
use zadp_protocol::{
    bind_offer, encode_offer_memo, BoardBody, BoardDescriptor, GeoArea, OfferDraft, Price,
    SellerKeyPair, ZcashNetwork,
};
use zadp_seller::{
    local::{load_identity, write_identity},
    publication::{inspect_fee, LocalWallet},
};
use zadp_zcash::{broadcast_transaction, connect_lightwalletd, decrypt_board_outputs};
use zcash_client_backend::proto::service::{BlockId, ChainSpec};
use zcash_keys::{address::Address, keys::UnifiedIncomingViewingKey};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId};

#[derive(Parser)]
#[command(
    about = "ZADP seller identity, board descriptor signing and explicit shielded OFFER publication"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    IdentityInit {
        #[arg(long)]
        output: PathBuf,
    },
    IdentityShow {
        #[arg(long)]
        identity: PathBuf,
    },
    BoardSign {
        #[arg(long)]
        body: PathBuf,
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Offline board-policy revision. Never changes a live descriptor or database.
    BoardPolicyPrepare {
        #[arg(long)]
        board: PathBuf,
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_parser=clap::value_parser!(u32).range(1..))]
        max_offer_ttl_seconds: u32,
    },
    /// Offline endpoint proof; deploy only the public JSON, never the identity key.
    EndpointProof {
        #[arg(long)]
        identity: PathBuf,
        #[arg(long)]
        origin: String,
        #[arg(long)]
        expires_at: chrono::DateTime<Utc>,
        #[arg(long)]
        output: PathBuf,
    },
    Publish(Box<Publish>),
    /// Pay a local self-payment test invoice using an existing protected wallet.
    PayTest(Box<zadp_seller::purchase::Purchase>),
    /// Explicit reviewed local shielded transfer; never called by discovery.
    Transfer(Box<zadp_seller::transfer::Transfer>),
}
#[derive(clap::Args)]
struct Publish {
    #[arg(long, default_value = "mainnet")]
    network: String,
    #[arg(long)]
    board: PathBuf,
    #[arg(long, env = "ZADP_IDENTITY_FILE")]
    identity: PathBuf,
    #[arg(long)]
    capability: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: Option<String>,
    /// Public approximate offer-area latitude, degrees multiplied by 100000.
    #[arg(long, allow_hyphen_values = true, requires_all = ["geo_lon_e5", "geo_radius_m"])]
    geo_lat_e5: Option<i32>,
    #[arg(long, allow_hyphen_values = true, requires_all = ["geo_lat_e5", "geo_radius_m"])]
    geo_lon_e5: Option<i32>,
    #[arg(long, requires_all = ["geo_lat_e5", "geo_lon_e5"])]
    geo_radius_m: Option<u32>,
    #[arg(long)]
    price_zec: Option<Decimal>,
    #[arg(long, requires = "price_zec")]
    endpoint: Option<String>,
    #[arg(long,default_value_t=86400,value_parser=clap::value_parser!(u32).range(1..))]
    ttl_seconds: u32,
    /// Explicit Offer version; legacy defaults remain unchanged. Manual prices require v3.
    #[arg(long, value_parser=clap::value_parser!(u16).range(1..=3))]
    offer_version: Option<u16>,
    /// Defaults to the operator's minimum listing value. This is not the service price.
    #[arg(long)]
    listing_value_zat: Option<u64>,
    /// Local Zallet RPC only; seeds and spending keys never leave that process.
    #[arg(long, default_value = "http://127.0.0.1:8232")]
    wallet_rpc: String,
    #[arg(long, env = "ZADP_WALLET_AUTH_FILE")]
    wallet_auth_file: PathBuf,
    #[arg(long)]
    wallet_account: String,
    #[arg(long)]
    lightwalletd: String,
    #[arg(long)]
    yes: bool,
    /// Build and inspect the unsigned proposal; never prove, sign, extract or broadcast.
    #[arg(long)]
    dry_run: bool,
    /// Save a public unsigned-transaction preview; requires --dry-run.
    #[arg(long, requires = "dry_run")]
    preview_output: Option<PathBuf>,
    /// Save protected unsigned PCZT for a later exact human-approved publication.
    #[arg(long, requires = "dry_run")]
    proposal_output: Option<PathBuf>,
    /// Reuse the exact reviewed unsigned proposal, not a newly generated payment.
    #[arg(long, requires_all = ["approved_preview", "prepared_offer", "yes"])]
    approved_proposal: Option<PathBuf>,
    #[arg(long, requires = "approved_proposal")]
    approved_preview: Option<PathBuf>,
    /// Reuse the exact signed public offer from a reviewed preview, never regenerate its ID.
    #[arg(long)]
    prepared_offer: Option<PathBuf>,
    /// Save signed OFFER + txid as a public acceptance-test receipt, never wallet data.
    #[arg(long)]
    receipt: Option<PathBuf>,
}
fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    ensure!(
        std::fs::metadata(path)?.len() <= 1_048_576,
        "input JSON too large"
    );
    serde_json::from_slice(&std::fs::read(path)?).context("invalid input JSON")
}
#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse().command {
        Command::IdentityInit { output } => {
            let keys = SellerKeyPair::generate();
            write_identity(&output, &keys)?;
            println!("Identity public key: {}", keys.id);
        }
        Command::IdentityShow { identity } => println!("{}", load_identity(&identity)?.id),
        Command::BoardSign {
            body,
            identity,
            output,
        } => {
            let body: BoardBody = read_json(&body)?;
            let board = body.sign(&load_identity(&identity)?)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            file.write_all(serde_json::to_string_pretty(&board)?.as_bytes())?;
            file.sync_all()?;
            println!("Signed descriptor fingerprint: {}", board.fingerprint()?);
        }
        Command::BoardPolicyPrepare {
            board,
            identity,
            output,
            max_offer_ttl_seconds,
        } => {
            let old: BoardDescriptor = read_json(&board)?;
            old.validate(old.board.network)?;
            ensure!(board != output, "A separate descriptor output is required");
            let mut body = old.board.clone();
            body.policy.max_offer_ttl_seconds = max_offer_ttl_seconds;
            let new = body.sign(&load_identity(&identity)?)?;
            new.validate(old.board.network)?;
            ensure!(
                old.fingerprint()? != new.fingerprint()?,
                "Policy is unchanged"
            );
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(output)?;
            file.write_all(serde_json::to_string_pretty(&new)?.as_bytes())?;
            file.sync_all()?;
            println!(
                "{}",
                json!({"old_fingerprint":old.fingerprint()?,"new_fingerprint":new.fingerprint()?,"network":new.board.network,"board_id":new.board.board_id,"birthday_height":new.board.birthday_height,"max_offer_ttl_seconds":max_offer_ttl_seconds,"signature_valid":true,"address_uivk_binding_valid":true,"unrelated_fields_preserved":true,"activated":false})
            );
        }
        Command::EndpointProof {
            identity,
            origin,
            expires_at,
            output,
        } => {
            let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
            let proof = zadp_protocol::EndpointProof::sign(
                &load_identity(&identity)?,
                &origin,
                now,
                expires_at,
            )?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            file.write_all(serde_json::to_string_pretty(&proof)?.as_bytes())?;
            file.sync_all()?;
            println!(
                "Public endpoint proof created for {}; expires {}",
                proof.origin, proof.expires_at
            );
        }
        Command::Publish(args) => publish(*args).await?,
        Command::PayTest(args) => zadp_seller::purchase::purchase(*args).await?,
        Command::Transfer(args) => zadp_seller::transfer::transfer(*args).await?,
    }
    Ok(())
}
fn pczt(result: &Value) -> Result<&str> {
    result["pczt"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("local wallet returned no PCZT"))
}
fn zec(zat: u64) -> Decimal {
    Decimal::from(zat) / Decimal::from(100_000_000u64)
}
async fn publish(args: Publish) -> Result<()> {
    let network = match args.network.as_str() {
        "mainnet" => ZcashNetwork::Mainnet,
        "testnet" => ZcashNetwork::Testnet,
        _ => anyhow::bail!("network must be mainnet or testnet"),
    };
    ensure!(
        args.wallet_account.len() == 36
            && args.wallet_account.chars().enumerate().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == '-'
                } else {
                    c.is_ascii_hexdigit()
                }
            }),
        "wallet-account must be a Zallet account UUID"
    );
    let board: BoardDescriptor = read_json(&args.board)?;
    board.validate(network)?;
    let value = args
        .listing_value_zat
        .unwrap_or(board.board.policy.min_listing_value_zat);
    ensure!(
        value > 0 && value <= 2_100_000_000_000_000,
        "listing value is outside the Zcash money range"
    );
    let keys = load_identity(&args.identity)?;
    let now = DateTime::from_timestamp(Utc::now().timestamp(), 0).context("invalid clock")?;
    let mut nonce = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let mut draft = match (args.endpoint, args.price_zec) {
        (Some(endpoint), Some(price)) => OfferDraft::new_ulid(
            keys.id.clone(),
            args.capability,
            args.title,
            Price::zec(price.normalize()),
            endpoint,
            now + Duration::seconds(i64::from(args.ttl_seconds)),
            B64.encode(nonce),
        ),
        (None, None) => OfferDraft::new_discovery(
            keys.id.clone(),
            args.capability,
            args.title,
            now + Duration::seconds(i64::from(args.ttl_seconds)),
            B64.encode(nonce),
        ),
        (None, Some(price)) => OfferDraft::new_discovery(
            keys.id.clone(),
            args.capability,
            args.title,
            now + Duration::seconds(i64::from(args.ttl_seconds)),
            B64.encode(nonce),
        )
        .with_price(Price::zec(price.normalize())),
        _ => anyhow::bail!("endpoint requires price"),
    };
    if let Some(v) = args.offer_version {
        draft = draft.with_version(v);
    }
    draft.description = args.description.filter(|s| !s.is_empty());
    match (args.geo_lat_e5, args.geo_lon_e5, args.geo_radius_m) {
        (Some(lat_e5), Some(lon_e5), Some(radius_m)) => {
            let area = GeoArea {
                lat_e5,
                lon_e5,
                radius_m,
            };
            area.validate()?;
            draft = draft.with_geo(area);
        }
        (None, None, None) => {}
        _ => anyhow::bail!("offer area requires latitude, longitude and radius"),
    }
    let candidate = bind_offer(draft.sign(&keys)?, &board.board.board_id, now, &keys)?;
    let offer = if let Some(path) = &args.prepared_offer {
        let prepared: zadp_protocol::Offer = read_json(path)?;
        ensure!(
            prepared.zadp == candidate.zadp
                && prepared.seller == candidate.seller
                && prepared.title == candidate.title
                && prepared.capability == candidate.capability
                && prepared.description == candidate.description
                && prepared.geo == candidate.geo
                && prepared.price == candidate.price
                && prepared.commerce == candidate.commerce
                && prepared.board_id == candidate.board_id,
            "prepared offer differs from requested content"
        );
        ensure!(
            prepared.issued_at.is_some_and(|issued| prepared
                .expires_at
                .signed_duration_since(issued)
                .num_seconds()
                == i64::from(args.ttl_seconds)),
            "prepared offer TTL differs"
        );
        prepared
    } else {
        candidate
    };

    board.validate_offer(&offer, value, now)?;
    let memo = encode_offer_memo(&offer)?;
    let wallet = LocalWallet::connect(&args.wallet_rpc, &args.wallet_auth_file)?;
    let mut client = connect_lightwalletd(&args.lightwalletd, network).await?;
    // Tie the local wallet to the same actual chain before asking it to create a proposal.
    let status = wallet.rpc("getwalletstatus", json!([])).await?;
    ensure!(
        status["locked"] == false,
        "local wallet is not fully synced"
    );
    let wallet_height = status["node_tip"]["height"]
        .as_u64()
        .context("missing wallet tip height")?;
    let remote = client
        .get_block(BlockId {
            height: wallet_height,
            hash: vec![],
        })
        .await?
        .into_inner();
    let mut hash = remote.hash.clone();
    hash.reverse();
    ensure!(
        status["node_tip"]["blockhash"].as_str() == Some(hex::encode(hash).as_str()),
        "local wallet and mainnet service are on different chains"
    );
    let amount: Value = serde_json::from_str(&zec(value).to_string())?;
    let created = if let Some(path) = &args.approved_proposal {
        serde_json::from_slice(&zadp_seller::local::read_protected_proposal(path)?)?
    } else {
        wallet.rpc("pczt_create",json!([args.wallet_account,[{"address":board.board.address,"amount":amount,"memo":hex::encode(memo)}],null,"FullPrivacy","orchard"])).await?
    };
    ensure!(
        created["privacy_policy"] == "FullPrivacy",
        "wallet proposal requires privacy downgrade"
    );
    let inspected = wallet.rpc("pczt_inspect", json!([pczt(&created)?])).await?;
    let fee = inspect_fee(&inspected, &board.board.address, value)?;
    let total = value
        .checked_add(fee)
        .filter(|v| *v <= 2_100_000_000_000_000)
        .context("spend exceeds Zcash money range")?;
    if let Some(path) = &args.approved_preview {
        let approved: Value = read_json(path)?;
        zadp_seller::publication::validate_approval(
            &approved,
            &offer,
            &board,
            value,
            fee,
            pczt(&created)?,
            Utc::now(),
        )?;
    }
    println!(
        "Signed Offer expiry (UTC): {}",
        offer.expires_at.to_rfc3339()
    );
    println!("ZADP {} PUBLICATION\n\nBoard: {}\nDestination: {}\nDescriptor: {}\nOffer: {}\nService price: {} ZEC\nListing value: {} ZEC ({value} zat)\nExpected network fee (wallet ZIP 317): {} ZEC ({fee} zat)\nTotal wallet spend: {} ZEC ({total} zat)\nNetwork: {:?}\nMemo: {} bytes",args.network.to_uppercase(),board.board.name,board.board.address,board.fingerprint()?,offer.title,offer.price.as_ref().map(|p|p.amount.to_string()).unwrap_or_else(||"none (discovery only)".into()),zec(value),zec(fee),zec(total),network,memo.len());
    if args.dry_run {
        if let Some(path) = &args.proposal_output {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(path)?;
            file.write_all(serde_json::to_vec(&created)?.as_slice())?;
            file.sync_all()?;
        }
        if let Some(path) = args.preview_output {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(
                serde_json::to_string_pretty(&json!({
                    "kind":"unsigned-publication-preview", "network": network, "preview_expires_at":Utc::now().timestamp()+900,
                    "board_fingerprint":board.fingerprint()?, "board_id":board.board.board_id,
                    "offer":offer, "payload_bytes":u16::from_be_bytes([memo[6],memo[7]]),
                    "listing_value_zat":value, "network_fee_zat":fee, "total_cost_zat":total,
                    "board_recipient":board.board.address,
                    "proposal_fingerprint":hex::encode(sha2::Sha256::digest(pczt(&created)?.as_bytes())),
                    "fee_source":"wallet-inspected-unsigned-proposal",
                    "zcash_transaction_signed":false, "broadcast":false
                }))?
                .as_bytes(),
            )?;
            file.sync_all()?;
        }
        println!("DRY RUN: unsigned proposal only; nothing signed or broadcast.");
        return Ok(());
    }
    if !args.yes {
        print!("Proceed? [y/N] ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    board.validate_offer(&offer, value, Utc::now())?;
    let proved = wallet.rpc("pczt_prove", json!([pczt(&created)?])).await?;
    let signed = wallet
        .rpc("pczt_sign", json!([pczt(&proved)?, "FullPrivacy", true]))
        .await?;
    let extracted = wallet.rpc("pczt_extract", json!([pczt(&signed)?])).await?;
    ensure!(
        extracted["stored"] == true,
        "wallet did not record the transaction; refusing broadcast"
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
        "transaction consensus branch changed; create a fresh proposal"
    );
    let ivk =
        UnifiedIncomingViewingKey::decode(&network.parameters(), &board.board.incoming_viewing_key)
            .map_err(|_| anyhow::anyhow!("invalid board UIVK"))?;
    let Address::Unified(ua) = Address::decode(&network.parameters(), &board.board.address)
        .context("invalid board address")?
    else {
        anyhow::bail!("expected UA");
    };
    let outputs = decrypt_board_outputs(
        &tx,
        ivk.orchard().as_ref().context("missing Orchard IVK")?,
        ua.orchard().context("missing Orchard receiver")?,
        0,
    );
    ensure!(
        outputs.len() == 1 && outputs[0].value_zat == value && outputs[0].memo == memo,
        "final transaction does not contain the exact signed board OFFER and listing value"
    );
    ensure!(
        tx.transparent_bundle().is_none() && tx.sapling_bundle().is_none(),
        "final transaction is not Orchard-family-only shielded"
    );
    let balances = tx
        .orchard_bundle()
        .map(|b| *b.value_balance())
        .unwrap_or_else(zcash_protocol::value::ZatBalance::zero)
        + tx.ironwood_bundle()
            .map(|b| *b.value_balance())
            .unwrap_or_else(zcash_protocol::value::ZatBalance::zero);
    ensure!(
        balances.is_some_and(|balance| i64::from(balance) == fee as i64),
        "final transaction fee differs from confirmed preview"
    );
    let txid = tx.txid().to_string();
    ensure!(
        extracted["txid"].as_str() == Some(&txid),
        "wallet returned inconsistent transaction ID"
    );
    // Record a recovery ID before the network call: a timeout may still mean accepted.
    println!("Prepared transaction ID: {txid}");
    if let Some(path) = args.receipt {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(serde_json::to_string_pretty(&json!({"network":network,"board_fingerprint":board.fingerprint()?,"txid":txid,"offer":offer,"listing_value_zat":value,"fee_zat":fee,"broadcast_status":"prepared; verify on chain"}))?.as_bytes())?;
        file.sync_all()?;
    }
    board.validate_offer(&offer, value, Utc::now())?;
    broadcast_transaction(&mut client, bytes).await?;
    println!(
        "Broadcast accepted: {txid}\nWait for mining, then query the independent board indexer."
    );
    Ok(())
}

#[cfg(test)]
mod geo_cli_tests {
    use super::*;
    #[test]
    fn discovery_cli_needs_neither_price_nor_endpoint() {
        let base = [
            "zadp-seller",
            "publish",
            "--board",
            "unused",
            "--identity",
            "unused",
            "--capability",
            "local.service",
            "--title",
            "Discovery",
            "--wallet-auth-file",
            "unused",
            "--wallet-account",
            "unused",
            "--lightwalletd",
            "https://fixture.invalid",
            "--dry-run",
        ];
        assert!(Args::try_parse_from(base).is_ok());
        assert!(Args::try_parse_from(
            base.into_iter()
                .chain(["--endpoint", "https://fixture.invalid"])
        )
        .is_err());
        assert!(Args::try_parse_from(base.into_iter().chain(["--price-zec", "0.01"])).is_ok());
        assert!(Args::try_parse_from(base.into_iter().chain([
            "--offer-version",
            "3",
            "--price-zec",
            "0.02"
        ]))
        .is_ok());
        assert!(Args::try_parse_from(
            base.into_iter()
                .chain(["--prepared-offer", "reviewed.json"])
        )
        .is_ok());
    }
    #[test]
    fn public_area_flags_are_complete_and_negative_coordinates_parse() {
        let base = [
            "zadp-seller",
            "publish",
            "--board",
            "unused-board.json",
            "--identity",
            "unused-identity",
            "--capability",
            "cleaning",
            "--title",
            "Fixture",
            "--price-zec",
            "0.01",
            "--endpoint",
            "https://fixture.invalid",
            "--wallet-auth-file",
            "unused-auth",
            "--wallet-account",
            "unused-account",
            "--lightwalletd",
            "https://fixture.invalid",
            "--dry-run",
        ];
        assert!(Args::try_parse_from(base).is_ok());
        for partial in [
            vec!["--geo-lat-e5", "-9000000"],
            vec!["--geo-lon-e5", "18000000"],
            vec!["--geo-radius-m", "50000"],
            vec!["--geo-lat-e5", "1", "--geo-lon-e5", "2"],
        ] {
            assert!(Args::try_parse_from(base.into_iter().chain(partial)).is_err());
        }
        let args = Args::try_parse_from(base.into_iter().chain([
            "--geo-lat-e5",
            "-9000000",
            "--geo-lon-e5",
            "-18000000",
            "--geo-radius-m",
            "500000",
        ]))
        .unwrap();
        let Command::Publish(p) = args.command else {
            panic!("publish expected")
        };
        assert_eq!(p.geo_lat_e5, Some(-9000000));
        assert_eq!(p.geo_lon_e5, Some(-18000000));
        assert_eq!(p.geo_radius_m, Some(500000));
        // Parsing only: no file or wallet process is accessed.
    }
}

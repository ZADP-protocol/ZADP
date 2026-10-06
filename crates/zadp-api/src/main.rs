//! Read-only single/multi-board scanner. No wallet, spending or public onboarding.
use anyhow::{ensure, Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use zadp_indexer::{
    BoardContext, BoardIndexer, BoardRegistry, BoardSourceType, BoardStatus, HostedBoardPolicy,
    SqliteOfferRepository, Visibility,
};
use zadp_protocol::{BoardDescriptor, ZcashNetwork};
use zadp_zcash::{BoardTransport, RealZcashScanner};

#[derive(Parser)]
#[command(about = "Index signed discovery boards using incoming viewing authority only")]
struct Args {
    #[arg(
        long,
        required_unless_present = "registry",
        conflicts_with = "registry"
    )]
    board: Option<PathBuf>,
    /// Operator-managed registry file. Descriptor/database paths are relative to this file.
    #[arg(long)]
    registry: Option<PathBuf>,
    #[arg(long, default_value = "mainnet")]
    network: String,
    #[arg(long, required_unless_present = "moderation_only")]
    lightwalletd: Option<String>,
    #[arg(long, default_value = "zadp-board.sqlite")]
    database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: std::net::SocketAddr,
    #[arg(long,default_value_t=100,value_parser=clap::value_parser!(u32).range(1..=10000))]
    batch_size: u32,
    #[arg(long,default_value_t=15,value_parser=clap::value_parser!(u64).range(1..=3600))]
    poll_seconds: u64,
    #[arg(long)]
    once: bool,
    /// Apply local operator overrides without connecting or scanning.
    #[arg(long, requires = "moderation")]
    moderation_only: bool,
    #[arg(long, requires = "moderation_only")]
    moderation: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    #[serde(default)]
    hosted_policy: HostedBoardPolicy,
    boards: Vec<Registration>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    descriptor: PathBuf,
    database: PathBuf,
    source_type: BoardSourceType,
    status: BoardStatus,
    #[serde(default)]
    hosting_fee_approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Override {
    board_id: String,
    seller: String,
    offer_id: String,
    status: Visibility,
    reason: String,
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    ensure!(
        std::fs::metadata(path)?.len() <= 1_048_576,
        "configuration too large"
    );
    let bytes = std::fs::read(path)?;
    ensure!(bytes.len() <= 1_048_576, "configuration too large");
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("invalid configuration JSON"))
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();
    let network = match args.network.as_str() {
        "mainnet" => ZcashNetwork::Mainnet,
        "testnet" => ZcashNetwork::Testnet,
        _ => anyhow::bail!("network must be mainnet or testnet"),
    };
    let (config, base) = if let Some(path) = &args.registry {
        (
            read_json::<RegistryFile>(path)?,
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        )
    } else {
        (
            RegistryFile {
                hosted_policy: HostedBoardPolicy::default(),
                boards: vec![Registration {
                    descriptor: args.board.clone().context("missing board")?,
                    database: args.database.clone(),
                    source_type: BoardSourceType::External,
                    status: BoardStatus::Active,
                    hosting_fee_approved: false,
                }],
            },
            PathBuf::new(),
        )
    };
    ensure!(
        !config.boards.is_empty() && config.boards.len() <= 100,
        "registry requires 1..100 boards"
    );
    // Preflight every descriptor before opening/migrating any databases.
    let mut entries = vec![];
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut databases = BTreeSet::new();
    for entry in config.boards {
        let descriptor: BoardDescriptor = read_json(&base.join(entry.descriptor))?;
        descriptor.validate(network)?;
        ensure!(
            ids.insert(descriptor.board.board_id.clone()),
            "duplicate board id"
        );
        ensure!(
            keys.insert(descriptor.incoming_scope_fingerprint()?),
            "boards require dedicated UIVKs"
        );
        let database = base.join(entry.database);
        ensure!(
            databases.insert(database.clone()),
            "boards require separate databases"
        );
        entries.push((
            descriptor,
            database,
            entry.source_type,
            entry.status,
            entry.hosting_fee_approved,
        ));
    }
    let mut registry = BoardRegistry::new(network);
    for (board, path, source_type, status, approved) in entries {
        let repository = Arc::new(SqliteOfferRepository::open(path, board)?);
        registry.register(
            BoardContext {
                repository,
                source_type,
                status,
            },
            &config.hosted_policy,
            approved,
        )?;
    }
    if args.moderation_only {
        let overrides: Vec<Override> = read_json(
            args.moderation
                .as_ref()
                .context("missing moderation file")?,
        )?;
        ensure!(overrides.len() <= 1000, "too many moderation overrides");
        for entry in overrides {
            let board = registry
                .get(&entry.board_id)
                .context("unknown moderation board")?;
            board.repository.moderate(
                &entry.seller,
                &entry.offer_id,
                entry.status,
                &entry.reason,
            )?;
        }
        println!("Local host moderation applied; canonical publications unchanged.");
        return Ok(());
    }
    let transport = BoardTransport::connect(
        args.lightwalletd.as_deref().context("missing transport")?,
        network,
    )
    .await?;
    let mut scanners = vec![];
    for board in registry
        .contexts()
        .filter(|b| b.status == BoardStatus::Active)
    {
        let source = Arc::new(RealZcashScanner::with_transport(
            board.descriptor(),
            &transport,
        )?);
        scanners.push((
            board.descriptor().board.board_id.clone(),
            Arc::new(BoardIndexer::new(source, Arc::clone(&board.repository))),
        ));
    }
    let registry = Arc::new(registry);
    if args.once {
        for (id, indexer) in scanners {
            loop {
                let repo = indexer.repository();
                let before = repo.cursor()?;
                let accepted = indexer.sync_once(args.batch_size).await?;
                let after = repo.cursor()?;
                println!(
                    "board={id} accepted={accepted} scanned_height={:?} rejected={}",
                    after.as_ref().map(|h| h.height),
                    repo.rejection_count()?
                );
                if before == after {
                    break;
                }
            }
        }
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    let app = zadp_api::multi_board_router(registry);
    // Independent workers: one board's failure or older birthday cannot block others.
    for (id, indexer) in scanners {
        tokio::spawn(async move {
            loop {
                let before = indexer.repository().cursor().ok().flatten();
                match indexer.sync_once(args.batch_size).await {
                    Ok(n) => {
                        if n > 0 {
                            tracing::info!(board_id=%id,accepted=n,"board offers indexed");
                        }
                    }
                    Err(_) => {
                        tracing::warn!(board_id=%id,"board sync paused; retrying failed block")
                    }
                }
                let caught_up = indexer.repository().cursor().ok().flatten() == before;
                tokio::time::sleep(std::time::Duration::from_secs(if caught_up {
                    args.poll_seconds
                } else {
                    1
                }))
                .await;
            }
        });
    }
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operator_registry_and_moderation_accept_no_secret_fields() {
        let config = serde_json::json!({"boards":[{"descriptor":"external.json","database":"external.sqlite","source_type":"external","status":"active"}]});
        assert!(serde_json::from_value::<RegistryFile>(config.clone()).is_ok());
        let mut invalid = config.clone();
        invalid["boards"][0]["spending_key"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<RegistryFile>(invalid).is_err());
        let mut invalid = config;
        invalid["seed"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<RegistryFile>(invalid).is_err());
        assert!(serde_json::from_value::<Override>(serde_json::json!({"board_id":"a","seller":"public","offer_id":"id","status":"hidden","reason":"demo","protocol_valid":false})).is_err());
    }
}

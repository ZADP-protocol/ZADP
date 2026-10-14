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
    SqliteOfferRepository,
};
use zadp_protocol::{BoardDescriptor, ZcashNetwork};
use zadp_zcash::{BoardTransport, RealZcashScanner};

#[derive(Parser)]
#[command(about = "Index signed discovery boards using incoming viewing authority only")]
pub struct Args {
    #[arg(
        long,
        required_unless_present = "registry",
        conflicts_with = "registry"
    )]
    pub board: Option<PathBuf>,
    /// Operator-managed registry file. Descriptor/database paths are relative to this file.
    #[arg(long)]
    pub registry: Option<PathBuf>,
    /// Operator-only public presentation metadata; never modifies signed descriptors.
    #[arg(long)]
    pub board_metadata: Option<PathBuf>,
    #[arg(long, default_value = "mainnet")]
    pub network: String,
    #[arg(long)]
    pub lightwalletd: Option<String>,
    #[arg(long, default_value = "zadp-board.sqlite")]
    pub database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub listen: std::net::SocketAddr,
    #[arg(long,default_value_t=100,value_parser=clap::value_parser!(u32).range(1..=10000))]
    pub batch_size: u32,
    #[arg(long,default_value_t=15,value_parser=clap::value_parser!(u64).range(1..=3600))]
    pub poll_seconds: u64,
    #[arg(long)]
    pub once: bool,
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
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    ensure!(
        std::fs::metadata(path)?.len() <= 1_048_576,
        "configuration too large"
    );
    let bytes = std::fs::read(path)?;
    ensure!(bytes.len() <= 1_048_576, "configuration too large");
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("invalid configuration JSON"))
}
/// Open canonical registries without invoking any hosted moderation package.
pub fn open_registry(args: &Args) -> Result<(ZcashNetwork, BoardRegistry)> {
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
    Ok((network, registry))
}
pub async fn run(
    args: Args,
    configure: impl FnOnce(
        Arc<BoardRegistry>,
        Option<(String, String)>,
        std::collections::BTreeMap<String, crate::BoardPresentation>,
    ) -> Result<axum::Router>,
) -> Result<()> {
    let (network, registry) = open_registry(&args)?;
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
    let versions = transport.backend_versions().await.ok();
    let metadata: std::collections::BTreeMap<String, crate::BoardPresentation> =
        match args.board_metadata {
            Some(path) => read_json(&path)?,
            None => Default::default(),
        };
    for (id, m) in &metadata {
        ensure!(
            registry.get(id).is_some()
                && !m.display_name.is_empty()
                && m.display_name.len() <= 128
                && m.description.len() <= 512,
            "invalid board presentation metadata"
        );
    }
    let app = configure(registry, versions, metadata)?;
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
    let app = crate::throttle::DiscoveryThrottle::from_env()
        .map_err(anyhow::Error::msg)?
        .apply(app);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operator_registry_accepts_no_secret_fields() {
        let config = serde_json::json!({"boards":[{"descriptor":"external.json","database":"external.sqlite","source_type":"external","status":"active"}]});
        assert!(serde_json::from_value::<RegistryFile>(config.clone()).is_ok());
        let mut invalid = config.clone();
        invalid["boards"][0]["spending_key"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<RegistryFile>(invalid).is_err());
        let mut invalid = config;
        invalid["seed"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<RegistryFile>(invalid).is_err());
    }
}

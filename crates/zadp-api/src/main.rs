//! Long-running mainnet board scanner + read-only HTTP API. Cannot spend funds.
use anyhow::{ensure, Context, Result};
use clap::Parser;
use std::{path::PathBuf, sync::Arc};
use zadp_indexer::{BoardIndexer, SqliteOfferRepository};
use zadp_protocol::{BoardDescriptor, ZcashNetwork};
use zadp_zcash::RealZcashScanner;

#[derive(Parser)]
#[command(about = "Scan an actual Zcash discovery board with public incoming viewing authority")]
struct Args {
    #[arg(long)]
    board: PathBuf,
    #[arg(long, default_value = "mainnet")]
    network: String,
    #[arg(long)]
    lightwalletd: String,
    #[arg(long, default_value = "zadp-board.sqlite")]
    database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: std::net::SocketAddr,
    #[arg(long,default_value_t=100, value_parser=clap::value_parser!(u32).range(1..=10000))]
    batch_size: u32,
    #[arg(long,default_value_t=15, value_parser=clap::value_parser!(u64).range(1..=3600))]
    poll_seconds: u64,
    /// Scan to the current tip and exit without starting HTTP.
    #[arg(long)]
    once: bool,
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
    ensure!(
        std::fs::metadata(&args.board)?.len() <= 1_048_576,
        "board descriptor too large"
    );
    let board: BoardDescriptor =
        serde_json::from_slice(&std::fs::read(&args.board)?).context("invalid board JSON")?;
    board.validate(network)?;
    println!(
        "Board: {}\nNetwork: {:?}\nDescriptor fingerprint: {}",
        board.board.name,
        network,
        board.fingerprint()?
    );
    let source = Arc::new(RealZcashScanner::connect(&board, &args.lightwalletd).await?);
    let repository = Arc::new(SqliteOfferRepository::open(&args.database, board)?);
    let indexer = Arc::new(BoardIndexer::new(source, Arc::clone(&repository)));
    if args.once {
        loop {
            let before = repository.cursor()?;
            let accepted = indexer.sync_once(args.batch_size).await?;
            let after = repository.cursor()?;
            println!(
                "accepted={accepted} scanned_height={:?} rejected={}",
                after.as_ref().map(|h| h.height),
                repository.rejection_count()?
            );
            if before == after {
                break;
            }
        }
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    let app = zadp_api::board_router(Arc::clone(&repository));
    tokio::spawn(async move {
        loop {
            let before = repository.cursor().ok().flatten();
            match indexer.sync_once(args.batch_size).await {
                Ok(n) => {
                    if n > 0 {
                        tracing::info!(accepted = n, "board offers indexed");
                    }
                }
                Err(e) => {
                    tracing::warn!(error=%e,"board sync paused; will retry without advancing failed block")
                }
            }
            let caught_up = repository.cursor().ok().flatten() == before;
            let delay = if caught_up { args.poll_seconds } else { 1 };
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    });
    axum::serve(listener, app).await?;
    Ok(())
}

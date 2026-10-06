use anyhow::{ensure, Result};
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};
use zadp_commerce::{http, Config};
use zadp_seller::local::read_protected;
#[derive(Parser)]
#[command(about = "Independent seller commerce reference. No signing or broadcasting.")]
struct Args {
    /// Protected seller configuration; never copy it into a public repository.
    #[arg(long)]
    config: PathBuf,
    /// HTTP is loopback-only; expose through your own HTTPS reverse proxy.
    #[arg(long, default_value = "127.0.0.1:8094")]
    bind: SocketAddr,
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        args.bind.ip().is_loopback(),
        "bind must be loopback; use an HTTPS reverse proxy"
    );
    let config: Config = serde_json::from_slice(&read_protected(&args.config)?)?;
    config.validate()?;
    let router = http::router(config)?;
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    eprintln!(
        "Seller commerce ready on {} (no payment signing or broadcasting)",
        args.bind
    );
    axum::serve(listener, router).await?;
    Ok(())
}

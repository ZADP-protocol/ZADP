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
    /// Public JSON generated offline with zadp-seller endpoint-proof; no signing key on this server.
    #[arg(long)]
    endpoint_proof: Option<PathBuf>,
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
    let proof = args
        .endpoint_proof
        .map(|path| -> Result<zadp_protocol::EndpointProof> {
            let file = std::fs::File::open(path)?;
            ensure!(
                file.metadata()?.is_file() && file.metadata()?.len() <= 16384,
                "invalid endpoint proof file"
            );
            use std::io::Read;
            let mut bytes = Vec::new();
            file.take(16385).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 16384, "endpoint proof too large");
            Ok(serde_json::from_slice(&bytes)?)
        })
        .transpose()?;
    let router = http::router_with_endpoint_proof(config, proof)?;
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    eprintln!(
        "Seller commerce ready on {} (no payment signing or broadcasting)",
        args.bind
    );
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

//! Real persistent indexer over synthetic offline blocks; no network scan or broadcast.
use anyhow::{ensure, Result};
use axum::{
    body::{to_bytes, Body},
    http::Response,
    middleware,
};
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use zadp_indexer::SqliteOfferRepository;
use zadp_protocol::{encode_offer_memo, BoardDescriptor, Offer};
use zadp_zcash::{BlockHeader, BoardBlock, BoardOutput, TxId};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    directory: PathBuf,
    #[arg(long)]
    bind: SocketAddr,
}
async fn label(response: Response<Body>) -> Response<Body> {
    if response
        .headers()
        .get("content-type")
        .is_some_and(|v| v.to_str().unwrap_or("").contains("application/json"))
    {
        let (mut parts, body) = response.into_parts();
        if let Ok(bytes) = to_bytes(body, 2_000_000).await {
            if let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(offers) = value["offers"].as_array_mut() {
                    for offer in offers {
                        offer["simulation"] = true.into();
                    }
                }
                parts.headers.remove("content-length");
                return Response::from_parts(
                    parts,
                    Body::from(serde_json::to_vec(&value).unwrap()),
                );
            }
        }
        return Response::from_parts(parts, Body::empty());
    }
    response
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(args.bind.ip().is_loopback(), "loopback fixture only");
    // Only public descriptor/offer files. Never read seller config, order DB or wallet.
    let board: BoardDescriptor =
        serde_json::from_slice(&std::fs::read(args.directory.join("board.json"))?)?;
    ensure!(
        board.board.network == zadp_protocol::ZcashNetwork::Testnet,
        "testnet fixture only"
    );
    let public: serde_json::Value =
        serde_json::from_slice(&std::fs::read(args.directory.join("offers.json"))?)?;
    let offer: Offer = serde_json::from_value(public["offers"][0].clone())?;
    let database = args.directory.join("index.sqlite");
    let repo = Arc::new(SqliteOfferRepository::open(database, board.clone())?);
    if repo.cursor()?.is_none() {
        let now = offer.issued_at.unwrap();
        for offset in 0..3 {
            repo.apply_block(BoardBlock {
                header: BlockHeader {
                    height: board.board.birthday_height + offset,
                    hash: format!("{:064x}", offset + 1),
                    prev_hash: format!("{:064x}", offset),
                    time: now,
                },
                outputs: if offset == 0 {
                    vec![BoardOutput {
                        txid: TxId("b".repeat(64)),
                        transaction_index: 0,
                        output_index: 0,
                        pool: "orchard".into(),
                        value_zat: 1000,
                        memo: encode_offer_memo(&offer)?.to_vec(),
                    }]
                } else {
                    vec![]
                },
            })?;
        }
    }
    ensure!(
        repo.rejection_count()? == 0,
        "fixture offer rejected by actual board policy"
    );
    let app = zadp_api::board_router(repo).layer(middleware::map_response(label));
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    eprintln!(
        "Offline persistent discovery fixture ready (synthetic blocks, no real chain publication)"
    );
    axum::serve(listener, app).await?;
    Ok(())
}

//! Offline geo discovery: actual signatures, memo decoder, persistent board index and HTTP API.
//! Fixed public test bytes only: never use or fund this synthetic testnet descriptor.
use anyhow::{ensure, Result};
use axum::{
    body::{to_bytes, Body},
    http::Response,
    middleware,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use zadp_indexer::SqliteOfferRepository;
use zadp_protocol::*;
use zadp_zcash::{BlockHeader, BoardBlock, BoardOutput, TxId};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8098")]
    bind: SocketAddr,
}
async fn label(response: Response<Body>) -> Response<Body> {
    if !response
        .headers()
        .get("content-type")
        .is_some_and(|v| v.to_str().unwrap_or("").contains("application/json"))
    {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = to_bytes(body, 2_000_000).await.unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    if let Some(offers) = value["offers"].as_array_mut() {
        for offer in offers {
            offer["simulation"] = true.into();
        }
    }
    parts.headers.remove("content-length");
    Response::from_parts(parts, Body::from(serde_json::to_vec(&value).unwrap()))
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        args.bind.ip().is_loopback(),
        "offline fixture must bind to loopback"
    );
    let params = ZcashNetwork::Testnet.parameters();
    // Public deterministic cryptographic fixture, not a wallet or real board creation.
    let ivk = UnifiedSpendingKey::from_seed(&params, &[19; 64], AccountId::ZERO)?
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let operator = SellerKeyPair::from_secret_bytes(&[20; 32]);
    let board = BoardBody {
        zadp: BOARD_DESCRIPTOR_VERSION,
        board_id: "geo-fixture".into(),
        name: "Offline geographic fixture — never fund".into(),
        network: ZcashNetwork::Testnet,
        address: ivk
            .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)?
            .encode(&params),
        incoming_viewing_key: ivk.encode(&params),
        birthday_height: 3_000_000,
        policy: BoardPolicy::default(),
        operator_key: operator.id.clone(),
    }
    .sign(&operator)?;
    let repo = Arc::new(SqliteOfferRepository::open(&args.database, board.clone())?);
    if repo.cursor()?.is_none() {
        let now = DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
        let seller = SellerKeyPair::from_secret_bytes(&[21; 32]);
        let entries = [
            (
                "Bike repair — San Francisco area",
                Some(GeoArea {
                    lat_e5: 3777493,
                    lon_e5: -12241942,
                    radius_m: 50000,
                }),
            ),
            (
                "Home cleaning — Boston area",
                Some(GeoArea {
                    lat_e5: 4236000,
                    lon_e5: -7105900,
                    radius_m: 25000,
                }),
            ),
            ("Remote document formatting", None),
        ];
        let mut outputs = vec![];
        for (i, (title, geo)) in entries.into_iter().enumerate() {
            let mut draft = OfferDraft::new_ulid(
                seller.id.clone(),
                "fixture.discovery",
                title,
                Price::zec("0.01".parse()?),
                "https://fixture.invalid/not-a-service",
                now + Duration::days(7),
                B64.encode([i as u8; 16]),
            );
            if let Some(area) = geo {
                draft = draft.with_geo(area);
            }
            let offer = bind_offer(draft.sign(&seller)?, &board.board.board_id, now, &seller)?;
            outputs.push(BoardOutput {
                txid: TxId(format!("{i:064x}")),
                transaction_index: i as u64,
                output_index: 0,
                pool: "orchard".into(),
                value_zat: 1000,
                memo: encode_offer_memo(&offer)?.to_vec(),
            });
        }
        for offset in 0..3 {
            repo.apply_block(BoardBlock {
                header: BlockHeader {
                    height: board.board.birthday_height + offset,
                    hash: format!("{:064x}", offset + 1),
                    prev_hash: format!("{offset:064x}"),
                    time: now,
                },
                outputs: if offset == 0 {
                    std::mem::take(&mut outputs)
                } else {
                    vec![]
                },
            })?;
        }
    }
    ensure!(
        repo.rejection_count()? == 0,
        "fixture must pass actual board admission"
    );
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    eprintln!(
        "Offline geo fixture ready: signed v1/v2, synthetic blocks, no real publication or payment"
    );
    axum::serve(
        listener,
        zadp_api::board_router(repo).layer(middleware::map_response(label)),
    )
    .await?;
    Ok(())
}

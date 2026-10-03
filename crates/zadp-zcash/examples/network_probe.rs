//! Explicit read-only transport diagnostic; never contacts a wallet or sends funds.
use zadp_protocol::ZcashNetwork;
use zadp_zcash::connect_lightwalletd;
use zcash_client_backend::proto::service::{BlockId, ChainSpec};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .ok_or("provide a lightwalletd HTTPS URL")?;
    let mut client = connect_lightwalletd(&url, ZcashNetwork::Mainnet).await?;
    let tip = client.get_latest_block(ChainSpec {}).await?.into_inner();
    let block = client
        .get_block(BlockId {
            height: tip.height,
            hash: vec![],
        })
        .await?
        .into_inner();
    if block.height != tip.height || block.hash.len() != 32 || block.prev_hash.len() != 32 {
        return Err("invalid compact block metadata".into());
    }
    println!(
        "network=mainnet height={} hash_protocol_order={} transactions={} timestamp={}",
        block.height,
        hex::encode(block.hash),
        block.vtx.len(),
        block.time
    );
    Ok(())
}

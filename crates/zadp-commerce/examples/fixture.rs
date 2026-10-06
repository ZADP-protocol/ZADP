//! Ephemeral local fixture. No board, chain transaction, saved seed or funding.
use anyhow::{ensure, Result};
use chrono::{Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};
use zadp_commerce::{private_directory, Config, ObserverConfig, Snapshot};
use zadp_protocol::{OfferDraft, Price, SellerKeyPair, ZcashNetwork};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};
fn main() -> Result<()> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("usage: fixture NEW_PRIVATE_DIRECTORY"))?,
    );
    ensure!(!directory.exists(), "use a new fixture directory");
    private_directory(&directory)?;
    let directory = directory.canonicalize()?;
    let parameters = ZcashNetwork::Testnet.parameters();
    let mut seed = [0u8; 64];
    OsRng.fill_bytes(&mut seed);
    let address = UnifiedSpendingKey::from_seed(&parameters, &seed, AccountId::ZERO)?
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key()
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)?
        .encode(&parameters);
    // This synthetic receiver has no retained spending authority. Never fund it.
    seed.fill(0);
    let identity = SellerKeyPair::generate();
    let offer = OfferDraft::new_ulid(
        identity.id.clone(),
        "text.uppercase",
        "Uppercase text · local simulation",
        Price::zec(rust_decimal::Decimal::new(1, 2)),
        "http://127.0.0.1:8094",
        Utc::now() + Duration::hours(24),
        "bG9jYWwtZml4dHVyZQ==",
    )
    .sign(&identity)?;
    let config = Config {
        offer: offer.clone(),
        network: ZcashNetwork::Testnet,
        recipient: address,
        seller_account: "synthetic-seller".into(),
        database: directory.join("orders.sqlite"),
        confirmations: 3,
        order_lifetime_seconds: 1800,
        browser_origin: "http://127.0.0.1:8095".into(),
        observer: ObserverConfig::Mock {
            snapshot_file: directory.join("snapshot.json"),
        },
    };
    for (name, value) in [
        ("config.json", serde_json::to_value(config)?),
        (
            "snapshot.json",
            serde_json::to_value(Snapshot {
                synchronized: true,
                tip: 100,
                payments: vec![],
            })?,
        ),
    ] {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join(name))?
            .write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
    }
    let mut public = serde_json::to_value(offer)?;
    public["simulation"] = true.into();
    public["chain"] = serde_json::json!({"network":"testnet","block_height":0,"scanned_height":0,"confirmations":0,"transaction_id":"0".repeat(64),"offer_status":"active","simulation":true});
    fs::write(
        directory.join("offers.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"offers":[public]}))?,
    )?;
    println!("Local mock fixture ready. Never fund its synthetic receiver.");
    Ok(())
}

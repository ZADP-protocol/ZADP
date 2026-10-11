//! Read-only target resolution and exact approval for append-only close messages.
use anyhow::{ensure, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use zadp_protocol::{BoardDescriptor, Offer, OfferRevoke, SellerId};

pub fn resolve_target(
    index: &Path,
    board: &BoardDescriptor,
    seller: &SellerId,
    id: &str,
) -> Result<Offer> {
    let db = Connection::open_with_flags(
        index,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(std::time::Duration::from_secs(2))?;
    let fp: String = db.query_row(
        "SELECT value FROM metadata WHERE key='board_fingerprint'",
        [],
        |r| r.get(0),
    )?;
    ensure!(
        fp == board.fingerprint()?,
        "index descriptor fingerprint mismatch"
    );
    let raw: String = db
        .query_row(
            "SELECT json FROM offers WHERE seller=?1 AND id=?2",
            params![seller.as_str(), id],
            |r| r.get(0),
        )
        .context("target does not exist for this seller")?;
    let offer: Offer = serde_json::from_str(&raw)?;
    ensure!(
        offer.seller == *seller
            && offer.id == id
            && offer.board_id.as_deref() == Some(&board.board.board_id),
        "target seller/board/id mismatch"
    );
    zadp_protocol::verify_offer(&offer, offer.expires_at - chrono::Duration::seconds(1))?;
    let closed: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM offer_revocations WHERE seller=?1 AND offer_id=?2)",
        params![seller.as_str(), id],
        |r| r.get(0),
    )?;
    ensure!(!closed, "offer already revoked");
    Ok(offer)
}

pub fn validate_approval(
    p: &Value,
    revoke: &OfferRevoke,
    board: &BoardDescriptor,
    value: u64,
    fee: u64,
    pczt: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    revoke.validate_board(board, value, now)?;
    ensure!(
        p["kind"] == "unsigned-revocation-preview"
            && p["broadcast"] == false
            && p["zcash_transaction_signed"] == false,
        "invalid revoke approval preview"
    );
    ensure!(
        p["offer_revoke"] == serde_json::to_value(revoke)?
            && p["board_id"] == board.board.board_id
            && p["board_fingerprint"] == board.fingerprint()?
            && p["board_recipient"] == board.board.address
            && p["network"] == serde_json::to_value(board.board.network)?,
        "approved revoke/board changed"
    );
    ensure!(
        p["listing_value_zat"] == value
            && p["network_fee_zat"] == fee
            && p["total_cost_zat"] == value.checked_add(fee).context("cost overflow")?,
        "approved costs changed"
    );
    ensure!(
        p["proposal_fingerprint"] == hex::encode(Sha256::digest(pczt.as_bytes())),
        "approved proposal changed"
    );
    ensure!(
        p["preview_expires_at"]
            .as_i64()
            .is_some_and(|expiry| expiry > now.timestamp()),
        "approval expired"
    );
    Ok(())
}

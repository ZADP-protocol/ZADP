//! Seller-owned orders. Discovery never receives commerce data.
use anyhow::{ensure, Result};
use chrono::Utc;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use zadp_protocol::{verify_offer, Offer, ZcashNetwork};
use zcash_keys::address::Address;
use zcash_protocol::{memo::MemoBytes, value::Zatoshis};

pub mod http;
pub mod observer;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub offer: Offer,
    pub network: ZcashNetwork,
    pub recipient: String,
    pub seller_account: String,
    pub database: PathBuf,
    pub confirmations: u32,
    pub order_lifetime_seconds: i64,
    pub browser_origin: String,
    pub observer: ObserverConfig,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObserverConfig {
    Mock { snapshot_file: PathBuf },
    Zallet { rpc_url: String, auth_file: PathBuf },
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        verify_offer(
            &self.offer,
            self.offer
                .expires_at
                .checked_sub_signed(chrono::Duration::seconds(1))
                .ok_or_else(|| anyhow::anyhow!("invalid offer expiry"))?,
        )?;
        ensure!(
            self.offer.capability == "text.uppercase",
            "unsupported capability"
        );
        ensure!(
            self.confirmations > 0 && self.confirmations <= 100,
            "invalid confirmation policy"
        );
        ensure!(
            (60..=86400).contains(&self.order_lifetime_seconds),
            "invalid lifetime"
        );
        ensure!(!self.seller_account.is_empty(), "missing seller account");
        let recipient = Address::decode(&self.network.parameters(), &self.recipient)
            .ok_or_else(|| anyhow::anyhow!("recipient network/address mismatch"))?;
        ensure!(
            recipient
                .to_zcash_address(&self.network.parameters())
                .can_receive_memo(),
            "shielded recipient required"
        );
        let endpoint = url::Url::parse(&self.offer.endpoint)?;
        let origin = url::Url::parse(&self.browser_origin)?;
        ensure!(
            origin.origin().ascii_serialization() == self.browser_origin,
            "exact browser origin required"
        );
        let mock = matches!(self.observer, ObserverConfig::Mock { .. });
        ensure!(
            !mock || self.network == ZcashNetwork::Testnet,
            "mock service is testnet only"
        );
        for uri in [&endpoint, &origin] {
            ensure!(
                uri.username().is_empty()
                    && uri.password().is_none()
                    && uri.query().is_none()
                    && uri.fragment().is_none(),
                "credential-free endpoint/origin required"
            );
            ensure!(
                uri.scheme() == "https"
                    || (mock && uri.scheme() == "http" && uri.host_str() == Some("127.0.0.1")),
                "HTTPS required outside loopback demo"
            );
        }
        ensure!(
            endpoint.path() == "/" || endpoint.path().trim_end_matches('/').is_empty(),
            "reference endpoint must be an origin URL"
        );
        self.amount_zat()?;
        Ok(())
    }
    pub fn amount_zat(&self) -> Result<u64> {
        use rust_decimal::prelude::ToPrimitive;
        let zat = self.offer.price.amount * rust_decimal::Decimal::from(100_000_000u64);
        ensure!(zat.fract().is_zero(), "price exceeds ZEC precision");
        let value = zat
            .to_u64()
            .ok_or_else(|| anyhow::anyhow!("invalid amount"))?;
        Zatoshis::from_u64(value)?;
        ensure!(value > 0, "zero amount");
        Ok(value)
    }
    pub fn simulation(&self) -> bool {
        matches!(self.observer, ObserverConfig::Mock { .. })
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderRequest {
    pub offer_id: String,
    pub request: TextRequest,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextRequest {
    pub text: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub synchronized: bool,
    pub tip: u64,
    pub payments: Vec<PaymentEvidence>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentEvidence {
    pub txid: String,
    pub pool: String,
    pub output_index: u32,
    pub account: String,
    pub recipient: String,
    pub amount_zat: u64,
    pub memo: String,
    pub is_change: bool,
    pub mined_height: u64,
    pub block_time: i64,
}
impl PaymentEvidence {
    fn key(&self) -> String {
        format!("{}:{}:{}", self.txid, self.pool, self.output_index)
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct OrderResponse {
    pub order_id: String,
    pub offer_id: String,
    pub status: String,
    pub network: ZcashNetwork,
    pub simulation: bool,
    pub amount_zat: u64,
    pub amount_zec: String,
    pub recipient: String,
    pub memo: String,
    pub payment_request: String,
    pub payment_qr_svg: String,
    pub expires_at: i64,
    pub confirmations_required: u32,
    pub confirmations: u64,
    pub payment_check: String,
    pub transitions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}
struct Row {
    id: String,
    token_hash: String,
    request_hash: String,
    offer_id: String,
    recipient: String,
    account: String,
    amount: u64,
    memo: String,
    created: i64,
    expires: i64,
    status: String,
    input: String,
    result: Option<String>,
    payment: Option<String>,
    transitions: Vec<String>,
}
pub struct Store {
    db: Connection,
    config: Config,
}
pub fn random_hex(bytes: usize) -> String {
    let mut v = vec![0; bytes];
    OsRng.fill_bytes(&mut v);
    hex::encode(v)
}
fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn token(key: &str) -> String {
    hash(&format!("ZADP-COMMERCE-CLAIM/1:{key}"))
}
fn safe_equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}
fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Store {
    pub fn open(config: Config) -> Result<Self> {
        config.validate()?;
        let parent = config
            .database
            .parent()
            .ok_or_else(|| anyhow::anyhow!("database needs a private directory"))?;
        let dir = fs::symlink_metadata(parent)?;
        let owner = fs::metadata("/proc/self")?.uid();
        ensure!(
            dir.is_dir() && dir.uid() == owner && dir.mode() & 0o077 == 0,
            "database directory must be owned and private (0700)"
        );
        if !config.database.exists() {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&config.database)?;
        }
        let info = fs::symlink_metadata(&config.database)?;
        ensure!(
            info.is_file() && info.uid() == owner && info.mode() & 0o077 == 0,
            "database must be an owned protected regular file"
        );
        let db = Connection::open(&config.database)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        let foreign_tables: i64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT IN ('commerce_meta','orders') AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0))?;
        ensure!(
            foreign_tables == 0,
            "use a separate commerce database; existing non-commerce tables found"
        );

        db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS commerce_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS orders(
            id TEXT PRIMARY KEY,token_hash TEXT NOT NULL,key_hash TEXT UNIQUE NOT NULL,request_hash TEXT NOT NULL,
            offer_id TEXT NOT NULL,recipient TEXT NOT NULL,account TEXT NOT NULL,amount INTEGER NOT NULL,
            memo TEXT UNIQUE NOT NULL,created INTEGER NOT NULL,expires INTEGER NOT NULL,status TEXT NOT NULL,
            input TEXT NOT NULL,result TEXT,payment TEXT UNIQUE,transitions TEXT NOT NULL,execution_count INTEGER NOT NULL DEFAULT 0);")?;
        let version: Option<String> = db
            .query_row(
                "SELECT value FROM commerce_meta WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(
            version.as_deref().is_none_or(|v| v == "1"),
            "unsupported commerce schema; explicit migration required"
        );
        db.execute(
            "INSERT OR IGNORE INTO commerce_meta VALUES('schema_version','1')",
            [],
        )?;
        let fingerprint = hash(&serde_json::to_string(&config)?);
        let old: Option<String> = db
            .query_row(
                "SELECT value FROM commerce_meta WHERE key='config'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(
            old.as_ref().is_none_or(|v| v == &fingerprint),
            "commerce configuration changed; use a separate database or an explicit migration"
        );
        db.execute(
            "INSERT OR IGNORE INTO commerce_meta VALUES('config',?1)",
            [&fingerprint],
        )?;
        Ok(Self { db, config })
    }
    fn row(&self, id: &str) -> Result<Option<Row>> {
        Ok(self.db.query_row("SELECT id,token_hash,request_hash,offer_id,recipient,account,amount,memo,created,expires,status,input,result,payment,transitions FROM orders WHERE id=?1", [id], |r| {
            let transitions: String = r.get(14)?;
            Ok(Row {id:r.get(0)?,token_hash:r.get(1)?,request_hash:r.get(2)?,offer_id:r.get(3)?,recipient:r.get(4)?,account:r.get(5)?,amount:r.get(6)?,memo:r.get(7)?,created:r.get(8)?,expires:r.get(9)?,status:r.get(10)?,input:r.get(11)?,result:r.get(12)?,payment:r.get(13)?,transitions:serde_json::from_str(&transitions).unwrap_or_default()})
        }).optional()?)
    }
    pub fn create(&mut self, request: OrderRequest, key: &str, now: i64) -> Result<OrderResponse> {
        ensure!(is_hex(key, 64), "random 256-bit idempotency key required");
        ensure!(
            request.offer_id == self.config.offer.id
                && !request.request.text.trim().is_empty()
                && request.request.text.len() <= 4000,
            "invalid offer/request"
        );
        let digest = hash(&serde_json::to_string(&request)?);
        let existing: Option<String> = self
            .db
            .query_row(
                "SELECT id FROM orders WHERE key_hash=?1",
                [hash(key)],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            let row = self
                .row(&id)?
                .ok_or_else(|| anyhow::anyhow!("missing order"))?;
            ensure!(row.request_hash == digest, "idempotency conflict");
            return self.response(&row, Some(token(key)), "not_checked", 0, false);
        }
        ensure!(
            self.config.offer.expires_at.timestamp() > now,
            "offer expired"
        );
        let count: i64 = self
            .db
            .query_row("SELECT count(*) FROM orders", [], |r| r.get(0))?;
        let active: i64 = self.db.query_row(
            "SELECT count(*) FROM orders WHERE status IN ('waiting_payment','paid','processing')",
            [],
            |r| r.get(0),
        )?;
        ensure!(
            count < 10_000 && active < 100,
            "seller order capacity reached"
        );
        let id = random_hex(16);
        let memo = format!("ZADP-COMMERCE/1:{id}:{}", random_hex(32));
        let amount = self.config.amount_zat()?;
        let expires = (now + self.config.order_lifetime_seconds)
            .min(self.config.offer.expires_at.timestamp());
        self.db.execute("INSERT INTO orders(id,token_hash,key_hash,request_hash,offer_id,recipient,account,amount,memo,created,expires,status,input,transitions) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'waiting_payment',?12,?13)", params![id,hash(&token(key)),hash(key),digest,request.offer_id,self.config.recipient,self.config.seller_account,amount,memo,now,expires,request.request.text,r#"["created","waiting_payment"]"#])?;
        self.response(
            &self.row(&id)?.unwrap(),
            Some(token(key)),
            "not_checked",
            0,
            false,
        )
    }
    fn authenticated(&self, id: &str, claim: &str) -> Result<Row> {
        let row = self
            .row(id)?
            .ok_or_else(|| anyhow::anyhow!("unknown order or invalid claim token"))?;
        ensure!(
            is_hex(claim, 64) && safe_equal(&row.token_hash, &hash(claim)),
            "unknown order or invalid claim token"
        );
        Ok(row)
    }
    pub fn cancel(&mut self, id: &str, claim: &str) -> Result<OrderResponse> {
        let mut row = self.authenticated(id, claim)?;
        ensure!(
            row.status == "waiting_payment" || row.status == "cancelled",
            "order cannot be cancelled"
        );
        if row.status != "cancelled" {
            self.transition(&mut row, "cancelled")?;
        }
        self.response(&row, None, "not_checked", 0, false)
    }
    fn transition(&mut self, row: &mut Row, status: &str) -> Result<()> {
        let allowed = matches!(
            (row.status.as_str(), status),
            ("waiting_payment", "paid" | "expired" | "cancelled")
                | ("paid", "processing")
                | ("processing", "completed" | "failed")
        );
        ensure!(allowed, "invalid state transition");
        row.status = status.into();
        row.transitions.push(status.into());
        self.db.execute(
            "UPDATE orders SET status=?1,transitions=?2 WHERE id=?3",
            params![row.status, serde_json::to_string(&row.transitions)?, row.id],
        )?;
        Ok(())
    }
    pub fn check(
        &mut self,
        id: &str,
        claim: &str,
        snapshot: &Snapshot,
        now: i64,
    ) -> Result<OrderResponse> {
        let mut row = self.authenticated(id, claim)?;
        if ["cancelled", "expired", "failed"].contains(&row.status.as_str()) {
            return self.response(&row, None, "not_checked", 0, false);
        }
        if !snapshot.synchronized || snapshot.tip == 0 {
            return self.response(&row, None, "observer_unavailable", 0, false);
        }
        let mut matches: Vec<&PaymentEvidence> = snapshot
            .payments
            .iter()
            .filter(|p| {
                p.account == row.account
                    && p.recipient == row.recipient
                    && p.amount_zat == row.amount
                    && p.memo == row.memo
                    && !p.is_change
                    && ["sapling", "orchard", "ironwood"].contains(&p.pool.as_str())
                    && is_hex(&p.txid, 64)
                    && p.mined_height > 0
                    && p.mined_height <= snapshot.tip
                    && (row.created..=row.expires).contains(&p.block_time)
            })
            .collect();
        matches.sort_by_key(|p| (p.mined_height, p.key()));
        let evidence = matches
            .iter()
            .copied()
            .find(|p| row.payment.as_ref().is_none_or(|key| key == &p.key()));
        let Some(p) = evidence else {
            if row.status == "waiting_payment" && now > row.expires {
                self.transition(&mut row, "expired")?;
            }
            return self.response(
                &row,
                None,
                if row.payment.is_some() {
                    "payment_evidence_changed"
                } else {
                    "waiting_payment"
                },
                0,
                false,
            );
        };
        let confirmations = snapshot.tip - p.mined_height + 1;
        if confirmations < u64::from(self.config.confirmations) {
            return self.response(&row, None, "confirming", confirmations, false);
        }
        if row.status == "waiting_payment" {
            // Unique output binding and PAID transition commit together before execution.
            let tx = self.db.transaction()?;
            let mut history = row.transitions.clone();
            history.push("paid".into());
            let update = tx.execute(
                "UPDATE orders SET payment=?1,status='paid',transitions=?2 WHERE id=?3",
                params![p.key(), serde_json::to_string(&history)?, row.id],
            );
            if update.is_err() {
                drop(tx);
                return self.response(&row, None, "payment_already_used", confirmations, false);
            }
            tx.commit()?;
            row = self.row(id)?.unwrap();
            return self.response(&row, None, "verified", confirmations, false);
        }
        if row.status == "paid" {
            self.transition(&mut row, "processing")?;
            return self.response(&row, None, "verified", confirmations, false);
        }
        if row.status == "processing" {
            // Trivial deterministic execution is atomic with result storage. No external effects.
            let result = row.input.to_uppercase();
            if result.len() > 12000 {
                self.transition(&mut row, "failed")?;
            } else {
                let tx = self.db.transaction()?;
                row.transitions.push("completed".into());
                tx.execute("UPDATE orders SET status='completed',result=?1,execution_count=execution_count+1,transitions=?2 WHERE id=?3 AND status='processing'",params![result,serde_json::to_string(&row.transitions)?,id])?;
                tx.commit()?;
                row = self.row(id)?.unwrap();
            }
        }
        self.response(&row, None, "verified", confirmations, true)
    }
    fn response(
        &self,
        row: &Row,
        claim: Option<String>,
        check: &str,
        confirmations: u64,
        release: bool,
    ) -> Result<OrderResponse> {
        let address = Address::decode(&self.config.network.parameters(), &row.recipient)
            .ok_or_else(|| anyhow::anyhow!("invalid stored address"))?
            .to_zcash_address(&self.config.network.parameters());
        let payment = zip321::Payment::new(
            address,
            Some(Zatoshis::from_u64(row.amount)?),
            Some(MemoBytes::from_bytes(row.memo.as_bytes())?),
            None,
            Some("ZADP seller order".into()),
            vec![],
        )?;
        let uri = zip321::TransactionRequest::new(vec![payment])?.to_uri();
        let qr = qrcode::QrCode::new(uri.as_bytes())?
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(256, 256)
            .build();
        Ok(OrderResponse {
            order_id: row.id.clone(),
            offer_id: row.offer_id.clone(),
            status: row.status.clone(),
            network: self.config.network,
            simulation: self.config.simulation(),
            amount_zat: row.amount,
            amount_zec: format!(
                "{}.{:08}",
                row.amount / 100_000_000,
                row.amount % 100_000_000
            ),
            recipient: row.recipient.clone(),
            memo: row.memo.clone(),
            payment_request: uri,
            payment_qr_svg: qr,
            expires_at: row.expires,
            confirmations_required: self.config.confirmations,
            confirmations,
            payment_check: check.into(),
            transitions: row.transitions.clone(),
            claim_token: claim,
            result: if release && row.status == "completed" {
                row.result.clone()
            } else {
                None
            },
        })
    }
}

pub fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn now() -> i64 {
    Utc::now().timestamp()
}

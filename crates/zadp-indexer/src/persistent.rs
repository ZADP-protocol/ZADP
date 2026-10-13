//! Transactional board index and scan cursor. No wallet database or secrets.
use crate::{ChainObservation, IndexerError, OfferRecord, OfferRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{path::Path, sync::Mutex};
use zadp_protocol::{decode_zadp_memo, BoardDescriptor, Offer, OfferRevoke, ZadpMessage};
use zadp_zcash::{BlockHeader, BoardBlock, TxId};

#[derive(Debug, Serialize)]
pub struct PublicationAudit {
    pub transaction_id: String,
    pub protocol_valid: bool,
    pub board_allowed: bool,
    pub reason: Option<String>,
    pub offer: Option<Offer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_revoke: Option<OfferRevoke>,
}

#[derive(Debug, Serialize)]
pub struct OfferLifecycle {
    pub status: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revocation_txid: Option<String>,
}

pub struct SqliteOfferRepository {
    connection: Mutex<Connection>,
    board: BoardDescriptor,
}

fn state(reason: &str) -> IndexerError {
    IndexerError::State(reason.into())
}
impl SqliteOfferRepository {
    pub fn open(path: impl AsRef<Path>, board: BoardDescriptor) -> Result<Self, IndexerError> {
        board.validate(board.board.network)?;
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(10))?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY,value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS blocks (height INTEGER PRIMARY KEY,hash TEXT NOT NULL,prev_hash TEXT NOT NULL,time INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS offers (seller TEXT NOT NULL,id TEXT NOT NULL,nonce TEXT NOT NULL,json TEXT NOT NULL,txid TEXT NOT NULL,pool TEXT NOT NULL,output_index INTEGER NOT NULL,value_zat INTEGER NOT NULL,height INTEGER NOT NULL REFERENCES blocks(height) ON DELETE CASCADE,block_time INTEGER NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(seller,id),UNIQUE(seller,nonce),UNIQUE(txid,pool,output_index));
            CREATE INDEX IF NOT EXISTS offers_seller_time ON offers(seller,block_time);
            CREATE TABLE IF NOT EXISTS rejections (height INTEGER NOT NULL REFERENCES blocks(height) ON DELETE CASCADE,txid TEXT NOT NULL,pool TEXT NOT NULL,output_index INTEGER NOT NULL,reason TEXT NOT NULL,PRIMARY KEY(txid,pool,output_index));")?;
        let fingerprint = board.fingerprint()?;
        let tx = connection.transaction()?;
        let old: Option<String> = tx
            .query_row(
                "SELECT value FROM metadata WHERE key='board_fingerprint'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if old.as_ref().is_some_and(|v| v != &fingerprint) {
            return Err(state(
                "database belongs to a different signed board descriptor; use a new database",
            ));
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS publications (
            height INTEGER NOT NULL REFERENCES blocks(height) ON DELETE CASCADE,
            txid TEXT NOT NULL,pool TEXT NOT NULL,output_index INTEGER NOT NULL,
            protocol_valid INTEGER NOT NULL,board_allowed INTEGER NOT NULL,reason TEXT,json TEXT,
            PRIMARY KEY(txid,pool,output_index));
            CREATE TABLE IF NOT EXISTS offer_revocations (
            seller TEXT NOT NULL,offer_id TEXT NOT NULL,nonce TEXT NOT NULL,json TEXT NOT NULL,
            txid TEXT NOT NULL,pool TEXT NOT NULL,output_index INTEGER NOT NULL,
            transaction_index INTEGER NOT NULL,height INTEGER NOT NULL REFERENCES blocks(height) ON DELETE CASCADE,
            block_time INTEGER NOT NULL,value_zat INTEGER NOT NULL,
            PRIMARY KEY(seller,offer_id),UNIQUE(seller,nonce),UNIQUE(txid,pool,output_index),
            FOREIGN KEY(seller,offer_id) REFERENCES offers(seller,id) ON DELETE CASCADE);
            ",
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO metadata VALUES ('board_fingerprint',?1)",
            [fingerprint],
        )?;
        tx.execute("INSERT OR IGNORE INTO publications SELECT height,txid,pool,output_index,1,1,NULL,json FROM offers", [])?;
        tx.commit()?;
        Ok(Self {
            connection: Mutex::new(connection),
            board,
        })
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, IndexerError> {
        self.connection
            .lock()
            .map_err(|_| state("database lock poisoned"))
    }
    /// Execute an application-owned SQLite extension under the canonical repository lock.
    /// Extensions own their tables; Core does not depend on or invoke an extension.
    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, IndexerError>,
    ) -> Result<T, IndexerError> {
        let mut connection = self.lock()?;
        operation(&mut connection)
    }
    pub fn board(&self) -> &BoardDescriptor {
        &self.board
    }
    pub fn cursor(&self) -> Result<Option<BlockHeader>, IndexerError> {
        let c = self.lock()?;
        c.query_row(
            "SELECT height,hash,prev_hash,time FROM blocks ORDER BY height DESC LIMIT 1",
            [],
            read_header,
        )
        .optional()
        .map_err(Into::into)
    }
    pub fn header(&self, height: u32) -> Result<Option<BlockHeader>, IndexerError> {
        self.lock()?
            .query_row(
                "SELECT height,hash,prev_hash,time FROM blocks WHERE height=?1",
                [height],
                read_header,
            )
            .optional()
            .map_err(Into::into)
    }
    /// Atomic rollback removes orphaned offers/rejections through FK cascades.
    pub fn rewind(&self, ancestor: Option<u32>) -> Result<(), IndexerError> {
        let mut c = self.lock()?;
        let tx = c.transaction()?;
        match ancestor {
            Some(h) => {
                tx.execute("DELETE FROM blocks WHERE height>?1", [h])?;
            }
            None => {
                tx.execute("DELETE FROM blocks", [])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn rejection_count(&self) -> Result<u64, IndexerError> {
        Ok(self
            .lock()?
            .query_row("SELECT count(*) FROM rejections", [], |r| r.get(0))?)
    }
    pub fn rejections(&self, limit: u32) -> Result<Vec<(String, String)>, IndexerError> {
        let c = self.lock()?;
        let mut s =
            c.prepare("SELECT txid,reason FROM rejections ORDER BY height DESC,txid LIMIT ?1")?;
        let rows = s.query_map([limit.min(1000)], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let result = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }
    /// Evaluate the board contract in chain order, not wall-clock ingestion order.
    pub fn apply_block(&self, mut block: BoardBlock) -> Result<usize, IndexerError> {
        let mut c = self.lock()?;
        let tx = c.transaction()?;
        let h = &block.header;
        if h.height < self.board.board.birthday_height
            || h.hash.len() != 64
            || h.prev_hash.len() != 64
        {
            return Err(state(
                "invalid block metadata or block before board birthday",
            ));
        }
        let existing: Option<String> = tx
            .query_row("SELECT hash FROM blocks WHERE height=?1", [h.height], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(hash) = existing {
            return if hash == h.hash {
                Ok(0)
            } else {
                Err(state("conflicting block requires rewind"))
            };
        }
        let cursor = tx
            .query_row(
                "SELECT height,hash FROM blocks ORDER BY height DESC LIMIT 1",
                [],
                |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        match cursor {
            Some((height, hash))
                if height.checked_add(1) != Some(h.height) || hash != h.prev_hash =>
            {
                return Err(state("non-contiguous block or wrong parent"))
            }
            None if h.height != self.board.board.birthday_height => {
                return Err(state("scan must begin at board birthday"))
            }
            _ => {}
        }
        tx.execute(
            "INSERT INTO blocks VALUES (?1,?2,?3,?4)",
            params![h.height, h.hash, h.prev_hash, h.time.timestamp()],
        )?;
        block.outputs.sort_by(|a, b| {
            (a.transaction_index, &a.pool, a.output_index).cmp(&(
                b.transaction_index,
                &b.pool,
                b.output_index,
            ))
        });
        let mut inserted = 0;
        for output in block.outputs {
            let decoded = match decode_zadp_memo(&output.memo) {
                Ok(None) => continue,
                Ok(Some(ZadpMessage::Offer(o))) => Ok(o),
                Ok(Some(ZadpMessage::OfferRevoke(r))) => {
                    let reason = self.accept_revoke(&tx, &r, &output, h)?;
                    let valid = r.verify().is_ok() && r.board_id == self.board.board.board_id;
                    tx.execute(
                        "INSERT OR IGNORE INTO publications VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![
                            h.height,
                            output.txid.0,
                            output.pool,
                            output.output_index,
                            valid,
                            reason.is_none(),
                            reason,
                            serde_json::to_string(&r)
                                .map_err(|_| state("revoke serialization failed"))?
                        ],
                    )?;
                    if let Some(reason) = reason {
                        tx.execute(
                            "INSERT OR IGNORE INTO rejections VALUES (?1,?2,?3,?4,?5)",
                            params![
                                h.height,
                                output.txid.0,
                                output.pool,
                                output.output_index,
                                reason
                            ],
                        )?;
                    }
                    continue;
                }
                Err(_) => Err("malformed_zadp_memo"),
            };
            let publication_json = decoded
                .as_ref()
                .ok()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|_| state("offer serialization failed"))?;
            let publication_valid = decoded
                .as_ref()
                .is_ok_and(|o| self.board.validate_publication(o, h.time).is_ok());
            let rejection = match decoded {
                Err(reason) => Some(reason),
                Ok(offer) => {
                    if let Err(error) = self.board.validate_offer(&offer, output.value_zat, h.time)
                    {
                        use zadp_protocol::ProtocolError;
                        Some(match error {
                            ProtocolError::SignatureMismatch
                            | ProtocolError::InvalidSignature(_)
                            | ProtocolError::InvalidSellerId(_) => "invalid_seller_signature",
                            ProtocolError::Expired(_) => "expired_offer",
                            ProtocolError::UnsupportedVersion(_) => "unsupported_version",
                            ProtocolError::UnsupportedType(_) => "unsupported_message_type",
                            ProtocolError::InvalidOffer(reason) => match reason.as_str() {
                                "wrong board binding" => "wrong_board",
                                "listing value below board minimum" => {
                                    "listing_value_below_minimum"
                                }
                                "seller is not authorized" => "unauthorized_seller",
                                "invalid offer TTL or future issue time" => {
                                    "invalid_ttl_or_issue_time"
                                }
                                _ => "invalid_offer_semantics",
                            },
                            _ => "invalid_offer_encoding",
                        })
                    } else {
                        let seen:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM offers WHERE (seller=?1 AND (id=?2 OR nonce=?3)) OR (txid=?4 AND pool=?5 AND output_index=?6)) OR EXISTS(SELECT 1 FROM offer_revocations WHERE (seller=?1 AND nonce=?3) OR (txid=?4 AND pool=?5 AND output_index=?6))",params![offer.seller.as_str(),offer.id,offer.nonce,output.txid.0,output.pool,output.output_index],|r|r.get(0))?;
                        let counts:(u32,u32)=tx.query_row("SELECT (SELECT COUNT(*) FROM offers o WHERE seller=?1 AND expires>?2 AND NOT EXISTS(SELECT 1 FROM offer_revocations r WHERE r.seller=o.seller AND r.offer_id=o.id)), (SELECT COUNT(*) FROM offers WHERE seller=?1 AND block_time>?3)+(SELECT COUNT(*) FROM offer_revocations WHERE seller=?1 AND block_time>?3)",params![offer.seller.as_str(),h.time.timestamp(),h.time.timestamp()-i64::from(self.board.board.policy.rate_window_seconds)],|r|Ok((r.get(0)?,r.get(1)?)))?;
                        if seen {
                            Some("duplicate_offer_nonce_or_output")
                        } else if counts.0 >= self.board.board.policy.max_active_offers_per_seller {
                            Some("active_offer_limit")
                        } else if counts.1
                            >= self
                                .board
                                .board
                                .policy
                                .max_publications_per_seller_per_window
                        {
                            Some("seller_rate_limit")
                        } else {
                            let json = serde_json::to_string(&offer)
                                .map_err(|_| state("offer serialization failed"))?;
                            tx.execute(
                                "INSERT INTO offers VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                                params![
                                    offer.seller.as_str(),
                                    offer.id,
                                    offer.nonce,
                                    json,
                                    output.txid.0,
                                    output.pool,
                                    output.output_index,
                                    output.value_zat,
                                    h.height,
                                    h.time.timestamp(),
                                    offer.expires_at.timestamp()
                                ],
                            )?;
                            inserted += 1;
                            None
                        }
                    }
                }
            };
            let protocol_valid =
                publication_valid && rejection != Some("duplicate_offer_nonce_or_output");
            tx.execute(
                "INSERT OR IGNORE INTO publications VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    h.height,
                    output.txid.0,
                    output.pool,
                    output.output_index,
                    protocol_valid,
                    rejection.is_none(),
                    rejection,
                    publication_json
                ],
            )?;
            if let Some(reason) = rejection {
                tx.execute(
                    "INSERT OR IGNORE INTO rejections VALUES (?1,?2,?3,?4,?5)",
                    params![
                        h.height,
                        output.txid.0,
                        output.pool,
                        output.output_index,
                        reason
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }
    fn accept_revoke(
        &self,
        tx: &rusqlite::Transaction<'_>,
        r: &OfferRevoke,
        output: &zadp_zcash::BoardOutput,
        h: &BlockHeader,
    ) -> Result<Option<&'static str>, IndexerError> {
        if r.validate_board(&self.board, output.value_zat, h.time)
            .is_err()
        {
            return Ok(Some("invalid_revoke_board_policy"));
        }
        // Existence at this point proves the original preceded this event in sorted chain order.
        let original: Option<String> = tx
            .query_row(
                "SELECT json FROM offers WHERE seller=?1 AND id=?2",
                params![r.seller.as_str(), r.offer_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(original) = original else {
            return Ok(Some("unknown_offer_or_wrong_seller"));
        };
        let original: Offer =
            serde_json::from_str(&original).map_err(|_| state("corrupt stored offer"))?;
        if original.issued_at.is_some_and(|t| r.issued_at < t) {
            return Ok(Some("revoke_predates_offer_issue"));
        }
        let seen:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM offer_revocations WHERE (seller=?1 AND (offer_id=?2 OR nonce=?3)) OR (txid=?4 AND pool=?5 AND output_index=?6)) OR EXISTS(SELECT 1 FROM offers WHERE (seller=?1 AND nonce=?3) OR (txid=?4 AND pool=?5 AND output_index=?6))",params![r.seller.as_str(),r.offer_id,r.nonce,output.txid.0,output.pool,output.output_index],|row|row.get(0))?;
        if seen {
            return Ok(Some("duplicate_revoke_or_nonce_or_output"));
        }
        let count:u32=tx.query_row("SELECT (SELECT COUNT(*) FROM offers WHERE seller=?1 AND block_time>?2)+(SELECT COUNT(*) FROM offer_revocations WHERE seller=?1 AND block_time>?2)",params![r.seller.as_str(),h.time.timestamp()-i64::from(self.board.board.policy.rate_window_seconds)],|row|row.get(0))?;
        if count
            >= self
                .board
                .board
                .policy
                .max_publications_per_seller_per_window
        {
            return Ok(Some("seller_rate_limit"));
        }
        tx.execute(
            "INSERT INTO offer_revocations VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                r.seller.as_str(),
                r.offer_id,
                r.nonce,
                serde_json::to_string(r).map_err(|_| state("revoke serialization failed"))?,
                output.txid.0,
                output.pool,
                output.output_index,
                output.transaction_index,
                h.height,
                h.time.timestamp(),
                output.value_zat
            ],
        )?;
        Ok(None)
    }
    /// Historical lookup: original signed Offer JSON is never changed by a control event.
    pub fn historical_offer(&self, seller: &str, id: &str) -> Result<Option<Offer>, IndexerError> {
        let json: Option<String> = self
            .lock()?
            .query_row(
                "SELECT json FROM offers WHERE seller=?1 AND id=?2",
                params![seller, id],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(|_| state("corrupt stored offer")))
            .transpose()
    }
    /// Revocation takes precedence over expiry for historical lifecycle reporting.
    pub fn lifecycle(
        &self,
        seller: &str,
        id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<OfferLifecycle>, IndexerError> {
        let row:Option<(i64,Option<i64>,Option<String>)>=self.lock()?.query_row("SELECT o.expires,r.block_time,r.txid FROM offers o LEFT JOIN offer_revocations r ON r.seller=o.seller AND r.offer_id=o.id WHERE o.seller=?1 AND o.id=?2",params![seller,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        Ok(row.map(|(expiry, revoked, txid)| OfferLifecycle {
            status: if revoked.is_some() {
                "revoked"
            } else if expiry <= now.timestamp() {
                "expired"
            } else {
                "active"
            }
            .into(),
            revoked_at: revoked.and_then(|t| DateTime::from_timestamp(t, 0)),
            revocation_txid: txid,
        }))
    }
    pub fn canonical_records(
        &self,
        capability: Option<&str>,
    ) -> Result<Vec<OfferRecord>, IndexerError> {
        self.filtered_records(capability, None, |_, _| Ok(true))
    }
    pub fn publication_audit(&self, limit: u32) -> Result<Vec<PublicationAudit>, IndexerError> {
        let c = self.lock()?;
        let mut s=c.prepare("SELECT txid,protocol_valid,board_allowed,reason,json FROM publications ORDER BY height,txid,pool,output_index LIMIT ?1")?;
        let rows = s.query_map([limit.min(1000)], |r| {
            let json = r.get::<_, Option<String>>(4)?;
            let control = json.as_ref().is_some_and(|j| {
                serde_json::from_str::<serde_json::Value>(j)
                    .ok()
                    .is_some_and(|v| v["type"] == "offer_revoke")
            });
            Ok(PublicationAudit {
                offer_revoke: if control {
                    json.as_ref()
                        .map(|j| serde_json::from_str(j))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?
                } else {
                    None
                },
                transaction_id: r.get(0)?,
                protocol_valid: r.get(1)?,
                board_allowed: r.get(2)?,
                reason: r.get(3)?,
                offer: if control {
                    None
                } else {
                    json.map(|j| serde_json::from_str(&j))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?
                },
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    /// Generic local storage extension: filters active canonical records before pagination.
    /// The callback executes under the repository lock. Trusted application extensions
    /// must not rewrite canonical data; this in-process interface is not a sandbox.
    pub fn filtered_records(
        &self,
        capability: Option<&str>,
        id: Option<&str>,
        accepts: impl Fn(&Connection, &Offer) -> Result<bool, IndexerError>,
    ) -> Result<Vec<OfferRecord>, IndexerError> {
        let c = self.lock()?;
        let scanned_height: Option<u32> =
            c.query_row("SELECT MAX(height) FROM blocks", [], |r| r.get(0))?;
        let mut stmt=c.prepare("SELECT json,txid,pool,output_index,value_zat,o.height,block_time,b.hash FROM offers o JOIN blocks b ON b.height=o.height WHERE expires>?1 AND NOT EXISTS (SELECT 1 FROM offer_revocations r WHERE r.seller=o.seller AND r.offer_id=o.id) AND (?2 IS NULL OR json_extract(json, '$.capability')=?2) AND (?3 IS NULL OR o.id=?3) ORDER BY o.height,o.txid,o.pool,o.output_index")?;
        let rows = stmt.query_map(params![Utc::now().timestamp(), capability, id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u32>(3)?,
                r.get::<_, u64>(4)?,
                r.get::<_, u32>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        let mut out = vec![];
        for row in rows {
            let (json, txid, pool, output_index, value, height, time, hash) = row?;
            let offer: Offer =
                serde_json::from_str(&json).map_err(|_| state("corrupt stored offer"))?;
            if capability.is_some_and(|c| offer.capability != c)
                || id.is_some_and(|i| offer.id != i)
            {
                continue;
            }
            if !accepts(&c, &offer)? {
                continue;
            }
            let confirmations = scanned_height.unwrap_or(height).saturating_sub(height) + 1;
            if out.len() >= 1000 {
                break;
            }
            out.push(OfferRecord {
                offer,
                source_txid: TxId(txid.clone()),
                indexed_at: DateTime::from_timestamp(time, 0)
                    .ok_or_else(|| state("corrupt block time"))?,
                chain: Some(ChainObservation {
                    network: self.board.board.network,
                    board_id: self.board.board.board_id.clone(),
                    block_height: height,
                    block_hash: hash,
                    transaction_id: txid,
                    pool,
                    output_index,
                    listing_value_zat: value,
                    scanned_height: scanned_height.unwrap_or(height),
                    confirmations,
                    confirmation_status: if confirmations
                        >= self.board.board.policy.confirmed_after_blocks
                    {
                        "confirmed"
                    } else {
                        "seen"
                    }
                    .into(),
                    offer_status: "active".into(),
                }),
            });
        }
        // Pagination is applied after any optional application filter.
        Ok(out.into_iter().take(1000).collect())
    }
}
fn read_header(r: &rusqlite::Row<'_>) -> rusqlite::Result<BlockHeader> {
    let timestamp: i64 = r.get(3)?;
    Ok(BlockHeader {
        height: r.get(0)?,
        hash: r.get(1)?,
        prev_hash: r.get(2)?,
        time: DateTime::from_timestamp(timestamp, 0).ok_or(rusqlite::Error::InvalidQuery)?,
    })
}
#[async_trait]
impl OfferRepository for SqliteOfferRepository {
    async fn upsert_first_wins(&self, _record: OfferRecord) -> Result<bool, IndexerError> {
        Err(state(
            "persistent board offers must be committed atomically with their source block",
        ))
    }
    async fn list(&self, capability: Option<&str>) -> Result<Vec<OfferRecord>, IndexerError> {
        self.filtered_records(capability, None, |_, _| Ok(true))
    }
    async fn get(&self, id: &str) -> Result<Option<OfferRecord>, IndexerError> {
        Ok(self
            .filtered_records(None, Some(id), |_, _| Ok(true))?
            .into_iter()
            .next())
    }
}

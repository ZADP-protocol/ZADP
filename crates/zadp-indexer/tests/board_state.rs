use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rust_decimal::Decimal;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};
use zadp_indexer::*;
use zadp_protocol::*;
use zadp_zcash::*;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zip32::{AccountId, DiversifierIndex};
const BIRTHDAY: u32 = 3_000_000;
fn descriptor(policy: BoardPolicy) -> BoardDescriptor {
    let keys = SellerKeyPair::generate();
    let params = ZcashNetwork::Mainnet.parameters();
    let mut seed = [0u8; 64];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let ivk = UnifiedSpendingKey::from_seed(&params, &seed, AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let ua = ivk
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap();
    BoardBody {
        zadp: 1,
        board_id: "ai-services".into(),
        name: "Test board".into(),
        network: ZcashNetwork::Mainnet,
        address: ua.encode(&params),
        incoming_viewing_key: ivk.encode(&params),
        birthday_height: BIRTHDAY,
        policy,
        operator_key: keys.id.clone(),
    }
    .sign(&keys)
    .unwrap()
}
fn offer(keys: &SellerKeyPair, nonce: u8) -> Offer {
    let now = DateTime::from_timestamp(Utc::now().timestamp() - 100, 0).unwrap();
    let draft = OfferDraft::new_ulid(
        keys.id.clone(),
        "llm.inference",
        "test",
        Price::zec(Decimal::new(1, 2)),
        "https://example.com/zadp",
        now + Duration::hours(24),
        B64.encode([nonce; 16]),
    )
    .sign(keys)
    .unwrap();
    bind_offer(draft, "ai-services", now, keys).unwrap()
}
fn block(height: u32, hash: u32, prev: u32, offers: Vec<Offer>) -> BoardBlock {
    BoardBlock {
        header: BlockHeader {
            height,
            hash: format!("{hash:064x}"),
            prev_hash: format!("{prev:064x}"),
            time: DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap(),
        },
        outputs: offers
            .into_iter()
            .enumerate()
            .map(|(i, o)| BoardOutput {
                txid: TxId(format!("{hash:060x}{i:04x}")),
                transaction_index: i as u64,
                output_index: 0,
                pool: "orchard".into(),
                value_zat: 1000,
                memo: encode_offer_memo(&o).unwrap().to_vec(),
            })
            .collect(),
    }
}
#[derive(Default)]
struct Source(RwLock<BTreeMap<u32, BoardBlock>>);
#[async_trait]
impl BoardSource for Source {
    async fn tip_height(&self) -> Result<u32, ZcashError> {
        Ok(*self.0.read().unwrap().last_key_value().unwrap().0)
    }
    async fn header(&self, height: u32) -> Result<BlockHeader, ZcashError> {
        Ok(self.0.read().unwrap().get(&height).unwrap().header.clone())
    }
    async fn block(&self, height: u32) -> Result<BoardBlock, ZcashError> {
        Ok(self.0.read().unwrap().get(&height).unwrap().clone())
    }
}
#[tokio::test]
async fn restart_cursor_persistence_duplicate_block_and_replay() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.sqlite");
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let o = offer(&seller, 1);
    let source = Arc::new(Source::default());
    source
        .0
        .write()
        .unwrap()
        .insert(BIRTHDAY, block(BIRTHDAY, 1, 0, vec![o.clone()]));
    {
        let repo = Arc::new(SqliteOfferRepository::open(&path, board.clone()).unwrap());
        let indexer = BoardIndexer::new(Arc::clone(&source), Arc::clone(&repo));
        assert_eq!(indexer.sync_once(100).await.unwrap(), 1);
        assert_eq!(indexer.sync_once(100).await.unwrap(), 0);
        assert_eq!(
            repo.apply_block(source.block(BIRTHDAY).await.unwrap())
                .unwrap(),
            0
        );
        assert_eq!(repo.cursor().unwrap().unwrap().height, BIRTHDAY);
    }
    let repo = Arc::new(SqliteOfferRepository::open(&path, board).unwrap());
    source
        .0
        .write()
        .unwrap()
        .insert(BIRTHDAY + 1, block(BIRTHDAY + 1, 2, 1, vec![o]));
    let indexer = BoardIndexer::new(source, Arc::clone(&repo));
    assert_eq!(indexer.sync_once(100).await.unwrap(), 0);
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    assert_eq!(repo.rejection_count().unwrap(), 1);
    assert_eq!(repo.cursor().unwrap().unwrap().height, BIRTHDAY + 1);
}
#[tokio::test]
async fn reorg_removes_orphans_and_independent_indexer_reconstructs() {
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let first = offer(&seller, 1);
    let second = offer(&seller, 2);
    let replacement = offer(&seller, 3);
    let source = Arc::new(Source::default());
    source
        .0
        .write()
        .unwrap()
        .insert(BIRTHDAY, block(BIRTHDAY, 1, 0, vec![first]));
    source.0.write().unwrap().insert(
        BIRTHDAY + 1,
        block(BIRTHDAY + 1, 2, 1, vec![second.clone()]),
    );
    let repo = Arc::new(SqliteOfferRepository::open(":memory:", board.clone()).unwrap());
    let indexer = BoardIndexer::new(Arc::clone(&source), Arc::clone(&repo));
    assert_eq!(indexer.sync_once(100).await.unwrap(), 2);
    source.0.write().unwrap().insert(
        BIRTHDAY + 1,
        block(BIRTHDAY + 1, 20, 1, vec![replacement.clone()]),
    );
    source
        .0
        .write()
        .unwrap()
        .insert(BIRTHDAY + 2, block(BIRTHDAY + 2, 21, 20, vec![]));
    assert_eq!(indexer.sync_once(100).await.unwrap(), 1);
    assert!(repo.get(&second.id).await.unwrap().is_none());
    let found = repo.list(None).await.unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0].chain.as_ref().unwrap().confirmation_status,
        "confirmed"
    );
    let independent = Arc::new(SqliteOfferRepository::open(":memory:", board).unwrap());
    BoardIndexer::new(source, Arc::clone(&independent))
        .sync_once(100)
        .await
        .unwrap();
    let a = repo
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.offer)
        .collect::<Vec<_>>();
    let b = independent
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.offer)
        .collect::<Vec<_>>();
    assert_eq!(a, b);
}
#[tokio::test]
async fn policy_active_limit_rate_limit_malformed_unrelated_and_duplicate_transaction() {
    let seller = SellerKeyPair::generate();
    let policy = BoardPolicy {
        max_active_offers_per_seller: 1,
        ..BoardPolicy::default()
    };
    let repo = SqliteOfferRepository::open(":memory:", descriptor(policy)).unwrap();
    let mut b = block(BIRTHDAY, 1, 0, vec![offer(&seller, 1), offer(&seller, 2)]);
    let duplicate = b.outputs[0].clone();
    b.outputs.push(duplicate);
    let mut bad = b.outputs[0].clone();
    bad.txid = TxId("malformed".into());
    bad.transaction_index = 3;
    bad.memo = b"\xf5ZADPbad".to_vec();
    b.outputs.push(bad);
    let mut unrelated = b.outputs[0].clone();
    unrelated.txid = TxId("unrelated".into());
    unrelated.transaction_index = 4;
    unrelated.memo = b"hello".to_vec();
    b.outputs.push(unrelated);
    assert_eq!(repo.apply_block(b).unwrap(), 1);
    assert_eq!(repo.rejection_count().unwrap(), 3);
    let policy = BoardPolicy {
        max_publications_per_seller_per_window: 1,
        ..BoardPolicy::default()
    };
    let rate = SqliteOfferRepository::open(":memory:", descriptor(policy)).unwrap();
    assert_eq!(
        rate.apply_block(block(
            BIRTHDAY,
            1,
            0,
            vec![offer(&seller, 1), offer(&seller, 2)]
        ))
        .unwrap(),
        1
    );
    assert!(rate
        .rejections(10)
        .unwrap()
        .iter()
        .any(|(_, r)| r == "seller_rate_limit"));
}
#[test]
fn wrong_descriptor_and_failed_block_do_not_advance_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.sqlite");
    let board = descriptor(BoardPolicy::default());
    let repo = SqliteOfferRepository::open(&path, board).unwrap();
    assert!(repo.apply_block(block(BIRTHDAY + 1, 1, 0, vec![])).is_err());
    assert!(repo.cursor().unwrap().is_none());
    assert!(SqliteOfferRepository::open(path, descriptor(BoardPolicy::default())).is_err());
}

#[tokio::test]
async fn nonce_replay_expiration_and_invalid_signature_are_rejected() {
    let board = descriptor(BoardPolicy::default());
    let repo = SqliteOfferRepository::open(":memory:", board).unwrap();
    let seller = SellerKeyPair::generate();
    let first = offer(&seller, 1);
    let replay = offer(&seller, 1); // Distinct offer id, same seller nonce.
    let mut expired = offer(&seller, 2);
    expired.issued_at = Some(Utc::now() - Duration::hours(2));
    expired.expires_at = Utc::now() - Duration::hours(1);
    expired.issued_at = DateTime::from_timestamp(expired.issued_at.unwrap().timestamp(), 0);
    expired.expires_at = DateTime::from_timestamp(expired.expires_at.timestamp(), 0).unwrap();
    expired.signature = seller.sign(&signing_bytes(&expired).unwrap());
    let mut bad_signature = offer(&seller, 3);
    bad_signature.title = "changed after signing".into();
    assert_eq!(
        repo.apply_block(block(
            BIRTHDAY,
            1,
            0,
            vec![first, replay, expired, bad_signature]
        ))
        .unwrap(),
        1
    );
    let reasons = repo.rejections(10).unwrap();
    for expected in [
        "duplicate_offer_nonce_or_output",
        "expired_offer",
        "invalid_seller_signature",
    ] {
        assert!(reasons.iter().any(|(_, reason)| reason == expected));
    }
    let after_expiry = block(BIRTHDAY + 1, 2, 1, vec![]);
    let mut after_expiry = after_expiry;
    after_expiry.header.time += Duration::days(2);
    repo.apply_block(after_expiry).unwrap();
    // Stored history remains for replay defense even when inclusion time advances.
    assert_eq!(repo.rejection_count().unwrap(), 3);
}

#[test]
fn valid_publication_can_fail_board_policy_without_becoming_invalid() {
    let board = descriptor(BoardPolicy {
        min_listing_value_zat: 2000,
        ..BoardPolicy::default()
    });
    let repo = SqliteOfferRepository::open(":memory:", board).unwrap();
    let seller = SellerKeyPair::generate();
    let valid = offer(&seller, 1);
    let mut invalid = offer(&seller, 2);
    invalid.title = "Tampered".into();
    assert_eq!(
        repo.apply_block(block(BIRTHDAY, 1, 0, vec![valid.clone(), invalid]))
            .unwrap(),
        0
    );
    let audit = repo.publication_audit(100).unwrap();
    assert!(audit[0].protocol_valid);
    assert!(!audit[0].board_allowed);
    assert_eq!(audit[0].offer.as_ref().unwrap(), &valid);
    assert_eq!(
        audit[0].reason.as_deref(),
        Some("listing_value_below_minimum")
    );
    assert!(!audit[1].protocol_valid);
}

#[test]
fn registry_isolates_authority_network_and_host_acceptance() {
    let a = descriptor(BoardPolicy::default());
    let mut registry = BoardRegistry::new(ZcashNetwork::Mainnet);
    registry
        .register(
            BoardContext {
                repository: Arc::new(SqliteOfferRepository::open(":memory:", a.clone()).unwrap()),
                source_type: BoardSourceType::External,
                status: BoardStatus::Active,
            },
            &HostedBoardPolicy::default(),
            false,
        )
        .unwrap();
    // Same viewing authority under a separately signed board id is not isolated.
    let key = SellerKeyPair::generate();
    let mut shared = a.board.clone();
    shared.board_id = "other-board".into();
    shared.operator_key = key.id.clone();
    let shared = shared.sign(&key).unwrap();
    assert!(registry
        .register(
            BoardContext {
                repository: Arc::new(SqliteOfferRepository::open(":memory:", shared).unwrap()),
                source_type: BoardSourceType::External,
                status: BoardStatus::Active
            },
            &HostedBoardPolicy::default(),
            false
        )
        .is_err());
    assert!(BoardRegistry::new(ZcashNetwork::Testnet)
        .register(
            BoardContext {
                repository: Arc::new(SqliteOfferRepository::open(":memory:", a.clone()).unwrap()),
                source_type: BoardSourceType::External,
                status: BoardStatus::Active
            },
            &HostedBoardPolicy::default(),
            false
        )
        .is_err());
    let fee = HostedBoardPolicy {
        enabled: true,
        creation_fee_zat: 1000,
        fee_recipient: Some(a.board.address.clone()),
    };
    assert!(fee.allows(BoardSourceType::External, false));
    assert!(!fee.allows(BoardSourceType::Hosted, false));
    assert!(fee.allows(BoardSourceType::Hosted, true));
    let mut host = BoardRegistry::new(ZcashNetwork::Mainnet);
    host.register(
        BoardContext {
            repository: Arc::new(SqliteOfferRepository::open(":memory:", a.clone()).unwrap()),
            source_type: BoardSourceType::Hosted,
            status: BoardStatus::Active,
        },
        &fee,
        false,
    )
    .unwrap();
    assert_eq!(
        host.get("ai-services").unwrap().status,
        BoardStatus::Rejected
    );
    assert!(host
        .get("ai-services")
        .unwrap()
        .descriptor()
        .validate(ZcashNetwork::Mainnet)
        .is_ok());
    let summary =
        serde_json::to_string(&host.get("ai-services").unwrap().summary().unwrap()).unwrap();
    assert!(!summary.contains(&a.board.incoming_viewing_key));
    assert!(!summary.contains(&a.board.address));
}

#[tokio::test]
async fn legacy_database_upgrade_preserves_original_board_and_acceptance() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite");
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let o = offer(&seller, 1);
    {
        let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
        repo.apply_block(block(BIRTHDAY, 1, 0, vec![o.clone()]))
            .unwrap();
    }
    // Reproduce the pre-multi-board schema, preserving its original fingerprint/index.
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE publications;").unwrap();
    drop(c);
    let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
    assert_eq!(repo.board(), &board);
    assert_eq!(repo.cursor().unwrap().unwrap().height, BIRTHDAY);
    assert_eq!(repo.list(None).await.unwrap()[0].offer, o);
    let audit = repo.publication_audit(10).unwrap();
    assert_eq!(audit.len(), 1);
    assert!(audit[0].protocol_valid && audit[0].board_allowed);
}

#[test]
fn invalid_external_descriptors_never_register() {
    let board = descriptor(BoardPolicy::default());
    let mut invalid = board.clone();
    invalid.board.name = "Tampered owner announcement".into();
    assert!(SqliteOfferRepository::open(":memory:", invalid).is_err());
    let mut json = serde_json::to_value(&board).unwrap();
    json["board"]["spending_key"] = serde_json::json!("not accepted");
    assert!(serde_json::from_value::<BoardDescriptor>(json).is_err());
    let mut invalid = board;
    invalid.board.incoming_viewing_key = "malformed".into();
    assert!(SqliteOfferRepository::open(":memory:", invalid).is_err());
}

#[tokio::test]
async fn signed_geo_on_v1_board_survives_restart_without_schema_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo.sqlite");
    let board = descriptor(BoardPolicy::default());
    let fingerprint = board.fingerprint().unwrap();
    let encoded_board = serde_json::to_string(&board).unwrap();
    let seller = SellerKeyPair::generate();
    let global = offer(&seller, 1);
    let mut local = offer(&seller, 2);
    local.zadp = OFFER_VERSION_V2;
    local.geo = Some(GeoArea {
        lat_e5: 3777493,
        lon_e5: -12241942,
        radius_m: 50000,
    });
    local.commerce = None;
    local.price = None;
    local.signature = seller.sign(&signing_bytes(&local).unwrap());
    let mut changed = local.clone();
    changed.geo.as_mut().unwrap().radius_m = 60000;
    changed.signature = seller.sign(&signing_bytes(&changed).unwrap());
    {
        let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
        assert_eq!(
            repo.apply_block(block(BIRTHDAY, 1, 0, vec![global.clone(), local.clone()]))
                .unwrap(),
            2
        );
        // Existing first-wins semantics: redefinition is not an UPDATE.
        assert_eq!(
            repo.apply_block(block(BIRTHDAY + 1, 2, 1, vec![changed]))
                .unwrap(),
            0
        );
        assert_eq!(repo.get(&local.id).await.unwrap().unwrap().offer, local);
    }
    let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 2);
    assert_eq!(repo.get(&global.id).await.unwrap().unwrap().offer.geo, None);
    assert_eq!(repo.get(&local.id).await.unwrap().unwrap().offer, local);
    assert_eq!(repo.board().fingerprint().unwrap(), fingerprint);
    assert_eq!(serde_json::to_string(repo.board()).unwrap(), encoded_board);
    assert_eq!(board.board.zadp, BOARD_DESCRIPTOR_VERSION);
    let connection = rusqlite::Connection::open(&path).unwrap();
    let names = connection
        .prepare("PRAGMA table_info(offers)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!names
        .iter()
        .any(|n| n.contains("geo") || n.contains("buyer")));
    assert!(repo
        .canonical_records(None)
        .unwrap()
        .iter()
        .any(|r| r.offer == local));
}

#[tokio::test]
async fn policy_revision_requires_separate_database_and_preserves_expired_history() {
    let dir = tempfile::tempdir().unwrap();
    let old = descriptor(BoardPolicy::default());
    let operator = SellerKeyPair::generate();
    // Use a fixture operator for both signed policies; receiver and birthday are unchanged.
    let mut body = old.board.clone();
    body.operator_key = operator.id.clone();
    let old = body.clone().sign(&operator).unwrap();
    body.policy.max_offer_ttl_seconds = 7_776_000;
    let new = body.sign(&operator).unwrap();
    let seller = SellerKeyPair::generate();
    let mut expired = offer(&seller, 1);
    let now = DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    expired.issued_at = Some(now - Duration::days(2));
    expired.expires_at = now - Duration::days(1);
    expired.signature = seller.sign(&signing_bytes(&expired).unwrap());
    let mut historical = block(BIRTHDAY, 1, 0, vec![expired.clone()]);
    historical.header.time = expired.issued_at.unwrap() + Duration::seconds(1);
    let old_path = dir.path().join("old.sqlite");
    let new_path = dir.path().join("new.sqlite");
    let before = SqliteOfferRepository::open(&old_path, old.clone()).unwrap();
    assert_eq!(before.apply_block(historical.clone()).unwrap(), 1);
    assert!(before.list(None).await.unwrap().is_empty());
    assert!(SqliteOfferRepository::open(&old_path, new.clone()).is_err());
    let rebuilt = SqliteOfferRepository::open(&new_path, new.clone()).unwrap();
    assert_eq!(rebuilt.apply_block(historical).unwrap(), 1);
    assert!(rebuilt.list(None).await.unwrap().is_empty());
    assert_eq!(
        rebuilt.publication_audit(10).unwrap()[0].offer.as_ref(),
        Some(&expired)
    );
    assert_eq!(before.board(), &old);
    assert_eq!(rebuilt.board(), &new);
    assert!(SqliteOfferRepository::open(&new_path, old).is_err());
}

#[tokio::test]
async fn priced_v3_listing_and_legacy_offers_share_existing_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("priced.sqlite");
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let legacy = offer(&seller, 1);
    let now = legacy.issued_at.unwrap();
    let draft = OfferDraft::new_discovery(
        seller.id.clone(),
        "local.service",
        "Bike repair",
        now + Duration::days(1),
        B64.encode([3; 16]),
    )
    .with_endpoint("https://seller.example");
    let service = bind_offer(draft.sign(&seller).unwrap(), "ai-services", now, &seller).unwrap();
    {
        let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
        assert_eq!(
            repo.apply_block(block(BIRTHDAY, 1, 0, vec![legacy.clone(), service.clone()]))
                .unwrap(),
            2
        );
    }
    let repo = SqliteOfferRepository::open(&path, board).unwrap();
    assert_eq!(repo.get(&legacy.id).await.unwrap().unwrap().offer, legacy);
    let saved = repo.get(&service.id).await.unwrap().unwrap().offer;
    assert_eq!(saved, service);
    assert_eq!(saved.commerce.unwrap().endpoint, "https://seller.example");
    assert!(saved.price.is_none());
}

fn revoke_block(height: u32, hash: u32, prev: u32, r: &OfferRevoke) -> BoardBlock {
    let mut b = block(height, hash, prev, vec![]);
    b.outputs.push(BoardOutput {
        txid: TxId(format!("{hash:064x}")),
        transaction_index: 0,
        output_index: 0,
        pool: "orchard".into(),
        value_zat: 1000,
        memo: encode_revoke_memo(r).unwrap().to_vec(),
    });
    b
}
fn revoke(o: &Offer, k: &SellerKeyPair, nonce: u8) -> OfferRevoke {
    OfferRevoke::sign(
        o.board_id.clone().unwrap(),
        o.id.clone(),
        DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap(),
        B64.encode([nonce; 16]),
        k,
    )
    .unwrap()
}
#[tokio::test]
async fn revoke_append_only_reorg_history_and_moderation() {
    let tmp = tempfile::tempdir().unwrap();
    let board = descriptor(BoardPolicy::default());
    let k = SellerKeyPair::generate();
    let o = offer(&k, 1);
    let repo = SqliteOfferRepository::open(tmp.path().join("board.sqlite"), board).unwrap();
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![o.clone()]))
        .unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    let r = revoke(&o, &k, 2);
    repo.apply_block(revoke_block(BIRTHDAY + 1, 2, 1, &r))
        .unwrap();
    assert!(repo.list(None).await.unwrap().is_empty());
    assert!(repo.get(&o.id).await.unwrap().is_none());
    assert_eq!(
        repo.lifecycle(k.id.as_str(), &o.id, Utc::now())
            .unwrap()
            .unwrap()
            .status,
        "revoked"
    );
    assert_eq!(
        repo.historical_offer(k.id.as_str(), &o.id).unwrap(),
        Some(o.clone())
    );
    assert_eq!(
        repo.lifecycle(k.id.as_str(), &o.id, o.expires_at + Duration::days(1))
            .unwrap()
            .unwrap()
            .status,
        "revoked"
    );
    assert_eq!(
        repo.publication_audit(100).unwrap()[1].offer_revoke,
        Some(r.clone())
    );
    let duplicate = revoke(&o, &k, 3);
    repo.apply_block(revoke_block(BIRTHDAY + 2, 3, 2, &duplicate))
        .unwrap();
    assert_eq!(repo.rejection_count().unwrap(), 1);
    repo.rewind(Some(BIRTHDAY)).unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    assert_eq!(
        repo.lifecycle(k.id.as_str(), &o.id, Utc::now())
            .unwrap()
            .unwrap()
            .status,
        "active"
    );
    repo.apply_block(revoke_block(BIRTHDAY + 1, 2, 1, &r))
        .unwrap();
    assert!(repo.list(None).await.unwrap().is_empty());
}
#[tokio::test]
async fn revoke_rejects_wrong_seller_board_unknown_nonce_future_and_before_offer() {
    let tmp = tempfile::tempdir().unwrap();
    let board = descriptor(BoardPolicy::default());
    let k = SellerKeyPair::generate();
    let o = offer(&k, 1);
    let repo = SqliteOfferRepository::open(tmp.path().join("board.sqlite"), board).unwrap();
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![o.clone()]))
        .unwrap();
    let other = SellerKeyPair::generate();
    let wrong_seller = revoke(&o, &other, 2);
    let wrong_board = OfferRevoke::sign(
        "wrong".into(),
        o.id.clone(),
        DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap(),
        B64.encode([3; 16]),
        &k,
    )
    .unwrap();
    let unknown = OfferRevoke::sign(
        "ai-services".into(),
        "unknown".into(),
        o.issued_at.unwrap(),
        B64.encode([4; 16]),
        &k,
    )
    .unwrap();
    let nonce_replay = revoke(&o, &k, 1);
    let future = OfferRevoke::sign(
        "ai-services".into(),
        o.id.clone(),
        o.expires_at + Duration::days(1),
        B64.encode([5; 16]),
        &k,
    )
    .unwrap();
    for (i, r) in [wrong_seller, wrong_board, unknown, nonce_replay, future]
        .iter()
        .enumerate()
    {
        repo.apply_block(revoke_block(
            BIRTHDAY + 1 + i as u32,
            2 + i as u32,
            1 + i as u32,
            r,
        ))
        .unwrap();
    }
    assert_eq!(repo.rejection_count().unwrap(), 5);
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    // Revoke appearing before its target in the very same block must be audited/rejected.
    repo.rewind(None).unwrap();
    let mut b = block(BIRTHDAY, 9, 0, vec![o.clone()]);
    b.outputs[0].transaction_index = 1;
    b.outputs
        .extend(revoke_block(BIRTHDAY, 8, 0, &revoke(&o, &k, 9)).outputs);
    repo.apply_block(b).unwrap();
    assert_eq!(repo.rejection_count().unwrap(), 1);
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
}
#[tokio::test]
async fn revokes_consume_rate_budget_and_release_active_slots() {
    let tmp = tempfile::tempdir().unwrap();
    let policy = BoardPolicy {
        max_active_offers_per_seller: 1,
        max_publications_per_seller_per_window: 2,
        ..BoardPolicy::default()
    };
    let board = descriptor(policy);
    let k = SellerKeyPair::generate();
    let o = offer(&k, 1);
    let repo = SqliteOfferRepository::open(tmp.path().join("board.sqlite"), board).unwrap();
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![o.clone()]))
        .unwrap();
    repo.apply_block(revoke_block(BIRTHDAY + 1, 2, 1, &revoke(&o, &k, 2)))
        .unwrap();
    repo.apply_block(block(BIRTHDAY + 2, 3, 2, vec![offer(&k, 3)]))
        .unwrap();
    assert_eq!(repo.rejections(10).unwrap()[0].1, "seller_rate_limit");
}

#[tokio::test]
async fn revoke_same_block_order_release_slot_and_old_schema_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("old.sqlite");
    let board = descriptor(BoardPolicy {
        max_active_offers_per_seller: 1,
        max_publications_per_seller_per_window: 10,
        ..BoardPolicy::default()
    });
    let key = SellerKeyPair::generate();
    let first = offer(&key, 1);
    let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![first.clone()]))
        .unwrap();
    drop(repo);
    // Simulate an actual pre-revoke schema. Offer bytes are retained across additive upgrade.
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("DROP TABLE offer_revocations", []).unwrap();
    drop(db);
    let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
    assert_eq!(
        repo.historical_offer(key.id.as_str(), &first.id).unwrap(),
        Some(first.clone())
    );
    let r = revoke(&first, &key, 2);
    let mut b = revoke_block(BIRTHDAY + 1, 2, 1, &r);
    let next = offer(&key, 3);
    let mut output = block(BIRTHDAY + 1, 3, 1, vec![next.clone()])
        .outputs
        .remove(0);
    output.transaction_index = 1;
    b.outputs.push(output);
    repo.apply_block(b).unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    assert_eq!(repo.list(None).await.unwrap()[0].offer, next);
    drop(repo);
    let repo = SqliteOfferRepository::open(&path, board).unwrap();
    assert_eq!(
        repo.lifecycle(key.id.as_str(), &first.id, Utc::now())
            .unwrap()
            .unwrap()
            .status,
        "revoked"
    );
    // Same-block Offer -> revoke is accepted, while revoke -> Offer is rejected elsewhere.
    repo.rewind(None).unwrap();
    let mut b = block(BIRTHDAY, 5, 0, vec![first.clone()]);
    let mut output = revoke_block(BIRTHDAY, 6, 0, &r).outputs.remove(0);
    output.transaction_index = 1;
    b.outputs.push(output);
    repo.apply_block(b).unwrap();
    assert!(repo.list(None).await.unwrap().is_empty());
    assert_eq!(repo.rejection_count().unwrap(), 0);
}

#[tokio::test]
async fn expired_offer_can_be_closed_without_rewriting_history_and_low_value_fails() {
    let board = descriptor(BoardPolicy::default());
    let key = SellerKeyPair::generate();
    let original = offer(&key, 1);
    let repo = SqliteOfferRepository::open(":memory:", board).unwrap();
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![original.clone()]))
        .unwrap();
    let mut low = revoke_block(BIRTHDAY + 1, 2, 1, &revoke(&original, &key, 2));
    low.outputs[0].value_zat = 0;
    repo.apply_block(low).unwrap();
    let before = OfferRevoke::sign(
        "ai-services".into(),
        original.id.clone(),
        original.issued_at.unwrap() - Duration::seconds(1),
        B64.encode([3; 16]),
        &key,
    )
    .unwrap();
    repo.apply_block(revoke_block(BIRTHDAY + 2, 3, 2, &before))
        .unwrap();
    assert_eq!(repo.rejection_count().unwrap(), 2);
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    let after = original.expires_at + Duration::seconds(1);
    let r = OfferRevoke::sign(
        "ai-services".into(),
        original.id.clone(),
        after,
        B64.encode([4; 16]),
        &key,
    )
    .unwrap();
    let mut expired = revoke_block(BIRTHDAY + 3, 4, 3, &r);
    expired.header.time = after;
    repo.apply_block(expired).unwrap();
    assert_eq!(
        repo.lifecycle(key.id.as_str(), &original.id, after)
            .unwrap()
            .unwrap()
            .status,
        "revoked"
    );
    assert_eq!(
        repo.historical_offer(key.id.as_str(), &original.id)
            .unwrap(),
        Some(original)
    );
}

#[tokio::test]
async fn same_block_distinct_canonical_offers_are_restart_stable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("same-block.db");
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let originals = vec![offer(&seller, 11), offer(&seller, 12), offer(&seller, 13)];
    {
        let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
        repo.apply_block(block(BIRTHDAY, 1, 0, originals)).unwrap();
        assert_eq!(repo.list(None).await.unwrap().len(), 3);
    }
    let repo = SqliteOfferRepository::open(&path, board).unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 3);
}

#[tokio::test]
async fn standalone_core_has_no_hosted_tables_or_content_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("canonical.sqlite");
    let board = descriptor(BoardPolicy::default());
    let key = SellerKeyPair::generate();
    let mut original = offer(&key, 81);
    original.title = "Email private.person@example.org".into();
    let original = bind_offer(
        original,
        "ai-services",
        DateTime::from_timestamp(Utc::now().timestamp() - 100, 0).unwrap(),
        &key,
    )
    .unwrap();
    let repo = SqliteOfferRepository::open(&path, board).unwrap();
    repo.apply_block(block(BIRTHDAY, 1, 0, vec![original.clone()]))
        .unwrap();
    assert_eq!(repo.list(None).await.unwrap()[0].offer, original);
    repo.with_connection(|c|{
        let count:i64=c.query_row("SELECT count(*) FROM sqlite_master WHERE name IN ('host_moderation','automatic_moderation','offer_reports','moderation_audit')",[],|r|r.get(0))?;
        assert_eq!(count,0,"Core must not install a hosted policy schema");
        // Existing operator tables are opaque to standalone canonical discovery.
        c.execute_batch("CREATE TABLE host_moderation(seller TEXT,id TEXT,status TEXT,reason TEXT);")?;
        c.execute("INSERT INTO host_moderation VALUES (?1,?2,'hidden','operator decision')",rusqlite::params![original.seller.as_str(),original.id])?;
        Ok(())
    }).unwrap();
    assert_eq!(
        repo.get(&original.id).await.unwrap().unwrap().offer,
        original
    );
}

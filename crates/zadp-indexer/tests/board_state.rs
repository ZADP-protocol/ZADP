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

#[tokio::test]
async fn host_visibility_is_separate_persistent_and_reversible() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.sqlite");
    let board = descriptor(BoardPolicy::default());
    let seller = SellerKeyPair::generate();
    let o = offer(&seller, 1);
    {
        let repo = SqliteOfferRepository::open(&path, board.clone()).unwrap();
        repo.apply_block(block(BIRTHDAY, 1, 0, vec![o.clone()]))
            .unwrap();
        for status in [Visibility::Hidden, Visibility::Quarantined] {
            repo.moderate(o.seller.as_str(), &o.id, status, "Demo host decision")
                .unwrap();
            assert!(repo.list(None).await.unwrap().is_empty());
            assert_eq!(repo.canonical_records(None).unwrap().len(), 1);
            let audit = repo.publication_audit(100).unwrap();
            assert!(audit[0].protocol_valid && audit[0].board_allowed);
            assert_eq!(audit[0].offer.as_ref().unwrap(), &o);
        }
    }
    let repo = SqliteOfferRepository::open(path, board).unwrap();
    assert!(repo.list(None).await.unwrap().is_empty());
    repo.moderate(
        o.seller.as_str(),
        &o.id,
        Visibility::Visible,
        "Manual override",
    )
    .unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
    assert!(repo
        .moderate(o.seller.as_str(), "unknown", Visibility::Hidden, "reason")
        .is_err());
    assert!(repo
        .moderate(o.seller.as_str(), &o.id, Visibility::Hidden, "\n")
        .is_err());
    repo.rewind(None).unwrap();
    assert!(repo.publication_audit(100).unwrap().is_empty());
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
    c.execute_batch("DROP TABLE publications; DROP TABLE host_moderation;")
        .unwrap();
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
    repo.moderate(
        local.seller.as_str(),
        &local.id,
        Visibility::Hidden,
        "Fixture",
    )
    .unwrap();
    assert_eq!(repo.list(None).await.unwrap().len(), 1);
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

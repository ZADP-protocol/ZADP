//! Offline real note-encryption fixture. Dummy consensus authorizations cannot be broadcast.
//! This proves decryption -> signed OFFER -> persistent index -> HTTP, NOT mainnet acceptance.
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{DateTime, Duration, Utc};
use nonempty::NonEmpty;
use orchard::{
    bundle::{Authorized, BundleVersion, Flags},
    note::{
        ExtractedNoteCommitment, Note, NoteVersion, Nullifier, RandomSeed, Rho,
        TransmittedNoteCiphertext,
    },
    note_encryption::{CompactAction, OrchardDomain, OrchardNoteEncryption},
    primitives::redpallas,
    value::{NoteValue, ValueCommitTrapdoor, ValueCommitment},
    Action, Anchor, Bundle, Proof,
};
use rand::RngCore;
use rust_decimal::Decimal;
use std::sync::Arc;
use tower::ServiceExt;
use zadp_indexer::{OfferRepository, SqliteOfferRepository};
use zadp_protocol::*;
use zadp_zcash::{decrypt_board_outputs, BlockHeader, BoardBlock};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_note_encryption::{try_compact_note_decryption, Domain};
use zcash_primitives::transaction::{Transaction, TransactionData, TxVersion};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    value::ZatBalance,
};
use zip32::{AccountId, DiversifierIndex};

fn encrypted_action(
    receiver: orchard::Address,
    memo: [u8; 512],
) -> Action<redpallas::Signature<redpallas::SpendAuth>> {
    let mut rng = rand::rngs::OsRng;
    let mut raw = [0u8; 32];
    loop {
        rng.fill_bytes(&mut raw);
        raw[31] &= 0x3f;
        if Option::<Nullifier>::from(Nullifier::from_bytes(&raw)).is_some() {
            break;
        }
    }
    let nf = Option::<Nullifier>::from(Nullifier::from_bytes(&raw)).unwrap();
    let rho = Option::<Rho>::from(Rho::from_bytes(&raw)).unwrap();
    let note = loop {
        rng.fill_bytes(&mut raw);
        if let Some(seed) = Option::<RandomSeed>::from(RandomSeed::from_bytes(raw, &rho)) {
            if let Some(n) = Option::<Note>::from(Note::from_parts(
                receiver,
                NoteValue::from_raw(1000),
                rho,
                seed,
                NoteVersion::V2,
            )) {
                break n;
            }
        }
    };
    let trap =
        Option::<ValueCommitTrapdoor>::from(ValueCommitTrapdoor::from_bytes([0u8; 32])).unwrap();
    let cv = ValueCommitment::derive(NoteValue::from_raw(1000) - NoteValue::from_raw(0), trap);
    let cmx = ExtractedNoteCommitment::from(note.commitment());
    let encryption = OrchardNoteEncryption::new(None, note, memo);
    let ciphertext = TransmittedNoteCiphertext {
        epk_bytes: OrchardDomain::epk_bytes(encryption.epk()).0,
        enc_ciphertext: encryption.encrypt_note_plaintext(),
        out_ciphertext: encryption.encrypt_outgoing_plaintext(&cv, &cmx, &mut rng),
    };
    let sk = loop {
        rng.fill_bytes(&mut raw);
        raw[31] &= 0x3f;
        if let Ok(key) = redpallas::SigningKey::<redpallas::SpendAuth>::try_from(raw) {
            break key;
        }
    };
    let rk = redpallas::VerificationKey::from(&sk);
    Action::from_parts(
        nf,
        rk,
        cmx,
        ciphertext,
        cv,
        redpallas::Signature::from([0u8; 64]),
    )
    .unwrap()
}
#[tokio::test]
async fn encrypted_memo_to_independent_persistent_indexes_and_get_offers() {
    encrypted_discovery_fixture(false).await;
}
#[tokio::test]
async fn encrypted_geo_memo_to_independent_indexes_and_public_feed() {
    encrypted_discovery_fixture(true).await;
}
async fn encrypted_discovery_fixture(with_geo: bool) {
    let params = ZcashNetwork::Mainnet.parameters();
    let mut seed = [0u8; 64];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let uivk = UnifiedSpendingKey::from_seed(&params, &seed, AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let ivk = uivk.orchard().as_ref().unwrap();
    let board_address = uivk
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap();
    let unrelated_address = uivk
        .address(DiversifierIndex::from(1u32), UnifiedAddressRequest::ORCHARD)
        .unwrap();
    let operator = SellerKeyPair::generate();
    let seller = SellerKeyPair::generate();
    let board = BoardBody {
        zadp: 1,
        board_id: "ai-services".into(),
        name: "Encrypted fixture".into(),
        network: ZcashNetwork::Mainnet,
        address: board_address.encode(&params),
        incoming_viewing_key: uivk.encode(&params),
        birthday_height: 3_000_000,
        policy: BoardPolicy::default(),
        operator_key: operator.id.clone(),
    }
    .sign(&operator)
    .unwrap();
    let now = DateTime::from_timestamp(Utc::now().timestamp() - 1, 0).unwrap();
    let draft = OfferDraft::new_ulid(
        seller.id.clone(),
        "llm.inference",
        "ZADP Demo Inference",
        Price::zec(Decimal::new(1, 2)),
        "https://example.com/zadp",
        now + Duration::hours(24),
        B64.encode([1u8; 16]),
    );
    let geo = GeoArea {
        lat_e5: 3777493,
        lon_e5: -12241942,
        radius_m: 50000,
    };
    let draft = if with_geo { draft.with_geo(geo) } else { draft };
    let offer = bind_offer(draft.sign(&seller).unwrap(), "ai-services", now, &seller).unwrap();
    let memo = encode_offer_memo(&offer).unwrap();
    let action = encrypted_action(*board_address.orchard().unwrap(), memo);
    let compact = CompactAction::from(&action);
    let decrypted = try_compact_note_decryption(
        &OrchardDomain::for_compact_action(&compact),
        &ivk.prepare(),
        &compact,
    )
    .unwrap();
    assert_eq!(decrypted.1, *board_address.orchard().unwrap());
    let unrelated = encrypted_action(*unrelated_address.orchard().unwrap(), memo);
    let actions = NonEmpty::from_vec(vec![action, unrelated]).unwrap();
    let authorization = Authorized::from_parts(
        Proof::new(vec![0; Proof::expected_proof_size(2)]),
        redpallas::Signature::from([0u8; 64]),
    );
    let bundle = Bundle::try_from_parts(
        actions,
        Flags::ENABLED,
        ZatBalance::zero(),
        Anchor::empty_tree(),
        authorization,
        BundleVersion::orchard_insecure_v1(),
    )
    .unwrap();
    let tx = TransactionData::from_parts(
        TxVersion::V5,
        BranchId::Nu5,
        0,
        BlockHeight::from_u32(1_700_000),
        None,
        None,
        None,
        Some(bundle),
    )
    .freeze()
    .unwrap();
    let mut bytes = vec![];
    tx.write(&mut bytes).unwrap();
    let full = Transaction::read(bytes.as_slice(), BranchId::Nu5).unwrap();
    let outputs = decrypt_board_outputs(&full, ivk, board_address.orchard().unwrap(), 1);
    assert_eq!(
        outputs.len(),
        1,
        "other diversified addresses sharing the IVK must be ignored"
    );
    assert_eq!(outputs[0].memo, memo);
    let block = BoardBlock {
        header: BlockHeader {
            height: 3_000_000,
            hash: "1".repeat(64),
            prev_hash: "0".repeat(64),
            time: now,
        },
        outputs,
    };
    let repo = Arc::new(SqliteOfferRepository::open(":memory:", board.clone()).unwrap());
    assert_eq!(repo.apply_block(block.clone()).unwrap(), 1);
    let other = SqliteOfferRepository::open(
        ":memory:",
        serde_json::from_str(&serde_json::to_string(&board).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(other.apply_block(block).unwrap(), 1);
    assert_eq!(
        repo.list(None).await.unwrap()[0].offer,
        other.list(None).await.unwrap()[0].offer
    );
    let response = zadp_api::board_router(repo)
        .oneshot(
            Request::builder()
                .uri("/offers?capability=llm.inference")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["offers"][0]["title"], "ZADP Demo Inference");
    assert_eq!(
        json["offers"][0]["chain"]["transaction_id"],
        full.txid().to_string()
    );
    assert_eq!(json["offers"][0]["chain"]["network"], "mainnet");
    assert_eq!(json["offers"][0]["chain"]["confirmations"], 1);
    let returned: Offer = serde_json::from_value(json["offers"][0].clone()).unwrap();
    verify_offer(&returned, Utc::now()).unwrap();
    assert_eq!(returned.geo, with_geo.then_some(geo));
    assert_eq!(returned.zadp, if with_geo { 2 } else { 1 });
}

fn isolated_descriptor(id: &str, birthday: u32) -> BoardDescriptor {
    let params = ZcashNetwork::Mainnet.parameters();
    let mut seed = [0u8; 64];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let uivk = UnifiedSpendingKey::from_seed(&params, &seed, AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .to_unified_incoming_viewing_key();
    let address = uivk
        .address(DiversifierIndex::from(0u32), UnifiedAddressRequest::ORCHARD)
        .unwrap();
    let owner = SellerKeyPair::generate();
    BoardBody {
        zadp: 1,
        board_id: id.into(),
        name: format!("Fixture {id}"),
        network: ZcashNetwork::Mainnet,
        address: address.encode(&params),
        incoming_viewing_key: uivk.encode(&params),
        birthday_height: birthday,
        policy: BoardPolicy::default(),
        operator_key: owner.id.clone(),
    }
    .sign(&owner)
    .unwrap()
}
fn same_id_offer(board: &BoardDescriptor, seller: &SellerKeyPair, now: DateTime<Utc>) -> Offer {
    let mut offer = OfferDraft::new_ulid(
        seller.id.clone(),
        "text.shorten",
        "Text service",
        Price::zec(Decimal::new(1, 2)),
        "https://service.example/zadp",
        now + Duration::hours(24),
        B64.encode([5u8; 16]),
    )
    .sign(seller)
    .unwrap();
    offer.id = "same-offer-id".into();
    bind_offer(offer, &board.board.board_id, now, seller).unwrap()
}
#[tokio::test]
async fn two_encrypted_boards_same_id_isolation_public_api_and_host_policy() {
    use zadp_indexer::{
        BoardContext, BoardRegistry, BoardSourceType, BoardStatus, HostedBoardPolicy, Visibility,
    };
    use zcash_keys::{address::Address, keys::UnifiedIncomingViewingKey};
    let now = DateTime::from_timestamp(Utc::now().timestamp() - 1, 0).unwrap();
    let seller = SellerKeyPair::generate();
    let a = isolated_descriptor("board-a", 3_000_000);
    let b = isolated_descriptor("board-b", 3_000_001);
    assert_ne!(a.board.incoming_viewing_key, b.board.incoming_viewing_key);
    let oa = same_id_offer(&a, &seller, now);
    let ob = same_id_offer(&b, &seller, now);
    let receiver = |board: &BoardDescriptor| {
        let Address::Unified(address) =
            Address::decode(&board.board.network.parameters(), &board.board.address).unwrap()
        else {
            panic!()
        };
        *address.orchard().unwrap()
    };
    let actions = NonEmpty::from_vec(vec![
        encrypted_action(receiver(&a), encode_offer_memo(&oa).unwrap()),
        encrypted_action(receiver(&b), encode_offer_memo(&ob).unwrap()),
    ])
    .unwrap();
    let bundle = Bundle::try_from_parts(
        actions,
        Flags::ENABLED,
        ZatBalance::zero(),
        Anchor::empty_tree(),
        Authorized::from_parts(
            Proof::new(vec![0; Proof::expected_proof_size(2)]),
            redpallas::Signature::from([0u8; 64]),
        ),
        BundleVersion::orchard_insecure_v1(),
    )
    .unwrap();
    let tx = TransactionData::from_parts(
        TxVersion::V5,
        BranchId::Nu5,
        0,
        BlockHeight::from_u32(1_700_000),
        None,
        None,
        None,
        Some(bundle),
    )
    .freeze()
    .unwrap();
    let ra = Arc::new(SqliteOfferRepository::open(":memory:", a.clone()).unwrap());
    let rb = Arc::new(SqliteOfferRepository::open(":memory:", b.clone()).unwrap());
    let independently_rebuilt = SqliteOfferRepository::open(":memory:", b.clone()).unwrap();
    for (board, repo) in [
        (&a, ra.as_ref()),
        (&b, rb.as_ref()),
        (&b, &independently_rebuilt),
    ] {
        let key = UnifiedIncomingViewingKey::decode(
            &board.board.network.parameters(),
            &board.board.incoming_viewing_key,
        )
        .unwrap();
        let outputs =
            decrypt_board_outputs(&tx, key.orchard().as_ref().unwrap(), &receiver(board), 0);
        assert_eq!(
            outputs.len(),
            1,
            "a board UIVK must not decrypt another account's offer"
        );
        let block = BoardBlock {
            header: BlockHeader {
                height: board.board.birthday_height,
                hash: "1".repeat(64),
                prev_hash: "0".repeat(64),
                time: now,
            },
            outputs,
        };
        assert_eq!(repo.apply_block(block).unwrap(), 1);
    }
    assert_eq!(ra.cursor().unwrap().unwrap().height, 3_000_000);
    assert_eq!(rb.cursor().unwrap().unwrap().height, 3_000_001);
    assert_eq!(
        rb.list(None).await.unwrap()[0].offer,
        independently_rebuilt.list(None).await.unwrap()[0].offer
    );
    let mut registry = BoardRegistry::new(ZcashNetwork::Mainnet);
    for repository in [Arc::clone(&ra), Arc::clone(&rb)] {
        registry
            .register(
                BoardContext {
                    repository,
                    source_type: BoardSourceType::External,
                    status: BoardStatus::Active,
                },
                &HostedBoardPolicy::default(),
                false,
            )
            .unwrap();
    }
    let app = zadp_api::multi_board_router(Arc::new(registry));
    async fn get(app: axum::Router, path: &str) -> (u16, serde_json::Value) {
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status().as_u16();
        let body = to_bytes(response.into_body(), 1_048_576).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }
    let (_, all) = get(app.clone(), "/offers").await;
    assert_eq!(all["offers"].as_array().unwrap().len(), 2);
    assert_eq!(all["offers"][0]["id"], all["offers"][1]["id"]);
    let (_, only_b) = get(app.clone(), "/offers?board_id=board-b&q=Text").await;
    assert_eq!(only_b["offers"].as_array().unwrap().len(), 1);
    assert_eq!(only_b["offers"][0]["board_id"], "board-b");
    let (_, metadata) = get(app.clone(), "/boards").await;
    let metadata = metadata.to_string();
    for board in [&a, &b] {
        assert!(!metadata.contains(&board.board.incoming_viewing_key));
        assert!(!metadata.contains(&board.board.address));
    }
    assert_eq!(get(app.clone(), "/boards/missing").await.0, 404);
    assert_eq!(
        get(app.clone(), "/boards/board-a/offers?board_id=board-b")
            .await
            .0,
        400
    );
    // Registry rejection/pausing is host policy, not descriptor or offer invalidity.
    for status in [BoardStatus::Paused, BoardStatus::Rejected] {
        let mut excluded = BoardRegistry::new(ZcashNetwork::Mainnet);
        excluded
            .register(
                BoardContext {
                    repository: Arc::clone(&ra),
                    source_type: BoardSourceType::External,
                    status,
                },
                &HostedBoardPolicy::default(),
                false,
            )
            .unwrap();
        let (_, response) = get(zadp_api::multi_board_router(Arc::new(excluded)), "/offers").await;
        assert!(response["offers"].as_array().unwrap().is_empty());
        assert!(ra.publication_audit(10).unwrap()[0].protocol_valid);
    }
    ra.moderate(
        seller.id.as_str(),
        &oa.id,
        Visibility::Hidden,
        "Host-only demo moderation",
    )
    .unwrap();
    assert_eq!(
        get(app.clone(), "/boards/board-a/offers").await.1["offers"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        get(app.clone(), "/boards/board-b/offers").await.1["offers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(ra.publication_audit(10).unwrap()[0].protocol_valid);
    assert_eq!(ra.canonical_records(None).unwrap().len(), 1);
    for path in [
        "/boards",
        "/boards/board-a/moderate",
        "/boards/board-a/descriptor",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(response.status().as_u16(), 404 | 405));
    }
}

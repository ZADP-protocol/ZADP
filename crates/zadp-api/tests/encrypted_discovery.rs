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
    let offer = bind_offer(
        OfferDraft::new_ulid(
            seller.id.clone(),
            "llm.inference",
            "ZADP Demo Inference",
            Price::zec(Decimal::new(1, 2)),
            "https://example.com/zadp",
            now + Duration::hours(24),
            B64.encode([1u8; 16]),
        )
        .sign(&seller)
        .unwrap(),
        "ai-services",
        now,
        &seller,
    )
    .unwrap();
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
}

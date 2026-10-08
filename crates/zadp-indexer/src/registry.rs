//! Host registry: read-only external registration, independent from protocol rules.
use crate::{IndexerError, SqliteOfferRepository};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use zadp_protocol::{BoardDescriptor, ZcashNetwork};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardStatus {
    Active,
    Paused,
    Rejected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardSourceType {
    External,
    Hosted,
}

/// Optional host onboarding policy; an operator approval is not payment evidence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedBoardPolicy {
    pub enabled: bool,
    pub creation_fee_zat: u64,
    #[serde(default)]
    pub fee_recipient: Option<String>,
}
impl HostedBoardPolicy {
    pub fn allows(&self, source: BoardSourceType, operator_approved: bool) -> bool {
        source == BoardSourceType::External
            || !self.enabled
            || self.creation_fee_zat == 0
            || operator_approved
    }
}

pub struct BoardContext {
    pub repository: Arc<SqliteOfferRepository>,
    pub source_type: BoardSourceType,
    pub status: BoardStatus,
}
impl BoardContext {
    pub fn descriptor(&self) -> &BoardDescriptor {
        self.repository.board()
    }
}

/// Public metadata deliberately excludes receive address and viewing authority.
#[derive(Debug, Serialize)]
pub struct BoardSummary {
    pub board_id: String,
    pub name: String,
    pub network: ZcashNetwork,
    pub owner_identity: String,
    pub descriptor_fingerprint: String,
    pub birthday_height: u32,
    pub max_offer_ttl_seconds: u32,
    pub scanned_height: Option<u32>,
    pub source_type: BoardSourceType,
    pub status: BoardStatus,
}
impl BoardContext {
    pub fn summary(&self) -> Result<BoardSummary, IndexerError> {
        let b = &self.descriptor().board;
        Ok(BoardSummary {
            board_id: b.board_id.clone(),
            name: b.name.clone(),
            network: b.network,
            owner_identity: b.operator_key.as_str().into(),
            descriptor_fingerprint: self.descriptor().fingerprint()?,
            birthday_height: b.birthday_height,
            max_offer_ttl_seconds: b.policy.max_offer_ttl_seconds,
            scanned_height: self.repository.cursor()?.map(|h| h.height),
            source_type: self.source_type,
            status: self.status,
        })
    }
}

pub struct BoardRegistry {
    network: ZcashNetwork,
    boards: BTreeMap<String, BoardContext>,
}
impl BoardRegistry {
    pub fn new(network: ZcashNetwork) -> Self {
        Self {
            network,
            boards: BTreeMap::new(),
        }
    }
    pub fn register(
        &mut self,
        mut context: BoardContext,
        policy: &HostedBoardPolicy,
        operator_approved: bool,
    ) -> Result<(), IndexerError> {
        context.descriptor().validate(self.network)?;
        if policy.enabled && policy.creation_fee_zat > 0 {
            let recipient = policy
                .fee_recipient
                .as_deref()
                .ok_or_else(|| IndexerError::State("host fee recipient required".into()))?;
            let address =
                zcash_keys::address::Address::decode(&self.network.parameters(), recipient)
                    .ok_or_else(|| {
                        IndexerError::State("invalid host fee receiver or network".into())
                    })?;
            if !matches!(address, zcash_keys::address::Address::Unified(_)) {
                return Err(IndexerError::State(
                    "host fee requires Unified Address".into(),
                ));
            }
        }

        let id = context.descriptor().board.board_id.clone();
        if self.boards.contains_key(&id) {
            return Err(IndexerError::State("duplicate board id".into()));
        }
        let scope = context.descriptor().incoming_scope_fingerprint()?;
        for board in self.boards.values() {
            if board.descriptor().incoming_scope_fingerprint()? == scope {
                return Err(IndexerError::State(
                    "independent boards must not share incoming decryption authority".into(),
                ));
            }
        }
        if policy.creation_fee_zat > 2_100_000_000_000_000 {
            return Err(IndexerError::State("invalid host fee".into()));
        }
        if !policy.allows(context.source_type, operator_approved) {
            context.status = BoardStatus::Rejected;
        }
        self.boards.insert(id, context);
        Ok(())
    }
    pub fn get(&self, id: &str) -> Option<&BoardContext> {
        self.boards.get(id)
    }
    pub fn contexts(&self) -> impl Iterator<Item = &BoardContext> {
        self.boards.values()
    }
}

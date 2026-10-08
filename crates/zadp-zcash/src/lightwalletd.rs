//! Actual CompactTxStreamer transport and local trial decryption. No spending keys.
use crate::{BlockHeader, BoardBlock, BoardOutput, BoardSource, TxId, ZcashError};
use async_trait::async_trait;
use chrono::DateTime;
use orchard::{
    keys::{IncomingViewingKey, PreparedIncomingViewingKey},
    note_encryption::{CompactAction, IronwoodDomain, OrchardDomain},
};
use std::time::Duration;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use zadp_protocol::{BoardDescriptor, ZcashNetwork};
use zcash_client_backend::proto::{
    compact_formats::CompactBlock,
    service::{
        compact_tx_streamer_client::CompactTxStreamerClient, BlockId, ChainSpec, Empty,
        RawTransaction, TxFilter,
    },
};
use zcash_keys::{address::Address, keys::UnifiedIncomingViewingKey};
use zcash_note_encryption::try_compact_note_decryption;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId};

fn error(reason: impl Into<String>) -> ZcashError {
    ZcashError::Scan(reason.into())
}
/// One network-validated read-only connection shared across isolated board scanners.
#[derive(Clone)]
pub struct BoardTransport {
    client: CompactTxStreamerClient<Channel>,
    network: ZcashNetwork,
}
impl BoardTransport {
    /// Public software metadata reported by the transport, never wallet data.
    pub async fn backend_versions(&self) -> Result<(String, String), ZcashError> {
        let info = self
            .client
            .clone()
            .get_lightd_info(Empty {})
            .await
            .map_err(|e| error(format!("GetLightdInfo: {}", e.code())))?
            .into_inner();
        let clean = |s: String| {
            if s.len() <= 128 && !s.chars().any(char::is_control) {
                s
            } else {
                String::new()
            }
        };
        Ok((clean(info.zcashd_subversion), clean(info.version)))
    }
    pub async fn connect(url: &str, network: ZcashNetwork) -> Result<Self, ZcashError> {
        Ok(Self {
            client: connect_lightwalletd(url, network).await?,
            network,
        })
    }
}

pub struct RealZcashScanner {
    client: CompactTxStreamerClient<Channel>,
    network: ZcashNetwork,
    ivk: IncomingViewingKey,
    prepared: PreparedIncomingViewingKey,
    receiver: orchard::Address,
}

/// Set an explicit process TLS provider before building tonic or wallet clients.
pub fn initialize_tls() {
    // The official backend can enable both rustls crypto providers transitively.
    // Select one explicitly to avoid a process panic on the first TLS connection.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

pub async fn connect_lightwalletd(
    url: &str,
    network: ZcashNetwork,
) -> Result<CompactTxStreamerClient<Channel>, ZcashError> {
    initialize_tls();
    let uri = url::Url::parse(url).map_err(|_| error("invalid lightwalletd URL"))?;
    if !uri.username().is_empty()
        || uri.password().is_some()
        || uri.query().is_some()
        || uri.fragment().is_some()
    {
        return Err(error(
            "credentials, query parameters and fragments are forbidden in lightwalletd URL",
        ));
    }
    let local = matches!(uri.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if uri.scheme() != "https" && !(uri.scheme() == "http" && local) {
        return Err(error(
            "lightwalletd requires TLS, except loopback development",
        ));
    }
    let mut endpoint = Endpoint::from_shared(url.to_string())
        .map_err(|_| error("invalid lightwalletd endpoint"))?
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60));
    if uri.scheme() == "https" {
        endpoint = endpoint
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .map_err(|_| error("TLS configuration failed"))?;
    }
    let channel = endpoint
        .connect()
        .await
        .map_err(|_| error("lightwalletd connection failed"))?;
    let mut client =
        CompactTxStreamerClient::new(channel).max_decoding_message_size(16 * 1024 * 1024);
    let info = client
        .get_lightd_info(Empty {})
        .await
        .map_err(|e| error(format!("GetLightdInfo: {}", e.code())))?
        .into_inner();
    if info.chain_name != network.chain_name() {
        return Err(error("lightwalletd network mismatch"));
    }
    let height = u32::try_from(info.block_height).map_err(|_| error("unsupported block height"))?;
    let expected = format!(
        "{:08x}",
        u32::from(BranchId::for_height(
            &network.parameters(),
            BlockHeight::from_u32(height)
        ))
    );
    if info.consensus_branch_id.to_ascii_lowercase() != expected {
        return Err(error("service consensus branch does not match supported Zcash parameters; update libraries before scanning"));
    }
    Ok(client)
}

fn header(block: &CompactBlock) -> Result<BlockHeader, ZcashError> {
    if block.hash.len() != 32 || block.prev_hash.len() != 32 || block.time == 0 {
        return Err(error("malformed compact block metadata"));
    }
    Ok(BlockHeader {
        height: u32::try_from(block.height).map_err(|_| error("height overflow"))?,
        hash: hex::encode(&block.hash),
        prev_hash: hex::encode(&block.prev_hash),
        time: DateTime::from_timestamp(i64::from(block.time), 0)
            .ok_or_else(|| error("invalid block time"))?,
    })
}

impl RealZcashScanner {
    pub async fn connect(board: &BoardDescriptor, url: &str) -> Result<Self, ZcashError> {
        let transport = BoardTransport::connect(url, board.board.network).await?;
        Self::with_transport(board, &transport)
    }
    /// Reuse one already network-validated transport for multiple isolated boards.
    pub fn with_transport(
        board: &BoardDescriptor,
        transport: &BoardTransport,
    ) -> Result<Self, ZcashError> {
        board
            .validate(transport.network)
            .map_err(|_| error("invalid signed board descriptor"))?;
        let network = board.board.network;
        let params = network.parameters();
        let ivk = UnifiedIncomingViewingKey::decode(&params, &board.board.incoming_viewing_key)
            .map_err(|_| error("invalid board UIVK"))?
            .orchard()
            .clone()
            .ok_or_else(|| error("missing Orchard viewing authority"))?;
        let Address::Unified(ua) = Address::decode(&params, &board.board.address)
            .ok_or_else(|| error("invalid board address"))?
        else {
            return Err(error("board requires UA"));
        };
        let receiver = *ua
            .orchard()
            .ok_or_else(|| error("missing Orchard receiver"))?;
        let prepared = ivk.prepare();
        Ok(Self {
            client: transport.client.clone(),
            network,
            ivk,
            prepared,
            receiver,
        })
    }
    async fn compact_block(&self, height: u32) -> Result<CompactBlock, ZcashError> {
        let block = self
            .client
            .clone()
            .get_block(BlockId {
                height: u64::from(height),
                hash: vec![],
            })
            .await
            .map_err(|e| error(format!("GetBlock: {}", e.code())))?
            .into_inner();
        if block.height != u64::from(height) {
            return Err(error("service returned wrong block height"));
        }
        Ok(block)
    }
    /// Extract authenticated note plaintexts only for the descriptor's exact receiver.
    pub fn decrypt_transaction(
        &self,
        tx: &Transaction,
        transaction_index: u64,
    ) -> Vec<BoardOutput> {
        decrypt_board_outputs(tx, &self.ivk, &self.receiver, transaction_index)
    }
}

pub fn decrypt_board_outputs(
    tx: &Transaction,
    ivk: &IncomingViewingKey,
    receiver: &orchard::Address,
    transaction_index: u64,
) -> Vec<BoardOutput> {
    let mut outputs = vec![];
    for (pool, bundle) in [
        ("orchard", tx.orchard_bundle()),
        ("ironwood", tx.ironwood_bundle()),
    ] {
        if let Some(bundle) = bundle {
            for (index, _, note, address, memo) in
                bundle.decrypt_outputs_with_keys(std::slice::from_ref(ivk))
            {
                if &address == receiver {
                    outputs.push(BoardOutput {
                        txid: TxId(tx.txid().to_string()),
                        transaction_index,
                        output_index: index as u32,
                        pool: pool.into(),
                        value_zat: note.value().inner(),
                        memo: memo.to_vec(),
                    });
                }
            }
        }
    }
    outputs
}

#[async_trait]
impl BoardSource for RealZcashScanner {
    async fn tip_height(&self) -> Result<u32, ZcashError> {
        let tip = self
            .client
            .clone()
            .get_latest_block(ChainSpec {})
            .await
            .map_err(|e| error(format!("GetLatestBlock: {}", e.code())))?
            .into_inner();
        u32::try_from(tip.height).map_err(|_| error("tip height overflow"))
    }
    async fn header(&self, height: u32) -> Result<BlockHeader, ZcashError> {
        header(&self.compact_block(height).await?)
    }
    async fn block(&self, height: u32) -> Result<BoardBlock, ZcashError> {
        let compact = self.compact_block(height).await?;
        let block_header = header(&compact)?;
        let mut outputs = vec![];
        for tx in &compact.vtx {
            if tx.txid.len() != 32 {
                return Err(error("invalid compact transaction ID"));
            }
            let mut matches = false;
            for (ironwood, actions) in [(false, &tx.actions), (true, &tx.ironwood_actions)] {
                for action in actions {
                    let action = CompactAction::try_from(action)
                        .map_err(|_| error("malformed compact Orchard-family action"))?;
                    let decrypted = if ironwood {
                        try_compact_note_decryption(
                            &IronwoodDomain::for_compact_action(&action),
                            &self.prepared,
                            &action,
                        )
                    } else {
                        try_compact_note_decryption(
                            &OrchardDomain::for_compact_action(&action),
                            &self.prepared,
                            &action,
                        )
                    };
                    if decrypted.is_some_and(|(_, address)| address == self.receiver) {
                        matches = true;
                    }
                }
            }
            if !matches {
                continue;
            }
            let raw = self
                .client
                .clone()
                .get_transaction(TxFilter {
                    hash: tx.txid.clone(),
                    ..Default::default()
                })
                .await
                .map_err(|e| error(format!("GetTransaction: {}", e.code())))?
                .into_inner();
            if raw.height != u64::from(height) {
                return Err(error(
                    "transaction height changed during scan; retry after reorg",
                ));
            }
            let branch =
                BranchId::for_height(&self.network.parameters(), BlockHeight::from_u32(height));
            let mut reader = std::io::Cursor::new(&raw.data);
            let full = Transaction::read(&mut reader, branch)
                .map_err(|_| error("invalid full transaction or unsupported consensus version"))?;
            if reader.position() != raw.data.len() as u64
                || full.txid().as_ref().as_slice() != tx.txid.as_slice()
                || full.consensus_branch_id() != branch
            {
                return Err(error("full transaction does not match compact transaction"));
            }
            outputs.extend(self.decrypt_transaction(&full, tx.index));
        }
        // Recheck after fetching full transactions to avoid committing a mixed-fork block.
        if self.header(height).await?.hash != block_header.hash {
            return Err(error("block changed during scan; retry"));
        }
        Ok(BoardBlock {
            header: block_header,
            outputs,
        })
    }
}

/// Explicit seller-only broadcast entry point; never called by the scanner.
pub async fn broadcast_transaction(
    client: &mut CompactTxStreamerClient<Channel>,
    bytes: Vec<u8>,
) -> Result<(), ZcashError> {
    let response = client
        .send_transaction(RawTransaction {
            data: bytes,
            height: 0,
        })
        .await
        .map_err(|e| {
            ZcashError::Publish(format!(
                "SendTransaction: {}; broadcast status may be unknown",
                e.code()
            ))
        })?
        .into_inner();
    if response.error_code != 0 {
        return Err(ZcashError::Publish(format!(
            "network rejected transaction (code {})",
            response.error_code
        )));
    }
    Ok(())
}

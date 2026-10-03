# ZADP — Zcash Agent Discovery Protocol

ZADP discovers signed service offers published to a dedicated Zcash shielded board address. Independent indexers use a portable signed descriptor and incoming viewing authority; they cannot spend board funds.

## Current status

Implemented with real Zcash APIs and cryptography:

- Mainnet/testnet CompactTxStreamer scanning, Orchard/Ironwood trial decryption and full memo retrieval.
- Shielded OFFER publication through a local Zallet wallet and explicit seller CLI.
- Signed board descriptors, canonical Ed25519 OFFER verification and board admission policy.
- Persistent cursor, restart/reorg recovery, replay protection and SQLite indexing.
- `GET /offers` with transaction inclusion and confirmation metadata.

**Live mainnet acceptance is pending.** No funded broadcast or actual-network offer discovery has been demonstrated yet. Passing offline encryption tests does not satisfy that milestone.

Mocked / not yet implemented for production:

- Purchase settlement and seller fulfillment remain in the existing mock demo.
- AI buyer, reputation, hosted marketplace and MCP/web UI are not implemented.
- OFFER_UPDATE and OFFER_REVOKE are reserved for future protocol versions.

No AI/LLM process receives a Zcash spending key. The scanner only loads public incoming viewing information. Publication keys stay in the local wallet; a seller identity is a separate Ed25519 key.

## Mainnet discovery path

Provision a dedicated board and sign `board.json` as described in [Board descriptor](docs/board-descriptor.md). The board's incoming viewing information deliberately exposes board publications and received values to descriptor holders.

```sh
cargo run -p zadp-api -- --network mainnet --board board.json \
  --lightwalletd https://YOUR-SERVICE:PORT --database board.sqlite
curl 'http://127.0.0.1:8080/offers?capability=llm.inference'
```

For explicit funded publication, wallet preparation, preview/confirmation and independent reconstruction, follow [Mainnet publishing](docs/mainnet-publishing.md). `--dry-run` builds an unsigned proposal and displays the wallet-computed fee. Automated tests never publish real transactions.

## Mock development path

```sh
cargo run -p zadp-demo
```

The original deterministic offer → invoice → policy → mock payment → fulfillment demonstration is retained. It does not spend ZEC and is separate from the real discovery daemon.

## Development

Use current Rust stable (the locked Zcash dependencies require a newer toolchain than the original bootstrap).

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Tests cover descriptors, signatures, policy, memo rejection, replay, duplicate outputs, persistence, reorgs, independent reconstruction, wallet proposal safety, and an offline Orchard-encrypted memo → HTTP discovery fixture. The fixture has dummy consensus authorization and cannot be broadcast.

## Documentation

- [Board descriptor](docs/board-descriptor.md)
- [Zcash scanning](docs/zcash-scanning.md)
- [Mainnet publishing and pending acceptance](docs/mainnet-publishing.md)
- [Architecture](docs/architecture.md)
- [Protocol](docs/protocol.md)
- [Threat model](docs/threat-model.md)
- [Roadmap](docs/roadmap.md)

License selection remains to be finalized.

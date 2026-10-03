# ZADP Roadmap

## Phase 1 — Local mock vertical slice (current)

- Versioned ZADP/1 offer messages (JSON + canonical CBOR signing).
- Seller identity (ed25519) sign/verify.
- `MockZcashAdapter` for publish/scan/pay/observe.
- Indexer validation + in-memory store.
- REST `GET /offers`.
- Invoice flow + payment policy engine.
- CLI demo: offer → index → invoice → policy → pay → fulfill.
- Docs: architecture, protocol, threat model, this roadmap.
- CI: fmt, clippy, tests.

## Phase 2 — Real Zcash testnet integration

- Replace mock adapter with testnet-backed implementation behind the same `ZcashAdapter` trait.
- Shielded publish of offer memos to a board address.
- Read-only scanning with viewing authority only for indexer.
- Shielded payments with memo-carried order metadata.
- Document exact memo framing after measuring real size limits.
- See [zcash-integration-todos.md](./zcash-integration-todos.md).

## Phase 3 — MCP / AI buyer

- MCP tools for offer discovery and `request_payment(invoice)`.
- Strict tool schema that cannot expose wallet secrets.
- Human approval hooks for `REQUIRE_APPROVAL` decisions.

## Phase 4 — Web marketplace UI

- Browse/search offers.
- Human-friendly purchase flow.
- Still no requirement to custody user spending keys in the web tier.

## Phase 5 — Hosted indexing / commercial infrastructure

Optional **ZADP Cloud** services that speak the open protocol:

- Hosted Zcash indexer
- High-performance offer search API
- MCP gateway
- Real-time offer feeds
- Payment webhooks
- Managed seller infrastructure
- Analytics
- Reputation / trust data
- Private enterprise marketplaces

The open-source protocol and reference implementation remain usable without ZADP Cloud.

## Explicitly deferred

Escrow, DAO, tokens, DID systems, production hardened wallets, cross-chain abstraction layers, and recommendation engines.

## Current mainnet discovery milestone

Implemented: signed portable board contract, actual network scanner/decryption, durable admission/reorg state, confirmation-aware GET /offers, and explicit local-wallet shielded publication CLI. Offline tests include real Orchard note encryption and two independent reconstructed indexes.

Still required before declaring this milestone complete: provision the dedicated mainnet board, integrate the funded local Zallet version, review the computed publication fee, explicitly publish one real OFFER, discover its mined output from the actual service, and reconstruct it with a fresh independent indexer. Preserve txid/descriptor/HTTP evidence without wallet data. Follow [the live checklist](mainnet-publishing.md).

Only after that acceptance: update/revoke semantics, portable delegated seller authorization, operational readiness/metrics and stronger service verification. Production purchasing/AI buyer, reputation and hosted marketplace remain future scope; the existing purchase demonstration is mock-only.

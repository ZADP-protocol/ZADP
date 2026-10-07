# ZADP Roadmap

Status labels reflect the **reference repository today**. First live mainnet publish → mine → scan → API acceptance completed on 2026-10-05; [public evidence](./mainnet-acceptance.md). The retained Rust commerce demo is mocked; the experimental local text service
has an authenticated payment gate tested with synthetic observations. A live
buyer payment and paid fulfillment acceptance remain pending.

## DONE

- ZADP/1 signed `offer` messages (JSON + canonical CBOR signing).
- Seller Ed25519 identity (`identity-init` / `identity-show`).
- Board descriptor + operator signature + admission policy.
- 512-byte ZIP 302 memo framing (`0xf5` + `ZADP` + wire v1/v2 + CBOR tuple).
- CompactTxStreamer scanner (`RealZcashScanner` / `BoardSource`) with trial decrypt + full memo fetch.
- SQLite indexer with cursor, first-wins replay rules, reorg rewind.
- Mainnet board HTTP: `GET /healthz`, `GET /offers` (`board_router`).
- Seller CLI: `board-sign`, `publish` via local Zallet PCZT (dry-run, confirm, final-tx verify).
- Mock vertical slice: invoice / policy / mock pay / fulfill (`zadp-demo`).
- Offline tests including real Orchard note encryption and independent SQLite reconstructions.
- Docs + CI (fmt, clippy, workspace tests, demo smoke).
- First mined mainnet OFFER, independently reconstructed by two fresh indices.
- Reviewed Zallet library scan fixes with pinned sources and regression tests.
- English local UI: discovery/details, unsigned drafts and exact memo byte budget.
- 86 cross-language cases compared with the actual Rust memo encoder, including
  UTF-8 field limits (title 128, capability/board ID 64 bytes) and 504-byte payload.
- Optional filtered wallet balances/sync/receive QR and authoritative unsigned
  publication preview with signed-board validation.
- Experimental local TLS text service: durable invoices, exact shielded-payment
  correlation, three-confirmation gate, reorg rechecks and idempotent results.
- Local UI invoice instructions and read-only fulfillment checks; no spending route.

## Current MVP focus

The primary story is signed offers → real Zcash publication → independent
indexing → multi-board discovery → geographic discovery → human web UI and
machine-readable discovery. Publish to a service area; match locally without
publishing the buyer’s location. Commerce remains available but is not the MVP
acceptance gate.

### P0 — geographic discovery (implemented reference path)

- Backward-compatible Offer/wire v1 plus signed Offer/wire v2 geo.
- Optional seller area, public-data warning and authoritative memo budget.
- Existing board v1, persistent index and public API expose signed geo.
- Buyer click-only location, in-memory matching, Nearby/Anywhere and radius zero.
- Protocol, privacy, regression tests and documentation.

### P1 — adoption and release verification

- Shared local matcher and small machine/agent example (implemented).
- Reproducible offline signed/memo/index/API/browser demo (implemented).
- Wider independent-reader compatibility and browser/device review.
- After explicit human review: protected deployment of the new reader/writer
  binaries and one first geographic mainnet offer. No geo offer has been
  published by this implementation task.

### Post-MVP — retained commerce and operations

- Real paid commerce acceptance with separate buyer wallet, stable HTTPS seller,
  exact payment preview and explicit approval. Existing working demos remain.
- Hosted seller infrastructure / SLA, advanced commerce and wallet integration.
- Optional maps, geocoding, routing and address semantics (no dependency today).
- Rich moderation, reputation and hosted marketplace features.

## LATER

- `offer_update` / `offer_revoke` wire acceptance.
- Portable delegated seller authorization beyond operator allowlists.
- Stronger spam economics (proof-of-payment-to-publish, global quotas).
- Full marketplace purchasing UI beyond the current local discovery/draft interface (no custody of user spending keys in the web tier).
- Reputation / trust data, private enterprise marketplaces.
- Broader ecosystem: independent indexers, alternate wallets, community boards.

## Explicitly deferred

Escrow, DAO, tokens, DID systems, production hardened custody products, cross-chain abstraction layers, and recommendation engines.

## Hosted coexistence

Optional hosted indexing or managed APIs may be sold while the open protocol and reference crates remain usable without them.

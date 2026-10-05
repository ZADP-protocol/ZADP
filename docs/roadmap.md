# ZADP Roadmap

Status labels reflect the **reference repository today**. First live mainnet publish → mine → scan → API acceptance completed on 2026-10-05; [public evidence](./mainnet-acceptance.md). The retained Rust commerce demo is mocked; the experimental local text service
has an authenticated payment gate tested with synthetic observations. A live
buyer payment and paid fulfillment acceptance remain pending.

## DONE

- ZADP/1 signed `offer` messages (JSON + canonical CBOR signing).
- Seller Ed25519 identity (`identity-init` / `identity-show`).
- Board descriptor + operator signature + admission policy.
- 512-byte ZIP 302 memo framing (`0xf5` + `ZADP` + wire v1 + CBOR tuple).
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
- 44 cross-language cases compared with the actual Rust memo encoder, including
  UTF-8 field limits (title 128, capability/board ID 64 bytes) and 504-byte payload.
- Optional filtered wallet balances/sync/receive QR and authoritative unsigned
  publication preview with signed-board validation.
- Experimental local TLS text service: durable invoices, exact shielded-payment
  correlation, three-confirmation gate, reorg rechecks and idempotent results.
- Local UI invoice instructions and read-only fulfillment checks; no spending route.

## IN PROGRESS

- Prepare a separately approved mainnet paid test. No new publication or buyer
  payment has been authorized; loopback endpoint is not publicly reachable.

## NEXT (priority order)

1. **Paid service acceptance** — review a real endpoint and fresh publication
   preview, obtain explicit approval, then separately review buyer payment costs;
   verify discovery → invoice → shielded payment → seller observation → result.
2. Stable self-hosted API / ops docs (bind, TLS termination, sync readiness beyond `/healthz`).
3. Optional hosted indexing / high-availability API (commercial; protocol stays self-hostable).
4. Agent / MCP buyer tools with strict schemas (still no spending keys in the model).
5. Broader buyer wallet integration, service expiry/refund policies and repeatable
   real-network acceptance beyond the first text service.

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

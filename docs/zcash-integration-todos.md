# Zcash integration: status and remaining work

Updated 2026-10-17 against the reference implementation and dated acceptance
records. This replaces the earlier mock-era TODO inventory. Implementation,
mainnet acceptance and hosted operator policy are different kinds of evidence.
See [mainnet evidence](mainnet-evidence.md) and [roadmap](roadmap.md).

## Implemented and exercised

### Publication and discovery

- **Canonical Offer wire 3:** signed model, deterministic encoding/decoding,
  512-byte memo envelope / 504-byte payload limit and Rust/browser parity tests.
  Historical wire 1/2 remains readable; supported new-publication tooling uses
  wire 3. No signed historical Offer is rewritten.
- **Signed BoardDescriptor:** dedicated board receiver, incoming viewing key,
  network, birthday and signed admission policy; descriptor fingerprint pinning
  and exact receiver binding. A descriptor is not spending authority.
- **Real chain scanning:** CompactTxStreamer downloads, trial decryption,
  reorg-aware persistent SQLite indexing, confirmation metadata and isolated
  multi-board databases. Lightwalletd trust is not full-node consensus validation.
- **Mainnet publication and reconstruction:** first funded publication and two
  independent indices on 2026-10-05; geographic and canonical wire-3 publications
  followed. These are completed milestones, not only encrypted fixtures.
- **Wallet publication path:** local Zallet PCZT preparation, exact fee/output
  review, approval, broadcast and final transaction checks. Browser/external-wallet
  handoff is a separate Web path; a local wallet bridge is not mandatory for
  every publisher. Wallet spending authority stays outside discovery/MCP.
- **Real paid discovery examples:** three wire-3 Miami gaming Offers, 90 days each,
  published on 2026-10-16. Actual combined publication amounts plus network fees:
  0.00767574 ZEC. They are fictional discovery-only listings, not service purchases.
  [Offer IDs, txids and block heights](mainnet-discovery-mcp-2026-10-16.md).

### Direct seller settlement

- **Incoming payment observation and fulfillment:** a human mainnet purchase of
  the deterministic summary service completed after exact payment correlation
  and confirmations, with `execution_count=1` recorded for that acceptance.
- **Agent preparation with human payment:** a later fresh order completed on
  2026-10-11 after agent discovery/validation/order preparation and payment from
  the user's separate phone wallet. That attempt did not independently expose
  payment txid, fee or execution count; do not infer them from the earlier order.
- **Invoice and policy checks:** signed Offer price/endpoint boundaries, invoice
  validation, ZIP-321 presentation and PaymentController approval gates exist.
  Offline agent settlement tests remain synthetic, but the real controller /
  wallet adapter also completed a separate mainnet test on 2026-10-14.

See [commerce](commerce.md), [privacy](privacy.md) and the exact
[acceptance boundaries](mainnet-evidence.md). The [real agent-controller purchase](agent-mainnet-acceptance-2026-10-14.md)
paid 1000 zat plus a 10000-zat wallet fee and fulfilled once. It used legacy wire 1
and separate accounts under one wallet/operator. No universal cross-wallet or
canonical wire-3 agent commerce acceptance is claimed.

### Hosted and agent access (separate repositories)

- ZADP-Hosted applies optional visibility/moderation and hosted publication-payment
  checks after canonical indexing. Indexing is not automatic public approval.
  Hosted fiat-linked prices are not signed Protocol 1.0 price constants.
- ZADP-Web supplies the globe, local Nearby matching and wallet handoff UI.
  Buyer GPS is not sent in discovery queries.
- ZADP-MCP provides four read-only discovery tools over moderated Hosted routes.
  Production Inspector acceptance on 2026-10-16 passed 56 checks across revisions
  2026-07-28 and 2025-11-25. It has no publication, order, wallet or payment tools.
  See [MCP discovery](mcp.md).
- Instance systemd/Caddy, deployment, backups, monitoring and wallet operations
  belong to ZADP-Server. Core does not require its private configuration.

## Remaining engineering and acceptance work

- **Broader real-wallet agent acceptance:** extend the completed legacy
  controller-driven purchase to canonical wire-3 services and independent wallet
  owners. Preserve exact recipient/memo/amount/fee/total/expiry review and record
  fulfillment. A same-operator test does not establish those broader cases.
- **Wallet interoperability:** add dated evidence for additional wallets and
  adapters, including invoice/memo compatibility, fees and failure recovery.
- **Broader seller capabilities:** move beyond deterministic reference text
  examples without weakening signed-term or payment confirmation checks.
- **Seller retention:** implement automated input/result purge/redaction with
  replay-safe tombstones. Current reference SQLite input/results are plaintext
  under filesystem permissions, not confidential from the seller operator.
- **Board sharing architecture:** design opt-in UIVK/signed-descriptor distribution,
  trust/pinning, historical recovery and multi-board policies. Sharing the same
  board enables reconstruction; moving Offers between distinct boards is not
  automatic. [Design scope](board-sharing-design.md).
- **Offer changes:** `offer_revoke` is implemented and offline-tested; a real
  mainnet revocation needs its own evidence. `offer_update` remains unimplemented.
- **Operational assurance:** extend independent client/reorg/recovery/load
  acceptance and keep tip-lag monitoring distinct from indexer liveness. There is
  no universal production SLO implied by a successful test.
- **Dependency findings:** resolve and retest the dated [dependency review](dependencies.md)
  findings rather than treating documentation work as a passing security audit.
- **Abuse resilience:** existing signed board limits and optional hosted controls
  do not eliminate Sybil attacks or distributed DoS. Keep generic Core admission
  separate from a host's content policy.

## Reproducible evidence and implementation boundaries

[Development checks](development.md) cover formatting, Clippy, Rust tests,
reference commerce process tests, browser modules, memo parity and doc links.
Live acceptance is additional evidence, not reproduced by offline CI. Previously
published transactions must never be replayed to refresh documentation.

Indexer/API code needs incoming viewing capability, not a seed or spending key.
Do not pass wallet credentials to discovery services, models or MCP tools.
Every new spend/publication follows its own wallet and authorization boundary;
this status document does not initialize wallets or authorize transactions.

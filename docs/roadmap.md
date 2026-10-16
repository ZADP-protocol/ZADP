# ZADP roadmap

Updated 2026-10-17. Engineering milestones and priorities, not normative protocol
semantics or a promise of release dates. Core and the optional hosted products
are listed separately where their responsibilities differ.
See [Protocol 1.0](protocol.md), [architecture](architecture.md) and
[dated mainnet evidence](mainnet-evidence.md).

## Delivered

- Protocol 1.0 canonical Offer/model/wire 3; frozen historical wire 1/2 reading.
- Signed boards, per-board SQLite isolation, reorg-aware indexing and multi-board
  read-only discovery. Independent reconstruction of mainnet publications.
- Optional geographic service areas and browser-local Nearby matching; the hosted
  globe uses self-hosted cartography.
- Offline-signed seller endpoint ownership proof with bounded, SSRF-safe fetching.
- Direct seller commerce and confirmation-gated deterministic fulfillment.
  Completed human purchase, agent-prepared human-paid acceptance and a real
  PaymentController / ReviewedWalletAdapter purchase on 2026-10-14. The latter
  used legacy wire 1 and separate accounts under one operator/wallet.
- Real-ZEC publication of canonical wire-3 discovery Offers, including three
  90-day Miami gaming examples on 2026-10-16. Publication payment is distinct
  from buying the advertised service.
- Optional ZADP-Hosted moderation/reporting and publication-payment visibility
  enforcement, separated from the independently runnable protocol repository.
- Separate read-only ZADP-MCP adapter and connection portal. Four discovery tools
  were exercised against the paid mainnet publications using MCP Inspector:
  56 checks passed across protocol revisions 2026-07-28 and 2025-11-25.
- Additive seller-signed `offer_revoke` implementation and offline regression tests;
  mainnet revocation acceptance is not established by this roadmap.

## Current priorities

- Public source readiness: resolve/retest the dependency findings recorded in
  [dependency review](dependencies.md); keep documentation and evidence current.
- Preserve explicit boundaries between protocol validity, signed board admission,
  hosted visibility, endpoint ownership and seller order availability.
- Seller retention/redaction with replay-safe tombstones and recovery. Automated
  purge/redaction is not implemented in the reference seller.
- Extend real-wallet agent acceptance to canonical wire-3 commerce and independent
  wallet owners, with exact fee proposals, policy boundaries and sanitized
  fulfillment evidence. The completed legacy same-operator test has narrower
  scope; see [agent-controller acceptance](agent-mainnet-acceptance-2026-10-14.md).
- Broader capabilities and request/response interoperability beyond deterministic
  reference text services.

## Next

- **Board UIVK / signed-descriptor sharing architecture:** design opt-in discovery,
  distribution, trust/pinning, recovery and multi-board operation. Independent
  indexers should reconstruct the same board while applying their own hosted
  visibility rules. Distinct boards retain their own signed admission policies
  and jurisdiction-specific operator rules. No automatic sharing protocol exists
  today; see the [design scope](board-sharing-design.md).
- More independent clients, indexers, wallet adapters and MCP interoperability
  evidence, with no dependency on the official hosted service.
- Seller-signed `offer_update` design/implementation; currently unimplemented.
- Further hosted abuse controls in ZADP-Hosted, without imposing them on Core.

## Deferred / outside current scope

Escrow, reputation, DAO/token systems, hosted custody and cross-chain abstractions.
No roadmap item authorizes funds, publication, deployment, wallet provisioning or
an automatic sweep. Hosted prices and rollout remain operator policy, not
Protocol 1.0 constants.

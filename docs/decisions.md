# Architectural decisions

Updated 2026-10-05. These decisions distinguish live discovery from the retained
mock commerce demo. First mainnet publication and discovery are verified;
an experimental local payment-gated text service is implemented with synthetic
acceptance tests. A live buyer payment and delivered result remain unverified.

## ADR-001: Rust workspace with domain crates — retained

**Decision:** Keep protocol, indexer, API, payment, seller, Zcash integration
and demo orchestration in separate crates under `crates/`.

**Why:** Domain logic stays independent of wallet credentials, HTTP and chain
transport. The local web UI in `ui/` consumes discovery separately.
`cargo run -p zadp-demo` remains the mock commerce demonstration.

## ADR-002: Separate real discovery from mock commerce — evolved

**Original Phase 1 decision:** Use `MockZcashAdapter` behind `ZcashAdapter` for
an in-process publication/payment demo without a live chain.

**Current decision:** Real board discovery uses `RealZcashScanner` through
`BoardSource`, with CompactTxStreamer compact blocks and matching full
transactions. Real publication uses the separate seller CLI and local Zallet
PCZT RPC. The mock adapter is retained for commerce; it is not the live
discovery implementation and does not prove real purchase settlement.

**Why:** Read-only indexing and privileged publication require different
permissions. Neither depends on the mock payment ledger.

## ADR-003: Ed25519 seller identity independent of Zcash keys — retained

**Decision:** Sign offers with Ed25519 (`ed25519-dalek`). Seller identity keys
are distinct from wallet transaction keys. The indexer verifies offer
signatures; transaction signing stays inside the local wallet.

**Why:** Anyone can verify offer origin without spending authority. A valid
signature does not guarantee service quality or delivery.

## ADR-004: Canonical signing and compact memo transport — evolved

**Decision:** Sign deterministic canonical CBOR and expose decoded offers as
JSON through REST. The memo uses a compact 13-field CBOR tuple, not the same
encoded map as the signature input. Decoding reconstructs the signed offer
and preserves the fields used for signature verification.

The memo is 512 bytes: an 8-byte framing header and at most 504 payload bytes.
Optional `description` is included on chain when present and shares the budget
with the title, capability, endpoint and metadata. Oversized offers are rejected
by the Rust encoder; the UI also provides an unsigned size preview.

**Why:** Compact transport fits the memo limit while preserving independently
verifiable signatures. UTF-8 byte length, not character count, determines size.
See [protocol](./protocol.md).

## ADR-005: Persistent SQLite indexing — supersedes in-memory-only Phase 1

**Original decision:** In-memory repositories were sufficient for the demo.

**Current decision:** Live discovery uses `SqliteOfferRepository`, persistent
scan cursors, first-valid-wins replay handling and reorg rewind. In-memory
repositories remain useful for tests and the mock demo.

**Why:** Restart recovery and independent reconstruction require durable state.
Two fresh SQLite indices reconstructed the same first live mainnet offer from
chain data and the signed descriptor; see [public evidence](./mainnet-acceptance.md).

## ADR-006: Separate runtime processes; retain single-command demo — evolved

**Decision:** Retain `zadp-demo` for mock commerce. Live operation separates
the chain node, wallet, board scanner/API and browser UI. The board API exposes
`GET /healthz` and `GET /offers` without payment or wallet routes.

**Why:** Indexers must operate without wallet access. HTTP liveness does not
prove completed synchronization; inspect scanned height and confirmations.

## ADR-007: Public board UIVK without custody — implemented

**Decision:** Use a dedicated board account and a signed portable descriptor
binding address, incoming viewing authority, network, birthday and policy.
The reference descriptor uses an Orchard-only UA and UIVK; the scanner handles
Orchard and Ironwood outputs and requires the exact intended receiver.

The **Unified Incoming Viewing Key (UIVK)** lets independent indexers decrypt
incoming board offers and values. It **cannot sign transactions or spend ZEC**.
Indexers receive no seed, spending key, wallet authentication or private seller
viewing authority. The browser can read indexed JSON without holding any key.

**Why:** Public discovery must be independently reproducible without control
of funds. Board content is deliberately visible to descriptor holders; personal
accounts and private seller viewing keys must not be reused or published.
See [board descriptor](./board-descriptor.md).

## ADR-008: Human-reviewed publication through local Zallet — implemented

**Decision:** Prepare a PCZT locally, show library-derived fee/destination
details, require human confirmation, verify the extracted final transaction,
and record a public receipt. Automated monitoring must not publish or retry.

The verified beta.3 deployment needed two reviewed librustzcash scan fixes.
Pins, regression patches and build instructions are versioned in
[integrations/zallet](../integrations/zallet/README.md); wallet data is excluded.

**Why:** Discovery authority must not become spending authority. A transport
timeout can leave acceptance unknown; check the transaction before any retry.
The first authorized publication is already mined and independently indexed,
and must not be replayed.

## ADR-009: English local UI with unsigned drafts and a free test service

**Decision:** Keep the initial UI local and English-only. It reads indexed
offers, shows details, stores unsigned drafts in the browser and previews the
memo byte budget. A bounded local text-shortening demo exercises request/response
without an AI provider, payment or blockchain publication.

**Why:** Validate service interaction before adding wallet or purchase operations.
The demo is separate from the mainnet test offer's placeholder endpoint.
See [UI instructions](../ui/README.md).

## ADR-010: Read-only operator boundary and exact unsigned previews

**Decision:** Optional owner-only configuration connects a local server to the
existing seller wallet. Only filtered balances/sync/receive address reach the UI.
Publication previews call the actual CLI with `--dry-run --preview-output`; board
verification and library fee policy remain authoritative. There is no browser
route for transaction signing, broadcasting or automatic approval.

The JavaScript memo counter is tested against the Rust encoder in 86 cases.
Shared payload and per-field UTF-8 limits both apply; character limits alone
cannot ensure a valid memo.

## ADR-011: Deterministic local service with seller-observed payment

**Decision:** The initial paid service returns the first three sentences without
external APIs. Private persistent invoices bind exact seller account, receiver,
amount and memo. Only an authenticated read-only Zallet observer supplies chain
evidence. Require three confirmations, invoice-window mining, shielded non-change
outputs, single-output consumption and evidence rechecks before each retrieval.
Tokens are hashed at rest; client-supplied payment evidence is rejected.

**Why:** A client assertion or a mempool transaction is insufficient to release
paid output. Synthetic tests prove the gate and restart behavior, not mainnet
settlement. The UI bridge is fixed loopback HTTPS with certificate verification;
it never follows untrusted advertised endpoints. Deployment, a new valid offer,
buyer fee preview and live payment require separate human review.

## Remaining decisions

- Live buyer payment acceptance and public deployment of the experimental service.
- Actual service endpoint/protocol and integration with a real service provider.
- Buyer wallet proposal preparation and approval UX; refund and retention policy.
- Update/revoke transport and conflict handling beyond current OFFER replay rules.
- Public API authentication, TLS, resource limits and operational readiness.

Current priorities: [roadmap](./roadmap.md). Security limits:
[threat model](./threat-model.md).

## ADR-012: Direct buyer-to-seller commerce, separate from discovery

**Decision:** The first general commerce reference is independently runnable
Rust `zadp-commerce`, serving deterministic `text.uppercase` orders. Reuse the
signed offer's endpoint; no new OFFER fields or public PAID message. Browsers,
CLI clients and agents use the same seller API directly. ZADP discovery never
custodies/routes payments or owns orders, negotiated prices, buyer input/results
or purchase history. The older Python summary service/loopback UI bridge is
retained as a separate compatibility demo, not a mandatory intermediary.

**Authorization and payment:** A random private idempotency capability recovers
one order; a domain-separated claim token grants anonymous retrieval. Only hashes
are persisted. A unique order memo and ZIP-321 request bind exact seller receiver
and amount. Read-only authenticated evidence, confirmation policy, mining window,
non-change output and unique output consumption gate the durable state machine.
Execution/result storage is atomic; retrieval rechecks evidence after restart.

**Validation:** The direct browser/mock seller slice completes locally and its
QR decodes to the exact invoice. No new mainnet offer, payment or wallet migration
was made. A production HTTPS endpoint, independent buyer wallet, reviewed fee
preview and explicit human approval remain prerequisites for real settlement.
[Commerce design and operating limits](commerce.md) include bounded observation,
abuse/retention concerns and schema/config migration requirements.

## ADR-013: Geographic discovery is a primary MVP capability

**Decision:** Separate optional public signed offer areas from private local
buyer location. Offer/wire v2 adds a compact integer triple with its own existing
canonical-CBOR signing domain. Offer/wire v1 bytes and BoardDescriptor v1 remain
unchanged. The indexer exposes accepted geo through existing GET /offers and
stores full OFFER JSON without a migration. Unknown query fields are rejected;
there is no coordinate search endpoint.

**Client:** One reusable pure Haversine matcher intersects buyer and offer circles.
Nearby excludes global offers; Anywhere includes both. Buyer radius zero checks
whether the offer area includes the point. Location acquisition is click-only,
denial is non-blocking, and buyer state is never persisted or transmitted.
Seller public-area drafts are intentionally separate. No maps or order geo fields.

**Priority:** Signed publication, independently reproducible multi-board indexing,
local human/agent discovery form the MVP. Retain commerce, but real payments,
hosted SLA and richer purchase flows move behind this discovery milestone.
[Compatibility](geographic-discovery-compatibility.md) and [guide](geographic-discovery.md).

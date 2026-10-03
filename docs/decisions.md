# Architectural decisions (Phase 1)

## ADR-001: Rust workspace with domain crates

**Decision:** Implement protocol, indexer, API, payment, seller, and Zcash adapter as separate crates under `/crates`, plus demo binaries under `/examples`.

**Why:** Clear boundaries, replaceable infrastructure, still one local workspace (`cargo run -p zap-demo`).

## ADR-002: Mock Zcash behind a trait

**Decision:** All chain I/O goes through `ZcashAdapter`; MVP uses `MockZcashAdapter` (in-process bus + payment ledger).

**Why:** Full demo without testnet; real adapter later without rewriting commerce logic.

## ADR-003: ed25519 seller identity, independent of Zcash keys

**Decision:** Offers are signed with ed25519 (`ed25519-dalek`). Zcash keys remain adapter-internal.

**Why:** Established primitive; no invented crypto; matches “sign offers independently of the Zcash transaction.”

## ADR-004: Canonical CBOR for signing; JSON for REST

**Decision:** Sign canonical CBOR bytes; expose JSON on the wire for humans/REST. Memo packing uses the same CBOR body later.

**Why:** Deterministic signatures; compact path toward 512-byte memos; JSON remains debuggable.

## ADR-005: In-memory repositories

**Decision:** No PostgreSQL/SQLite requirement in Phase 1.

**Why:** Fastest understandable MVP; persistence is a later infrastructure swap.

## ADR-006: Single-process demo orchestration

**Decision:** `examples/zap-demo` starts in-memory services and prints the vertical slice narrative.

**Why:** One command local demo; avoid premature microservices.

## Unresolved product questions

- Exact Zcash board addressing and memo framing (see zcash-integration-todos.md).
- Whether offer `description` lives on-chain or is fetched from `endpoint` when memo space is tight.
- Multi-indexer conflict resolution beyond “first valid wins.”

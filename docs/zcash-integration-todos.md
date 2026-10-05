# Zcash Integration TODOs

Audit of Zcash-facing work against the **current** codebase. Do not treat offline fixtures or transport probes as live mainnet acceptance.

**Hard rules (unchanged):** indexer never gets a spending key; no Zcash secrets in LLM/agent processes; do not invent unverified RPC shapes in app code without checking the pinned wallet/lightwalletd APIs.

## Discovery / publication (code status)

| Item | Status | Notes |
| --- | --- | --- |
| Memo encode/decode + 512-byte framing | **Done** | `zadp-protocol` memo module; max CBOR payload 504 |
| Board addressing (Orchard-only UA + UIVK + birthday) | **Done in code** | Dedicated signed board provisioned; first live acceptance verified |
| Read-only CompactTxStreamer scanning | **Done** | `RealZcashScanner`; viewing only |
| Reorg-aware SQLite indexing | **Done** | Cursor + rewind; confirmations vs policy |
| Shielded OFFER publish (Zallet PCZT) | **Done (developer preview)** | Local wallet RPC; dry-run / confirm / final-tx verify |
| View vs spend separation | **Done** | Scanner/API vs Zallet publish boundary |
| Live mainnet publish → mine → scan → API | **Verified 2026-10-05** | [Public evidence](./mainnet-acceptance.md) |
| Independent second empty-DB indexer on live tx | **Verified 2026-10-05** | Matching offer, transaction, policy acceptance and listing value |

## Settlement / commerce (real chain)

| Item | Status |
| --- | --- |
| Shielded buyer payments | **Open** (mock `ZcashAdapter` only) |
| Seller payment observation on chain | **Experimental code + synthetic tests** — authenticated read-only Zallet observer; real paid acceptance pending |
| Invoice `pay_to` / UA conventions across wallets | **Open** |
| Binding `invoice_id` inside encrypted payment memo | **Experimental code** — exact text memo; cross-wallet live acceptance pending |

## Live acceptance checklist (first run completed)

The following procedure completed on 2026-10-05. It remains a checklist for new deployments, not instructions to recreate the existing wallet or replay the test publication:

1. Provision dedicated **board** and **seller** accounts (not reused personal wallets).
2. Create Orchard-only board UA + incoming viewing capability; capture birthday height.
3. Sign and pin the public board descriptor (fingerprint via a trusted channel).
4. Human seller funding; library-derived fee preview; explicit publication confirmation (or documented `--dry-run` then confirm).
5. Broadcast; wait for mining/inclusion; record txid, fingerprint, height.
6. Run scanner → query `GET /offers?capability=…` and match the signed offer + chain metadata.
7. Start a **second** scanner with the same descriptor and a **fresh empty database**; verify identical offer identity and inclusion metadata (scanned heights/confirmations may differ).
8. Archive **public** evidence only — never wallet credentials, seeds, or auth files.

A local encrypted fixture is **not** a substitute. Details: [mainnet-publishing.md](./mainnet-publishing.md).

## Previously unresolved questions (updated)

1. **Memo capacity:** Closed in code for this path — fixed 512-byte ZIP 302 memo, 504-byte CBOR budget. Revisit only if pool/library framing changes.
2. **Board addressing:** Closed in code — Orchard-only UA + UIVK in signed descriptor. Live dedicated board provisioning and signed descriptor verified.
3. **Scan model:** Closed in code for reference — lightwalletd CompactTxStreamer + SQLite reorg handling. Residual: trust in the lightwalletd service (not a validating full node).
4. **Payment correlation:** Implemented in the experimental local text service; live buyer settlement, standardized invoice semantics and wallet interoperability remain open.
5. **Fee / change / UA for purchases:** Still open for real settlement; publish path uses Zallet PCZT fee inspect today.
6. **Library / process choice:** Partially decided — `librustzcash` decrypt path, Zallet PCZT for publish, lightwalletd for compact blocks/broadcast. Re-evaluate if upstream APIs shift.
7. **Testnet / ops SLOs:** Open (confirmations, tip lag, runbooks).
8. **Spam resistance:** Partial — listing value + per-seller limits + optional allowlist; open-board Sybil and global DoS remain.

## Repository polish (observed, not modified by doc tasks)

| Item | Observation |
| --- | --- |
| License texts | Dual-license declaration with `LICENSE`, `LICENSE-MIT` and `LICENSE-APACHE` present |
| CI | `.github/workflows/ci.yml`: fmt, clippy `-D warnings`, `cargo test --workspace`, `zadp-demo` smoke, Python/Node suites, Rust memo parity and relative doc links |
| Ignore rules | Local agent inbox under `/assistant-agent/` is gitignored |
| Examples | `examples/README.md` → `zadp-demo`; `crates/zadp-zcash/examples/network_probe.rs` (probe ≠ acceptance) |
| Deploy packaging | No in-tree Docker/systemd/TLS termination for public API |

## Non-goals for Zcash wrappers

- Do not redesign domain types when swapping mock commerce → real settlement.
- Do not give the indexer a spending key.
- Do not pass Zcash secrets into LLM/agent processes.
- Do not claim production readiness from unbroadcastable fixtures or network probes alone.

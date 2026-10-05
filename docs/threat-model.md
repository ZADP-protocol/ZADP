# ZADP Threat Model

This threat model covers the **current** ZADP reference implementation: real CompactTxStreamer board discovery, SQLite indexing, seller publish CLI, the retained mock commerce demo, optional local operator UI and experimental payment-gated text endpoint. First live mainnet discovery acceptance was verified on 2026-10-05; see [public evidence](./mainnet-acceptance.md).

## Assets

| Asset | Sensitivity |
| --- | --- |
| Buyer Zcash spending key | Critical |
| Seller Zcash spending key | Critical |
| Board UIVK / public board descriptor | High (intentional discovery surface) |
| Seller Ed25519 identity private key | High |
| Offer authenticity and freshness | High |
| Invoice / order metadata (experimental local service) | High |
| Buyer purchase graph | High (privacy) |
| Indexed offer catalog integrity | Medium |

## Trust boundaries

```mermaid
flowchart LR
  Agent[AI agent / LLM]
  PayCtl[Payment controller]
  Wallet[Buyer or seller wallet]
  Indexer[Indexer + board UIVK]
  Seller[Seller service]
  Lwd[lightwalletd CompactTxStreamer]
  Zallet[Local Zallet PCZT]
  Chain[Zcash network]

  Agent -->|request_payment only| PayCtl
  PayCtl --> Wallet
  Wallet --> Chain
  Zallet --> Lwd
  Indexer --> Lwd
  Seller --> Chain
  Agent --> Indexer
  Agent --> Seller
```

**Hard invariant:** no AI or LLM process may possess a Zcash spending key. The agent may request payment; only a separate payment controller + wallet may sign/spend. The indexer holds board **incoming viewing** material only.

## Threats and mitigations

Status labels: **Implemented** (in current code), **Partial**, **Remaining**.

### Compromised LLM / prompt injection

- **Risk:** Agent is manipulated into requesting many payments, wrong sellers, or exfiltrating data it can see. Public offer text/URLs may contain adversarial instructions.
- **Implemented:** Spending keys never in the model; mock path uses `request_payment` + policy (`ALLOW` / `DENY` / `REQUIRE_APPROVAL`) with ceilings; consumers must treat offer fields as untrusted text (escape, never execute as instructions).
- **Remaining:** Production AI buyer tooling, stronger human-approval UX, hosted gateway hardening.
- **Residual:** Agent can still spam invoice requests and leak offer or invoice inputs.

### Malicious seller

- **Risk:** Fake capability, bait pricing, non-delivery after payment, malicious HTTPS endpoint.
- **Implemented:** Ed25519 offer identity; endpoint bound into signed offer; board admission requires HTTPS with host and without userinfo; indexer does **not** fetch endpoints.
- **Remaining:** Escrow, reputation, bonds, dispute flows (out of scope).
- **Residual:** No service-quality guarantee; buyer accepts counterparty risk for real purchases (live buyer acceptance still pending).

### Malicious buyer

- **Risk:** Abuse seller endpoints, unpaid load, scrape or DoS seller APIs.
- **Partial:** Discovery is read-only. Rust commerce remains mocked; the separate local text service authenticates seller observations and gates results on exact confirmed shielded outputs. Synthetic tests do not prove live paid acceptance.
- **Implemented local gate:** Exact account/receiver/amount/memo, non-change shielded output, invoice mining window, three confirmations, single-output use and reorg rechecks. TLS verification and fixed loopback routing prevent arbitrary offer-controlled requests.
- **Remaining:** Public service deployment/auth, stronger rate limits, retention/refund policy and live buyer payment acceptance.

### Forgery / modified offer

- **Risk:** Attacker alters price, endpoint, or capability in transit or in indexer storage.
- **Implemented:** Ed25519 over canonical CBOR (`ZADP1\0offer\0`); indexer verifies before admit; clients SHOULD re-verify retrieved offers.
- **Residual:** A compromised indexer can omit valid offers or prefer attacker-favored valid ones.

### Malformed / oversized payload

- **Risk:** Crash or confuse indexers with junk memos.
- **Implemented:** ZIP 302 framing checks (magic, wire v1, length ≤504, zero padding); unsupported types rejected; unrelated memos ignored; rejections store reason codes/output ids — not raw private memo content.

### Replay / duplicate publication

- **Risk:** Rebroadcast of an old valid signed offer or double-admit of the same output.
- **Implemented:** `expires_at`; nonce bound into signature; SQLite first-wins on `(seller,id)`, `(seller,nonce)`, and `(txid,pool,output_index)`; duplicate outputs/blocks idempotent.
- **Remaining:** `offer_update` / `offer_revoke` wire acceptance.

### Expiry / reorg

- **Risk:** Buyer acts on stale terms; chain reorg orphans indexed offers.
- **Implemented:** API lists only unexpired offers; admission uses inclusion **block time** for TTL/rate; reorg rewind deletes orphan heights (cascades offers/rejections).
- **Residual:** lightwalletd is trusted for chain data; scanner is not a consensus-validating full node. Full-transaction requests can reveal which transactions interest the indexer.

### Compromised indexer

- **Risk:** Censorship, selective omission, query surveillance.
- **Implemented / design:** Clients can verify signatures independently; anyone can run another indexer from the same public descriptor; DB pins board fingerprint (silent descriptor swap refuses open).
- **Residual:** Omission and query logging remain possible for a malicious operator. Operator signatures authenticate a pinned key, not a real-world identity; pin the descriptor fingerprint through a trusted channel.

### Viewing / spending key exposure

- **Risk:** UIVK leak exposes board discovery traffic; spending key leak drains funds.
- **Implemented:** Dedicated board accounts recommended; UIVK ≠ spend; seller identity and wallet auth files are owner-only, regular, symlink-resistant (`O_NOFOLLOW`), never printed; publish path keeps spending in local Zallet; RPC bodies/errors suppressed to avoid wallet leaks.
- **Remaining:** Hardware-backed seller/buyer wallets; broader OS secret-store guidance.
- **Residual:** Public UIVK is an intentional discovery surface — board traffic is not private from descriptor holders. Use a dedicated board account to isolate discovery from personal and purchase traffic.

### Spam / Sybil on open boards

- **Risk:** Flood board with validly signed junk; many identities bypass per-seller caps.
- **Implemented:** `min_listing_value_zat`, per-seller active and rate-window limits, optional `authorized` allowlist.
- **Remaining:** Stronger economic admission controls, global quotas, and operator tooling; the listing-value requirement is already enforced.
- **Residual:** `open` admission remains Sybil-vulnerable; disk/network DoS via many incoming txs still possible.

### Malicious endpoints / SSRF-style abuse

- **Risk:** Offer `endpoint` points at internal resources; buyers fetch unsafely.
- **Implemented:** Indexer never fetches endpoints; board admission requires HTTPS.
- **Remaining:** Buyer clients should restrict schemes/hosts and not follow arbitrary redirects with secrets.

### Denial of service (API / DB)

- **Risk:** Overwhelm loopback or public API, fill SQLite, stall sync.
- **Partial:** Default listen `127.0.0.1:8080`; no sync-readiness on `/healthz` (liveness only).
- **Remaining:** Auth/TLS for non-loopback deploys, connection limits, metrics, operational runbooks.

### Publication path abuse / mistakes

- **Risk:** Wrong destination, fee surprise, double broadcast after timeout.
- **Implemented:** Library-derived fee preview; explicit `Proceed? [y/N]` (or `--dry-run`); final extracted tx verified (board output, memo, Orchard-only, value, fee, txid) before broadcast; public receipt option without wallet data.
- **Residual:** Network timeout can leave broadcast status unknown — check txid before retry.

## Explicit non-protections (current)

- No escrow or payment-vs-delivery atomicity.
- No formal reputation beyond pubkey / board allowlist.
- Mock commerce provides **no** real privacy or settlement.
- No consensus-level spam resistance on `open` boards.
- No production board API authentication or TLS termination in-tree; local text TLS is development-only.

## Security checklist for contributors

- [ ] Never pass spending keys into agent/LLM prompts, tools, or traces.
- [ ] Verify offer signatures before trust.
- [ ] Enforce invoice and offer expiry before payment.
- [ ] Keep policy decisions outside the model.
- [ ] Log payment *decisions* without logging secrets.
- [ ] Treat public offer text/URLs as untrusted input.
- [ ] Pin board descriptor fingerprints through a trusted channel.

## Local operator UI boundary

Owner-only configuration and authentication remain in protected local processes.
The browser gets only filtered seller balance/synchronization/receiver data and
public unsigned publication previews. There is no sign/spend/broadcast route.
Cross-origin writes and unexpected Host headers are rejected; invoice tokens
stay in memory rather than localStorage or URLs. Draft offer strings remain
untrusted. A preview can sign an Ed25519 offer but never a Zcash transaction.

The paid service stores inputs privately in SQLite and cannot automatically
refund incorrect/late payments. Its bounded 1,000-transaction observer fails
closed on truncation or synchronization changes. Production needs durable
reorg/recovery policy, abuse/resource limits and privacy/retention controls.
A previously released result cannot be recalled after a later chain reorg;
subsequent retrievals can only withhold it. A mainnet invoice is not a buyer
transaction fee preview and must not be interpreted as permission to pay.

HTTP repository failures now return a generic error; detailed database errors
stay in operator logs. UI validates nested response shapes and does not claim
browser signature verification. Python wallet responses have bounded reads;
protected configuration/auth/database files require current-user ownership.
The text service caps stored invoices at 10,000 in addition to 100 active slots.
SQLite board history/rejections and public API concurrency remain unbounded over
long operation; production quotas/monitoring are still needed. Local TLS and
Origin/Host checks do not defend against a compromised local OS/user process.

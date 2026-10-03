# ZADP Protocol Specification (ZADP/1)

Status: **Draft for Phase 1 MVP**  
Protocol major version: `1`

This document defines message semantics for ZADP Discovery and the commerce objects used in the MVP vertical slice. It deliberately separates:

1. **JSON representation** — human-readable, used by REST and demos today.
2. **Canonical compact representation** — intended for Zcash shielded memos later (not production-implemented in Phase 1).

## Design constraints

- Zcash Orchard/Sapling **memo fields are size-limited** (commonly discussed as 512 bytes for the memo field; exact usable payload after any framing is an **unresolved integration detail** — see [zcash-integration-todos.md](./zcash-integration-todos.md)).
- Offers must be **signed independently** of the Zcash transaction that carries them.
- Indexers need only **read/view** capability to discover board-addressed messages; they must not require spending authority.
- No account registration: seller identity is a public key.

## Message envelope

Every ZADP discovery message includes:

| Field | Type | Description |
| --- | --- | --- |
| `zadp` | u16 | Protocol major version (`1`) |
| `type` | string | Message type discriminator |
| body fields | — | Type-specific payload |
| `signature` | string | Base64 ed25519 signature over canonical signing bytes |

### Message types (ZADP/1)

| Type | Phase 1 | Purpose |
| --- | --- | --- |
| `offer` | **Implemented** | Publish a new service/product offer |
| `offer_update` | Specified only | Replace mutable fields of an existing offer |
| `offer_revoke` | Specified only | Invalidate an offer before expiry |

Unknown `type` values MUST be rejected by conforming indexers.

## Offer (`type = "offer"`)

### JSON schema (conceptual)

```json
{
  "zadp": 1,
  "type": "offer",
  "id": "01JABCDEF...",
  "seller": "ed25519:BASE64_PUBKEY",
  "capability": "llm.inference",
  "title": "Qwen inference",
  "description": "Mock LLM inference for demo",
  "price": {
    "amount": "0.01",
    "currency": "ZEC"
  },
  "endpoint": "http://127.0.0.1:8080",
  "expires_at": "2026-10-04T12:00:00Z",
  "nonce": "BASE64_16_BYTES",
  "signature": "BASE64_64_BYTES"
}
```

### Field semantics

| Field | Rules |
| --- | --- |
| `id` | Unique offer identifier chosen by seller (ULID recommended). Immutable. |
| `seller` | Seller identity public key. Format: `ed25519:` + standard Base64 of 32-byte raw public key. |
| `capability` | Capability taxonomy string (e.g. `llm.inference`). Used for discovery filters. |
| `title` | Short human-readable title. |
| `description` | Optional longer text. May be truncated or omitted in compact memo form. |
| `price.amount` | Decimal string in ZEC (not floating binary). Positive. |
| `price.currency` | Must be `"ZEC"` in ZADP/1. |
| `endpoint` | Absolute URL of seller commerce HTTP base. |
| `expires_at` | RFC 3339 UTC timestamp. Indexers MUST reject expired offers. |
| `nonce` | 16+ random bytes (Base64). Binds the signature; mitigates naive replay of identical bodies. |
| `signature` | ed25519 signature (Base64) over **canonical signing bytes** (below). |

### Validation rules (indexer)

1. `zadp == 1` and `type == "offer"`.
2. All required fields present and well-typed.
3. `price.currency == "ZEC"` and `amount` parses as positive decimal.
4. `expires_at` is in the future at validation time.
5. `seller` parses as ed25519 public key.
6. Signature verifies over canonical signing bytes.
7. Duplicate `id` with differing payload → reject/update policy: Phase 1 **rejects** conflicting redefinition; first valid wins. (`offer_update` will supersede this later.)

## Canonical signing bytes

Signatures MUST cover a deterministic byte string independent of JSON key order and insignificant whitespace.

**Phase 1 algorithm (`zadp1-sign-v1`):**

1. Take the offer **without** the `signature` field.
2. Serialize to **canonical CBOR** (RFC 8949 definite lengths; map keys sorted by bytewise encoded key; no tags unless specified).
3. Prepend ASCII prefix `ZADP1\0offer\0` (bytes).
4. Sign the resulting byte string with ed25519.

JSON is **not** the signing serialization. Implementers MAY display JSON; they MUST verify against CBOR canonical bytes.

Reference field order for CBOR maps (bytewise by encoded CBOR key):
`id`, `type`, `zadp`, `nonce`, `price`, `title`, `seller`, `endpoint`, `capability`, `expires_at`, `description`
(`price` is a map with keys `amount`, `currency`).

Optional `description` omitted entirely when empty.

## Compact memo representation (planned, not production)

Intended future path for shielded memo payloads:

1. Encode the unsigned offer body as canonical CBOR (`zadp1-cbor-v1`).
2. Append raw 64-byte ed25519 signature (not Base64) outside or inside a thin framing header.
3. Optionally strip/compress human fields (`description`) to meet memo budget.

**Phase 1 status:** the CBOR canonicalization used for signing is the **design anchor** for memo encoding. Full memo framing, board addressing, and compression are **not** implemented. Mock transport carries JSON envelopes in-process.

### Size budget (working assumption)

| Component | Approx. size |
| --- | --- |
| CBOR body (short title, no description) | ~180–280 bytes |
| Raw signature | 64 bytes |
| Framing / version byte | ~4–16 bytes |
| **Target** | **≤ 512 bytes memo** |

If real usable memo capacity differs, protocol may introduce `offer_ref` (hash + URI) — marked unresolved.

## `offer_update` (specified, not implemented)

```json
{
  "zadp": 1,
  "type": "offer_update",
  "id": "<existing offer id>",
  "seller": "ed25519:...",
  "title": "...",
  "description": "...",
  "price": { "amount": "0.02", "currency": "ZEC" },
  "endpoint": "https://...",
  "expires_at": "...",
  "nonce": "...",
  "signature": "..."
}
```

Rules (future):

- Must be signed by the same `seller` key as the original offer.
- Cannot change `id` or `seller`.
- Indexer replaces mutable fields if signature and expiry valid.

## `offer_revoke` (specified, not implemented)

```json
{
  "zadp": 1,
  "type": "offer_revoke",
  "id": "<existing offer id>",
  "seller": "ed25519:...",
  "reason": "optional",
  "nonce": "...",
  "signature": "..."
}
```

Rules (future): signed by original seller; indexer marks offer `REVOKED`.

## Commerce objects (off-chain HTTP, MVP)

These are **not** Zcash memo messages in Phase 1. They are REST JSON between buyer and seller.

### Invoice request

`POST {endpoint}/v1/invoices`

```json
{
  "offer_id": "...",
  "buyer_ref": "optional-correlation-id"
}
```

### Invoice response

```json
{
  "invoice_id": "...",
  "offer_id": "...",
  "amount": "0.01",
  "currency": "ZEC",
  "pay_to": "mock:shielded:...",
  "memo_hint": { "order_id": "..." },
  "expires_at": "..."
}
```

### Payment observation / fulfillment

Seller watches `ZcashAdapter::observe_payments` for matching `invoice_id` / amount / payee. On match: mark `PAID`, execute service, return result to buyer (`POST` poll or sync demo path).

## Payment policy decisions

The payment controller (not the LLM) evaluates invoices:

| Decision | Meaning |
| --- | --- |
| `ALLOW` | Create payment via adapter |
| `DENY` | Reject |
| `REQUIRE_APPROVAL` | Human gate required before paying |

Agent-facing API surface is limited to `request_payment(invoice)` semantics — never wallet secrets.

## Board addressing (conceptual)

Sellers publish messages “to the ZADP board.” Exact on-chain addressing (unified address, memo convention, UA receivers) is **unresolved** for real Zcash and mocked as a shared in-memory topic in Phase 1.

## Versioning

- Breaking envelope changes increment `zadp`.
- Additive optional JSON fields may appear within ZADP/1 if unknown-field-tolerant; Phase 1 indexers MAY ignore unknown fields after signature verification over known canonical set only (unknown fields MUST NOT be in the signed CBOR set unless version bump).

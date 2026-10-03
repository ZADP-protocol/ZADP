# ZADP Protocol Specification (ZADP/1)

Status: **Draft** — matches the current reference implementation
Protocol major version: `1` (`zadp` field / `PROTOCOL_VERSION`)

This document defines discovery message semantics, signing, memo wire format, board binding, and the **mock** commerce objects used by the local demo. It separates:

1. **JSON representation** — human-readable REST / files / examples.
2. **Canonical signing representation** — deterministic CBOR **map** + domain prefix (`zadp1-sign-v1`).
3. **Memo wire representation** — fixed 512-byte ZIP 302 framing + CBOR **tuple** (implemented for `offer`).

Live mainnet publish → scan → index → API acceptance is **not** claimed complete by this document.

## Design constraints

- Zcash shielded **memo fields are 512 bytes** (ZIP 302 arbitrary-data path used here).
- Offers are **signed independently** of the Zcash transaction that carries them.
- Indexers need only the board’s **incoming viewing** capability; they must not require spending authority.
- No account registration: seller identity is an Ed25519 public key.
- Open publication is subject to each board’s **admission policy**.

## Message envelope (logical / JSON)

Every ZADP discovery message includes:

| Field | Type | Description |
| --- | --- | --- |
| `zadp` | u16 | Protocol major version (`1`) |
| `type` | string | Message type discriminator |
| body fields | — | Type-specific payload |
| `signature` | string | Base64 Ed25519 signature over canonical signing bytes |

### Message types (ZADP/1)

| Type | Status | Purpose |
| --- | --- | --- |
| `offer` | **Implemented** (JSON, signing, memo wire, indexer) | Publish a new service/product offer |
| `offer_update` | Specified only | Replace mutable fields of an existing offer |
| `offer_revoke` | Specified only | Invalidate an offer before expiry |

Unknown `type` values MUST be rejected. The memo encoder accepts only `type == "offer"`.

## Offer (`type = "offer"`)

### JSON schema (conceptual)

```json
{
  "zadp": 1,
  "type": "offer",
  "id": "01JABCDEF...",
  "seller": "ed25519:BASE64_PUBKEY",
  "capability": "llm.inference",
  "title": "Example inference",
  "description": "optional",
  "price": {
    "amount": "0.01",
    "currency": "ZEC"
  },
  "endpoint": "https://seller.example/zadp",
  "expires_at": "2026-10-04T12:00:00Z",
  "nonce": "BASE64_16_TO_32_BYTES",
  "board_id": "example-board",
  "issued_at": "2026-10-03T12:00:00Z",
  "signature": "BASE64_64_BYTES"
}
```

`board_id` and `issued_at` are **required for mainnet memo publication** (`bind_offer` / `encode_offer_memo`). Legacy mock paths may omit them in in-process JSON only.

### Field semantics

| Field | Rules |
| --- | --- |
| `id` | Unique offer identifier (ULID recommended). Immutable. Memo: 1..=64 UTF-8 bytes. |
| `seller` | `ed25519:` + standard Base64 of 32-byte raw public key. Verification uses Ed25519 `verify_strict`. |
| `capability` | Taxonomy string for discovery filters. Memo: 1..=64 UTF-8 bytes. |
| `title` | Short title. Memo: 1..=128 UTF-8 bytes. |
| `description` | Optional. Omitted from canonical CBOR when empty. |
| `price.amount` | Positive decimal string in ZEC (not binary float). Memo encodes integer **zatoshis** (exact 8 decimals; 1..=2.1e15 zat). |
| `price.currency` | Must be `"ZEC"` in ZADP/1. |
| `endpoint` | Absolute seller commerce URL. Board admission requires **HTTPS**. Mock demo may use `http://127.0.0.1`. |
| `expires_at` | RFC 3339; memo requires whole seconds. Indexers reject expired offers at API list time. |
| `issued_at` | RFC 3339 whole seconds for memo; required when board-bound. |
| `board_id` | Must match descriptor; descriptor IDs are lowercase `[a-z0-9-]{1..64}`. |
| `nonce` | Random bytes, Base64 in JSON; memo: 16..=32 raw bytes after decode. |
| `signature` | Base64 of 64 raw Ed25519 signature bytes. |

### Validation (`verify_offer`)

1. `zadp == 1` and `type == "offer"`.
2. Required fields present and well-typed (non-empty id/capability/title/endpoint/nonce).
3. `price.currency == "ZEC"` and amount positive.
4. `expires_at` is in the future at validation time.
5. `seller` parses as Ed25519 public key.
6. Signature verifies over canonical signing bytes.

HTTPS and board policy checks are applied at **board admission**, not inside bare `verify_offer`.

### Indexer admission / replay (current)

After decode + `verify_offer`, the board-bound indexer additionally enforces descriptor policy (TTL, clock skew vs **block time**, min listing value, authorized allowlist, active/rate limits), first-wins on seller/`id` and seller/`nonce`, and output uniqueness. Duplicate outputs/blocks are idempotent; reorgs remove orphan admissions. API listing filters `expires > now()` and reports confirmation state vs `confirmed_after_blocks`.

Conflicting redefinition of the same offer id by the same seller is rejected (first valid wins). `offer_update` will supersede this later.

## Canonical signing bytes (`zadp1-sign-v1`)

Signatures MUST cover a deterministic byte string independent of JSON key order and insignificant whitespace.

1. Take the offer **without** the `signature` field.
2. Serialize to **canonical CBOR map** (RFC 8949 definite lengths; map keys sorted by bytewise encoded key).
3. Prepend ASCII prefix `ZADP1\0offer\0`.
4. Sign with Ed25519.

JSON is **not** the signing serialization.

Fields included when present (keys sorted by encoded CBOR key bytes at runtime):
`board_id`, `capability`, `description` (if non-empty), `endpoint`, `expires_at`, `id`, `issued_at`, `nonce`, `price` (`amount`, `currency`), `seller`, `title`, `type`, `zadp`.

Price in the signed map uses the **decimal string** amount (not zatoshis).

## Compact memo representation (implemented for `offer`)

Transport encoding for shielded memos:

```
0xf5 || "ZADP" || wire_version(1) || payload_length(u16 big-endian) || CBOR payload || zero padding
```

| Constant | Value |
| --- | --- |
| Magic | `0xf5` + ASCII `ZADP` (5 bytes) |
| Wire version | `1` |
| Memo size | exactly **512** bytes |
| Max CBOR payload | **504** bytes |
| Padding | trailing bytes MUST be zero |

Non-matching magic → ignore (not a ZADP memo). Wrong length/version/padding/payload size → reject.

### Wire CBOR tuple (transport only)

```
(
  zadp: u16,
  board_id: text,
  offer_id: text,
  seller_public_key: bstr(32),
  capability: text,
  title: text,
  description: text | null,
  price_zat: u64,
  endpoint: text,
  issued_unix_seconds: i64,
  expires_unix_seconds: i64,
  nonce: bstr(16..=32),
  signature: bstr(64)
)
```

Wire `type` and ZEC currency are **implicit**. This tuple is **not** what is signed. Signatures cover the canonical typed OFFER map (including `board_id` / `issued_at` when bound). Encode path re-binds/normalizes and round-trips before accepting the memo.

**TODO / discrepancy note:** fractional zatoshis and subsecond timestamps cannot be memo-encoded; amounts must normalize to 8 decimal places and whole-second times.

## Board descriptor and binding

Portable public board viewing uses a signed `BoardDescriptor`:

```json
{
  "board": {
    "zadp": 1,
    "board_id": "example-board",
    "name": "...",
    "network": "mainnet",
    "address": "<Orchard-only unified address>",
    "incoming_viewing_key": "<UIVK>",
    "birthday_height": 1687104,
    "policy": { "...": "..." },
    "operator_key": "ed25519:..."
  },
  "signature": "..."
}
```

The example birthday is the mainnet NU5 activation height; a newly provisioned board should use its actual birthday height.

Operator signature domain: `ZADP1\0board\0` || canonical CBOR of `BoardBody`.
Board fingerprint: SHA-256 hex of canonical CBOR of the **entire** descriptor (body + signature); SQLite pins this fingerprint.

### Policy fields (current defaults)

| Field | Default (reference) |
| --- | --- |
| `admission` | `open` or `authorized` |
| `authorized_sellers` | allowlist when `authorized` |
| `max_offer_ttl_seconds` | 604800 |
| `min_listing_value_zat` | 1000 |
| `max_active_offers_per_seller` | 20 |
| `max_publications_per_seller_per_window` | 20 |
| `rate_window_seconds` | 86400 |
| `confirmed_after_blocks` | 3 |
| `max_clock_skew_seconds` | 300 |

Binding requires `offer.board_id == descriptor.board_id`, issued time, TTL/skew checks, listing value, allowlist when authorized, and HTTPS endpoint. See [board-descriptor.md](./board-descriptor.md).

**Note:** a new seller key can bypass per-seller quotas on an `open` board; listing value or `authorized` mode strengthens admission.

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

Future rules: same seller key; cannot change `id` or `seller`; indexer replaces mutable fields if valid. **Not accepted as wire memos today.**

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

Future rules: signed by original seller; indexer marks `REVOKED`. **Not accepted as wire memos today.**

## Commerce objects (mock HTTP only)

These are **not** Zcash memo messages. They are REST JSON used by `zadp-demo` / full `router()` — **not** by mainnet `board_router`.

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

Seller watches `ZcashAdapter::observe_payments` (mock). On match: mark `PAID`, execute service, return via demo fulfill path. **No production purchasing protocol in this milestone.**

## Payment policy decisions

| Decision | Meaning |
| --- | --- |
| `ALLOW` | Create payment via adapter |
| `DENY` | Reject |
| `REQUIRE_APPROVAL` | Human gate required before paying |

Agent-facing surface is `request_payment(invoice)` semantics — never wallet secrets.

## Discovery API (mainnet board daemon)

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `/healthz` | Liveness |
| `GET` | `/offers?capability=` | Indexed offers + chain metadata |

There is **no** `GET /offers/{id}` on `board_router`.

## Versioning

- Breaking envelope / wire changes increment `zadp` and/or `wire_version`.
- Unknown fields MUST NOT enter the signed canonical CBOR set without a version bump.
- Additive optional JSON display fields may appear if verification still uses the known canonical set only.

## Implementation status summary

| Item | Status |
| --- | --- |
| OFFER signing + board-bound memo wire | Implemented |
| Board descriptor + admission | Implemented |
| CompactTxStreamer scan + SQLite index | Implemented |
| Live mainnet acceptance | **Pending** |
| Real shielded settlement | **Not implemented** (mock only) |
| OFFER_UPDATE / OFFER_REVOKE wire | **Not implemented** |

# ZADP Protocol Specification (Offer v1/v2)

Status: **Draft** — matches the current reference implementation
Offer/wire major versions: `1` and `2`. BoardDescriptor remains `1`; the legacy `PROTOCOL_VERSION` alias remains Offer v1.

This document defines discovery message semantics, signing, memo wire format, board binding, and the **mock** commerce objects used by the local demo. It separates:

1. **JSON representation** — human-readable REST / files / examples.
2. **Canonical signing representation** — deterministic CBOR **map** + version-specific domain prefix.
3. **Memo wire representation** — fixed 512-byte ZIP 302 framing + CBOR **tuple** (implemented for `offer`).

Live mainnet publish → scan → index → API acceptance was verified on 2026-10-05; see [public evidence](./mainnet-acceptance.md).

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

### Message types (legacy v1 and Offer v2)

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
| `price.currency` | V1 legacy price or v2 commerce price must be `"ZEC"`; absent commerce has no price. |
| `endpoint` | V1 legacy field; v2 uses optional `commerce.endpoint`. Complete commerce requires **HTTPS**; no endpoint for discovery-only v2. Legacy mock may use loopback HTTP. |
| `expires_at` | RFC 3339; memo requires whole seconds. Indexers reject expired offers at API list time. |
| `issued_at` | RFC 3339 whole seconds for memo; required when board-bound. |
| `board_id` | Must match descriptor; descriptor IDs are lowercase `[a-z0-9-]{1..64}`. |
| `nonce` | Random bytes, Base64 in JSON; memo: 16..=32 raw bytes after decode. |
| `signature` | Base64 of 64 raw Ed25519 signature bytes. |

### Validation (`verify_offer`)

1. `zadp` is 1 or 2 and `type == "offer"`; geo is permitted only in v2 and must be complete and within bounds.
2. Required fields present and well-typed (non-empty id/capability/title/nonce; v1 requires legacy endpoint/price, v2 allows absent commerce).
3. When commerce exists, price currency is ZEC and amount positive; discovery-only v2 needs neither price nor endpoint.
4. `expires_at` is in the future at validation time.
5. `seller` parses as Ed25519 public key.
6. Signature verifies over canonical signing bytes.

HTTPS and board policy checks are applied at **board admission**, not inside bare `verify_offer`.

### Indexer admission / replay (current)

After decode + `verify_offer`, the board-bound indexer additionally enforces descriptor policy (TTL, clock skew vs **block time**, min listing value, authorized allowlist, active/rate limits), first-wins on seller/`id` and seller/`nonce`, and output uniqueness. Duplicate outputs/blocks are idempotent; reorgs remove orphan admissions. API listing filters `expires > now()` and reports confirmation state vs `confirmed_after_blocks`.

Conflicting redefinition of the same offer id by the same seller is rejected (first valid wins). `offer_update` will supersede this later.

## Canonical signing bytes (Offer v1/v2)

Signatures MUST cover a deterministic byte string independent of JSON key order and insignificant whitespace.

1. Take the offer **without** the `signature` field.
2. Serialize to **canonical CBOR map** (RFC 8949 definite lengths; map keys sorted by bytewise encoded key).
3. Prepend ASCII prefix `ZADP1\0offer\0` for Offer v1, or `ZADP2\0offer\0` for Offer v2.
4. Sign with Ed25519.

JSON is **not** the signing serialization.

Fields included when present (keys sorted by encoded CBOR key bytes at runtime):
`board_id`, `capability`, `description` (if non-empty), `endpoint`, `expires_at`, `id`, `issued_at`, `nonce`, `price` (`amount`, `currency`), `seller`, `title`, `type`, `zadp`.

Price in the signed map uses the **decimal string** amount (not zatoshis).

## Compact memo representation (implemented for `offer`)

Transport encoding for shielded memos:

```
0xf5 || "ZADP" || wire_version(1 or 2) || payload_length(u16 big-endian) || CBOR payload || zero padding
```

| Constant | Value |
| --- | --- |
| Magic | `0xf5` + ASCII `ZADP` (5 bytes) |
| Wire version | `1` (13-field legacy tuple) or `2` (13-field optional-commerce tuple) |
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

Wire `type` and ZEC currency are **implicit**. This tuple is **not** what is signed. Signatures cover the canonical typed OFFER map (including `board_id` / `issued_at` when bound). The publication caller binds/normalizes before signing; the encoder round-trips and compares signed fields before accepting the memo.

**Enforced transport normalization:** fractional zatoshis and subsecond timestamps are rejected. Amounts must represent an exact integer zatoshi value (at most eight meaningful decimals) and whole-second times. `bind_offer` normalizes the decimal before signing; the memo encoder does not silently normalize signed fields. Bare legacy JSON validation is less restrictive than memo transport.

## Offer v2 geographic extension

Offer v1 JSON, signing preimage and memo bytes remain unchanged; see the captured
[golden fixture](../crates/zadp-protocol/tests/fixtures/offer-v1-golden.json).
Separate constants are `OFFER_VERSION_V1=1`, `OFFER_VERSION_V2=2`,
`WIRE_VERSION_V1=1`, `WIRE_VERSION_V2=2`, `BOARD_DESCRIPTOR_VERSION=1`.
`PROTOCOL_VERSION` remains a legacy alias for Offer v1; it is not a board bump.

Offer v2 adds optional `geo` and optional `commerce` to the typed JSON model. Discovery requires no commerce endpoint; see [exact model and wire](optional-commerce.md). Geo example:

```json
"geo": { "lat_e5": 3777493, "lon_e5": -12241942, "radius_m": 50000 }
```

Latitude/longitude are signed integer degrees × 100000, with ranges
−9000000..9000000 and −18000000..18000000. Public offer radius is 1..500000 meters.
Absent geo is valid; present geo requires exactly all three integer fields.
Partial, floating, unknown geo fields and invalid ranges are rejected. This is
an optional, self-declared **offer/service area**, not an address or proof of a
physical position. Holders of the public board UIVK can read it.

**Protocol precision versus reference UI policy:** e5 is the deterministic signed
integer representation, with all existing integer values and ranges allowed.
The reference seller UI rounds only browser-acquired coordinates to three decimal
degrees locally, then converts the reviewable form values to e5. For example:
`37.774929 → 37.775 → 3777500 lat_e5`. Manual coordinates retain seller-selected
precision up to e5; they are not silently rounded to three decimals. This does
not constrain wire values to multiples of 100, change signing/version/framing,
or require other clients to adopt the same precision. A conforming ZADP client
**MUST NOT assume that all geo offers use 3-decimal coordinates**. Three decimal
degrees represent approximately 100-meter-scale precision, with ground distance
varying by latitude, especially for longitude; rounding is not anonymization.

V2 canonical signing uses the existing map design with domain `ZADP2\0offer\0`.
When present, the `geo` map entry is a compact CBOR array
`[lat_e5, lon_e5, radius_m]`, sorted with other canonical keys. When absent it is
omitted. Changing any geo value invalidates the signature. The map and the
transport tuple remain distinct signing/transport representations.

Wire v2 uses the corrected 13-element tuple documented in
[optional commerce](optional-commerce.md): common discovery fields, nullable
`[endpoint, price_zat]` commerce, times/nonce/signature, nullable geo triple.
The old experimental 14-element v2 tuple was replaced before first recorded
mainnet v2 publication. Canonical v2 always signs `commerce` as null or a map;
legacy `price` and `endpoint` keys apply only to v1. Adding/removing commerce
invalidates the signature. Framing and the 504-byte payload budget are unchanged.

A BoardDescriptor v1 admits both versions without changing its domain, address,
UIVK, fingerprint or policy. Legacy readers continue reading v1 but reject v2;
upgrade readers before publishing geographic offers. Persistence stores full
OFFER JSON, so no SQLite schema migration is introduced. An older indexer that
already rejected v2 must deliberately reconstruct in a separate database from
the signed birthday after upgrading; advancing its cursor alone cannot recover
previously rejected offers. Do not overwrite a running index.

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

Binding requires `offer.board_id == descriptor.board_id`, issued time, TTL/skew checks, listing value, allowlist when authorized, and HTTPS endpoint when commerce is present. See [board-descriptor.md](./board-descriptor.md).

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

There is **no** `GET /offers/{id}` on `board_router`. Both versions appear in
`GET /offers`; geo uses the same integer e5 fields as typed JSON. Coordinates
are not accepted as query fields. There is no `POST /offers/search`. API tracing
records method only, not request URI/query/body. Buyer matching is client-only.

## Versioning

- Breaking envelope / wire changes increment `zadp` and/or `wire_version`.
- Unknown fields MUST NOT enter the signed canonical CBOR set without a version bump.
- Additive optional JSON display fields may appear if verification still uses the known canonical set only.

## Implementation status summary

| Item | Status |
| --- | --- |
| OFFER v1/v2 signing + board-bound memo wire | Implemented; signed optional geo in v2 |
| Board descriptor + admission | Implemented |
| CompactTxStreamer scan + SQLite index | Implemented |
| Live mainnet discovery acceptance | **Verified 2026-10-05** — mined OFFER and two independent indices; [public evidence](./mainnet-acceptance.md) |
| Real shielded settlement | Buyer proposal/payment acceptance pending; experimental seller observation/gate implemented and synthetically tested |
| OFFER_UPDATE / OFFER_REVOKE wire | **Not implemented** |

## Experimental service integration boundary

The Python [paid text service](../services/text/README.md) uses exact integer
zatoshis and a text invoice memo, not the Rust mock invoice schema above.
See [paid-service design](commerce.md). Neither schema is a standardized
ZADP/1 purchase message; no purchase wire semantics changed in this stabilization.

Compatibility details: OFFER JSON ignores unknown properties during typed
Serde decoding; they do not enter the known-field signature preimage. A board
descriptor instead rejects unknown properties. Consumers must not assign trusted
meaning to unsigned JSON extensions. Empty descriptions are omitted from the
signature map but may be represented as empty text or null in the memo. The wire
decoder accepts CBOR forms supported by ciborium and validates tuple types,
bounds and complete consumption; it does not independently enforce shortest
CBOR integer encoding. Emitted memos use the current definite-length serializer.
Tightening decoder canonicality or JSON unknown-field rules needs an explicit
compatibility review/version decision rather than a silent overnight change.

A signature authenticates the reconstructed canonical typed OFFER, not transport
bytes or service quality. The bytewise encoded-key ordering and timestamp/decimal
string conventions used by this implementation are normative for both offer versions.

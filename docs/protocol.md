# ZADP Protocol 1.0

Status: **Canonical specification for Protocol 1.0**. Implementation remains
alpha software; this version designation is not a security-audit claim.

Protocol release `1.0` has one canonical Offer model, identified on the wire and
in signed JSON by **3**. The existing wire ID is intentionally retained:
`zadp: 3`, wire byte `3`, signing prefix `ZADP3\0offer\0`.
Release numbering is separate from these stable encoding identifiers.
BoardDescriptor remains version **1**, with its existing signing, address/UIVK
binding, fingerprint and policy unchanged.

Offer/wire 1 and 2 are **pre-release legacy**. Readers retain strict, unchanged
historical decoding and signature verification; existing signed mainnet evidence
is not migrated or reinterpreted. Supported new Offer construction/publication
accepts only model/wire 3. The [legacy record](pre-release-legacy.md) is
non-normative historical documentation.

## Boundaries

ZADP publishes signed discovery metadata through Zcash shielded 512-byte memo
outputs to a signed board. Indexers need incoming-only board viewing capability,
not spending authority. Seller Ed25519 identity is separate from the Zcash payer.
Board admission and hosted visibility are independent from protocol validity.
Discovery infrastructure does not receive buyer GPS, commerce orders, payments
or service results. A signature proves identity attribution, not service quality.

## Canonical Offer model

```json
{
  "zadp": 3,
  "type": "offer",
  "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "seller": "ed25519:BASE64_PUBKEY",
  "capability": "local.service",
  "title": "Bike repair — San Francisco area",
  "description": "Illustrative service-area example",
  "price": {"amount": "0.02", "currency": "ZEC"},
  "expires_at": "2030-01-31T00:00:00Z",
  "issued_at": "2030-01-01T00:00:00Z",
  "board_id": "local-services",
  "geo": {"lat_e5": 3777500, "lon_e5": -12241900, "radius_m": 25000},
  "nonce": "BASE64_16_TO_32_BYTES",
  "signature": "BASE64_64_BYTES"
}
```

This conceptual example is a manual listing, not a real provider or signed
publication. The exact typed model and mode examples are specified in
[commercial modes](commercial-modes-v3.md).

- Discovery: `price` absent and `commerce` absent.
- Manual settlement: positive fixed ZEC `price`, `commerce` absent.
- Automated: positive fixed ZEC `price` and `commerce: {endpoint}`.
- Endpoint without price is invalid. Currency is ZEC only. No contact protocol,
  static payment address, fiat/unit pricing or negotiation is encoded.

JSON may omit absent optional fields or supply null. Canonical CBOR signs price
and commerce as explicit maps or null. No v1 flat endpoint or v2 nested price is
accepted in model 3. API `purchase_mode` is derived metadata, not a signed field.

## Field and validation rules

Identity is `ed25519:` plus standard Base64 of a 32-byte public key; signature is
Base64 of 64 bytes and uses strict Ed25519 verification. IDs are immutable,
ULID recommended; memo permits 1–64 UTF-8 bytes. Capability permits 1–64 bytes,
title 1–128 bytes. Optional non-empty description is signed and shares the memo
budget. Required textual fields and board IDs retain existing validation rules.

Price is a positive normalized decimal string, exactly representable in integer
zatoshis, at most eight meaningful decimals and at most 21 million ZEC. No
floating-point money arithmetic is used. This service price is separate from
board listing value, hosted publication price and Zcash network fee.

Automated endpoint must be credential-free HTTPS. Hosted Buy additionally needs
a supported seller order capability, host approval and a valid
[endpoint ownership proof](endpoint-ownership.md). Manual/discovery listings
require no endpoint proof and create no automated invoice or order.

Expiry and issue time are RFC 3339 UTC; transport requires whole seconds. Expiry
must be after issue time and obey the signed board TTL. Active discovery uses
`expires_at > now`, excluding the exact boundary. Existing signed Offers are
never silently extended. Board ID/issue time are required for memo publication.
Nonce is 16–32 raw bytes encoded as Base64 in JSON.

Optional geo is the complete signed integer triple
`{lat_e5, lon_e5, radius_m}`. Latitude range is -9000000..9000000, longitude
-18000000..18000000 and radius 1..500000 meters. e5 representation is unchanged.
It describes a public self-declared service area, not a verified seller position.
Reference seller GPS is rounded to three decimal degrees before review; manual
coordinates retain explicit e5 precision. Buyer GPS remains browser-memory only,
never in API requests, URLs, logs or persistent storage.

## Canonical signing

1. Build the typed Offer map without `signature`.
2. Encode deterministic definite-length CBOR, map keys ordered bytewise by their
   encoded CBOR key, with normalized decimal price string and RFC 3339 times.
3. Prefix `ZADP3\0offer\0` (ASCII with two NUL bytes).
4. Sign using the seller Ed25519 identity.

The signed map includes identity, ID, type, model ID 3, capability, title,
optional non-empty description, board/issue binding, expiry, nonce, explicit
price-or-null and commerce-or-null, and geo when present. Price map is
`{amount,currency}`; commerce map is `{endpoint}`. Changing price, endpoint,
endpoint absence or geo invalidates the signature. JSON ordering is not signing
ordering; wire tuples are not the signing preimage.

## Wire 3 memo

Framing is exactly:

```text
0xf5 || "ZADP" || 3 || u16_be(payload_length) || CBOR || zero padding
```

Total memo is exactly 512 bytes, CBOR payload at most 504 bytes. Non-matching
magic is ignored; invalid length, padding, unsupported version or tuple shape
is rejected. Model ID must match framing wire ID.

Canonical wire tuple has 14 elements:

```text
[3, board_id, offer_id, seller_key_bytes32, capability, title,
 description_or_null, price_zat_u64_or_null, endpoint_text_or_null,
 issued_unix_seconds, expiry_unix_seconds, nonce_bytes16_to32,
 signature_bytes64, geo_or_null]
```

Geo is null or `[lat_e5_i32,lon_e5_i32,radius_m_u32]`. Type `offer` and ZEC
currency are implicit. Encoding checks the exact 504-byte budget and verifies
roundtrip preservation of signed fields; it does not silently normalize an
already signed Offer. Legacy encoder paths exist only to roundtrip historical
records; the publication gate rejects them.

## New publication versus historical verification

`PROTOCOL_RELEASE = "1.0"`, `PROTOCOL_VERSION = OFFER_VERSION_V3 = 3`.
All OfferDraft constructors default to model 3; signing an explicit 1/2 draft
is rejected. CLI accepts only `--offer-version 3`. `verify_offer_for_publication`
is used before wallet proposal/publication and again at exact approval.
Read/index APIs use `verify_offer`, which still supports legacy 1/2 signatures
without changing their domain separators or wire bytes. Generic Ed25519 signing
primitives are not a publication authorization mechanism.

Publication requires the signed board binding/admission checks plus the exact
human-reviewed wallet-generated proposal. A quote, UI action, passing test or
Protocol 1.0 label does not authorize a mainnet broadcast.

## Indexer, replay and history

Existing board admission enforces TTL, clock skew against block time, minimum
listing value, authorized sellers and configured active/rate limits. Seller/ID
and seller/nonce remain first-wins; outputs/blocks are idempotent and reorgs
remove orphan admissions. Legacy records keep original signatures, version and
JSON; no database rewrite, descriptor revision or board rebuild is needed for
this release. Expired Offers may remain in canonical history but never appear
as active inventory. Hosted moderation/distribution remains independent.

## Commerce and future messages

Automated commerce uses the existing direct seller flow: order, ZIP-321 request,
separate buyer wallet approval, observed payment, confirmation, execution and
result. Endpoint proof binds the same seller identity to the HTTPS origin
without installing seller identity private keys on the public seller server.
Manual settlement is outside this automated flow. See [commerce](commerce.md)
and [process boundaries](commerce-process-boundaries.md).

`offer_update` and `offer_revoke` are not implemented message types; Protocol 1.0
currently supports `offer` only. Renewal creates a new signed Offer. Do not
infer escrow, refund, reputation, messaging or generic pricing extensions.

## Evidence and compatibility

[Mainnet acceptance](mainnet-acceptance.md) preserves the real pre-release wire
1/2 publication evidence, including the first geographic wire 2 Offer. It is
not relabelled as a Protocol 1.0 wire 3 publication. A real wire 3 mainnet
publication remains a separately approved future action.

Tests cover frozen v1/v2 golden/mainnet bytes, legacy read rejection at the new
publication boundary, all three model 3 modes, signatures, exact memo budgets,
independent encrypted-note reconstruction, persistent DB/API behavior, endpoint
ownership, expiry and browser-local matching.

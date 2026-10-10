# Optional commerce in Offer v2

**Pre-release legacy record:** wire 1/2 remain read/historical compatibility only. New Offers use [Protocol 1.0 / wire 3](protocol.md).

Historical v2 design record. V2 is now published and frozen; the current independent-price model and Web/CLI behavior are documented in [Offer v3 commercial modes](commercial-modes-v3.md). The pre-publication rationale below does not authorize reinterpreting published v2.

**Discovery is core. Commerce is optional. ZADP discovery does not require a commerce endpoint.**

This is a deliberate correction of the draft v2 model before its first recorded
mainnet publication. The private repository's live receipt is Offer v1; earlier
v2 evidence is offline. The old experimental 14-element v2 format is superseded,
not silently supported as a second interpretation. Rebuild disposable old v2
fixtures; upgrade independent readers before first live v2 publication.
BoardDescriptor v1, Offer/wire v1, their signatures and memo bytes are unchanged.

## Model and JSON/API

Core fields remain seller, id, capability, title, optional description, expiry,
nonce, board binding/issue time, optional public geo, and signature.

V1 retains its exact top-level `endpoint` and `price` JSON fields.
V2 has `commerce: Option<Commerce>`. Absent commerce is omitted from serialized
JSON; an explicit JSON `commerce: null` is accepted equivalently. No top-level
endpoint or price is emitted for v2. No price or endpoint is required when
commerce is absent. Partial commerce objects and legacy fields mixed into v2
are rejected. Commerce, when present, contains both fields:

```json
{"commerce":{"endpoint":"https://seller.example","price":{"amount":"0.01","currency":"ZEC"}}}
```

The endpoint must be valid credential-free HTTPS and price must be positive ZEC,
exactly representable in zatoshis and within the existing money bound. Board
publication value/fee are independent of this displayed service price. A
non-purchasable offer still pays the board listing value and transaction fee.

Discovery-only example: **Bike repair — San Francisco area**, self-declared
25 km area, description explicitly identifying the ZADP demo, no automated
commerce endpoint. Commerce example: **Text transformation service**, HTTPS
seller origin, positive ZEC price, direct seller order flow.

## Signed canonical representation

V1 preserves its exact existing canonical map and `ZADP1\0offer\0` prefix.
V2 uses `ZADP2\0offer\0`; the canonical map always contains `commerce`, either
CBOR null or a canonically ordered map containing `endpoint` and `price`.
The price map contains `amount` and `currency`. The geo triple, when present,
remains signed under `geo`. Every map is sorted by encoded key bytes.
Adding/removing commerce, modifying its endpoint or price, or modifying geo
invalidates the seller signature. An indexer cannot turn a discovery-only offer
into a purchasable offer. Transport encoding is distinct from signing encoding.

## Exact wire v2

Framing remains `0xf5ZADP`, wire byte 2, big-endian u16 payload length, CBOR,
zero padding to exactly 512 bytes; payload maximum 504 bytes.

The new v2 tuple has **13 elements** in this exact order:

```text
[2, board_id, offer_id, seller_key_bytes32, capability, title, description_or_null,
 commerce_or_null, issued_unix_seconds, expiry_unix_seconds,
 nonce_bytes16_to32, signature_bytes64, geo_or_null]
```

`commerce_or_null` is CBOR null or `[endpoint_text, price_zat_u64]`.
`geo_or_null` is CBOR null or `[lat_e5_i32, lon_e5_i32, radius_m_u32]`.
No fake endpoint or price is encoded for absent commerce. Version-specific tuple
shapes are strict. V1 keeps its original 13 fields, including separate price
and endpoint, with wire byte 1. The same arity does not make the formats
interchangeable; wire/offer versions and typed fields must match.

## UI, seller, indexer and commerce

My offer defaults to discovery-only. **Enable automated commerce** reveals and
requires endpoint/price; disabling it removes both from the draft. An optional
service area is independent. GPS is click-only; manually entered coordinates
are preserved. Buyer GPS remains runtime-memory-only and never enters a draft,
API request, commerce request, URL, log or persistent storage.

Buyer cards/details show **Discovery only** and no Buy button when commerce is
absent. Complete supported commerce retains the direct seller order flow and
network invariant. Nearby matches only service areas; commerce neither filters
nor improves ranking. API serialization uses the same versioned model and
SQLite stores the complete offer JSON without a new table/schema migration.
Existing v1 persistent data remains readable; experimental old v2 JSON with
flat commerce is incompatible and must be rebuilt in disposable fixtures.

The seller CLI accepts neither `--endpoint` nor `--price-zec` for discovery-only
publication, producing v2 even without geo. Supplying either flag requires the
other. Commerce without geo retains the v1 default; commerce with geo uses v2.
`--dry-run --preview-output` signs the public offer and builds an unsigned
wallet proposal only: no Zcash signing or broadcast. `--prepared-offer` reuses the exact signed public offer (ID, nonce,
issue/expiry and signature) saved from a reviewed preview; its content must match
the requested flags. Expired prepared offers are refused. Later publication requires
explicit approval of exact content, recipient and wallet-derived fee.

## Verification boundary

Regression tests cover exact v1 golden bytes; v2 null/complete commerce; partial
and invalid states; presence/endpoint/price signature tampering; memo budgeting
against the Rust encoder; encrypted notes reconstructed by two independent
persistent indexes and GET /offers; buyer UI and local GPS privacy; existing
commerce, restart/idempotency, payment matching and network checks.
Offline encrypted fixtures are not evidence of a live v2 mainnet publication.

## Endpoint ownership

Hosted automated buying requires a separate [offline-signed endpoint ownership proof](endpoint-ownership.md). Endpoint downtime disables commerce without invalidating signed discovery. Discovery-only Offers are unchanged.

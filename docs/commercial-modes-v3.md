# Protocol 1.0 canonical Offer (wire/model ID 3)

## Compatibility decision

Published v1 and v2 semantics are frozen. V1 has mandatory top-level price and
endpoint; v2 has optional nested `commerce: {endpoint, price}`. Neither can sign
a standalone price without changing its interpretation. Protocol 1.0 keeps the existing Offer v3 / wire v3 identifier and
therefore uses independent signed price and endpoint presence. Existing
v1/v2 JSON, signing bytes, wire bytes, IDs, board policy and geographic e5 fields
remain unchanged. Regression goldens include the actual first mainnet v2 geo
Offer `01M4H88HGMFHVH2VXA0AFBTM7E`, compared at its original issue time.

BoardDescriptor remains v1. Readers must support v3 before publishing v3; old
readers reject unsupported wire versions. No signed Offers are migrated. The
existing SQLite JSON storage holds each version as originally serialized and
requires no schema migration, descriptor replacement or database rebuild.

## Normalized typed model

`Offer.price: Option<Price>` is independent of
`Offer.commerce: Option<Commerce>`, where Commerce contains only `endpoint`.
Price is one fixed positive ZEC Decimal, representable in integer zatoshis (at
most eight meaningful decimal places), bounded by the existing money limit.
Version-specific serialization adapters retain legacy JSON shapes. This changes
the Rust source API for callers that accessed `commerce.price`; use
`offer.price` instead. It does not change their persisted v1/v2 representation.

`purchase_mode()` / API `purchase_mode` is derived, unsigned metadata:

- no price, no commerce: `discovery`
- price, no commerce: `manual`
- price and commerce: `automated`
- commerce without price: invalid

Examples below show the commercial fields of v3 API records, with the usual
signed identity, ID, expiry, board and other fields omitted for brevity:

```json
{"zadp":3,"purchase_mode":"discovery"}
```

```json
{"zadp":3,"price":{"amount":"0.02","currency":"ZEC"},"purchase_mode":"manual"}
```

```json
{"zadp":3,"price":{"amount":"0.01","currency":"ZEC"},"commerce":{"endpoint":"https://seller.example"},"purchase_mode":"automated"}
```

Absent optional fields are omitted in JSON; null is accepted. V2 rejects new
top-level price/endpoint fields, and v3 rejects legacy top-level endpoint or
nested commerce price. Legacy API price shapes remain unchanged; purchase_mode
is additive metadata. Browser adapters normalize all three versions.

## Canonical signing and wire

V3 uses `ZADP3\0offer\0` followed by the deterministic canonical CBOR map.
The map includes `price` (canonical `{amount,currency}` map or null) and
`commerce` (canonical `{endpoint}` map or null). Price decimal normalization,
key ordering and other fields follow existing canonical rules. The geographic
triple remains signed. Changing price, endpoint, endpoint absence or geo breaks
the signature. V1/v2 canonical encoders and their domain prefixes are preserved.

Framing remains `0xf5ZADP`, wire byte **3**, big-endian u16 payload length,
CBOR and zero padding to 512 bytes. Payload limit remains **504 bytes**.
Exact v3 tuple (14 elements):

```text
[3, board_id, offer_id, seller_key_bytes32, capability, title,
 description_or_null, price_zat_u64_or_null, endpoint_text_or_null,
 issued_unix_seconds, expiry_unix_seconds, nonce_bytes16_to32,
 signature_bytes64, geo_or_null]
```

Geo is null or `[lat_e5_i32,lon_e5_i32,radius_m_u32]`. Wire version and tuple
Offer version must agree. Compared with v2, discovery adds one null byte;
automated commerce removes the nested array header (one byte smaller). A manual
listing adds only the price integer size relative to v2 discovery. Tuple headers
remain one byte. Exact sizes still depend on text, times, nonce and integer
widths. Every mode, with and without geo, has an exact 504-byte boundary test;
the next byte is rejected. Browser budget estimates are checked against Rust.
For `local-services`, `Bike repair`, `local.service`, a 16-byte nonce and
30-day lifetime from 2030-01-01, payload sizes are 202/206/228 bytes for
discovery/manual/automated respectively (price 0.02 ZEC, endpoint
`https://seller.example`). The SF geo triple adds 13 bytes: 215/219/241.

## CLI and Web

CLI discovery needs neither price nor endpoint. `--price-zec 0.02` alone selects
v3 manual listing. `--price-zec 0.01 --endpoint https://seller.example` also uses model/wire 3.
`--offer-version 3` is optional; 1/2 are rejected for new signing/publication.
New Web drafts explicitly select model 3 for all modes. Wire 1/2 are pre-release legacy read-only compatibility. Endpoint without price is
rejected. Existing dry-run, prepared Offer, exact wallet proposal, expiry and
human publication approval requirements are unchanged.

The Web Selling mode selector shows neither commercial field for discovery,
only Service price (ZEC) for manual, and price plus endpoint for automated.
Service price remains separate from hosted publication price, board listing
value and Zcash network fee. Manual listings show `Manual settlement` and no
Buy action; contact method is not standardized yet. No invoice, recipient,
endpoint proof or `/orders` request is generated for manual listings.

Only automated commerce invokes the bounded endpoint proof flow and requires
verified ownership before hosted Buy. Manual API commerce status is
`not_required`; discovery retains legacy `discovery_only`. Endpoint outage
continues to disable commerce without invalidating signed discovery. Approval
and host distribution policy remain separate. Reference commerce requires both
price and endpoint; its invoice/order/payment/result and network checks are
unchanged. No buyer GPS behavior changes.

## Release boundary

This implementation is local/offline tested. It does not activate production
charging, publish v3, deploy code or change custody. Review and upgrade Core/API
and Web readers together before explicitly approved v3 publication. Manual
settlement is outside ZADP automated commerce; no messaging, escrow, static
payment addresses, automatic manual payments or renewal/revoke protocol is
introduced.

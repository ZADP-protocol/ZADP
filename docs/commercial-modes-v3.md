# Protocol 1.0 seller endpoint primitives (wire/model ID 3)

## Pre-release compatibility decision

The 2026-10-10 audit found Core and Web private, no Core GitHub releases or
release tags, and no wire-3 publication in the remote canonical publication
history (including expired rows). Recorded mainnet evidence is legacy wire 1/2.
Repository evidence does not establish any independent released wire-3 dependency.
This is a scoped observation, not a claim about unseen third-party code.

Baseline Core: `20d99520f15462c1526749d20758b97f20fb5231`.
Baseline Web: `230414a341d77464eff5ab4946aba2d83391e382`.
The unpublished wire-3 draft is cleaned up in place; no wire 4 is introduced.
Legacy wire 1/2 JSON, canonical preimages, signatures and memo bytes remain frozen.
Regression goldens include mainnet v2 Offer `01M4H88HGMFHVH2VXA0AFBTM7E`.
BoardDescriptor v1 and SQLite schema are unchanged. No historical Offer is rewritten.
Old unpublished price-without-endpoint wire-3 drafts must be revised and re-signed;
do not reuse their prepared publication quotes.

## Typed model and validation

`Offer.price: Option<Price>`, `Offer.commerce: Option<Commerce>` where Commerce
contains only `endpoint`, and `Offer.geo: Option<GeoArea>` are signed primitives.
There is no protocol PurchaseMode enum.

- No endpoint, no price: valid discovery announcement.
- Endpoint, no price: valid seller service announcement, without a fixed payment.
- Endpoint and price: valid fixed-price service candidate.
- Price without endpoint: invalid canonical wire 3.

Geo is optional for all three valid combinations. Price is one positive ZEC
Decimal, exactly representable in zatoshis, bounded by 21 million ZEC.
Endpoint is credential-free HTTPS. Legacy JSON adapters remain version-specific:
v1 flat price/endpoint; v2 optional nested commerce price/endpoint. Legacy endpoint
without price and price without endpoint remain invalid, exactly as before.
New signing/publication accepts only wire 3.

## Signing and memo

Prefix remains `ZADP3\0offer\0`. Canonical CBOR field names/order/layout remain
unchanged, with independent price map/null and endpoint map/null. Modifying price,
endpoint or geo invalidates the seller signature. The 14-element tuple remains:

```text
[3, board_id, offer_id, seller_key_bytes32, capability, title,
 description_or_null, price_zat_u64_or_null, endpoint_text_or_null,
 issued_unix_seconds, expiry_unix_seconds, nonce_bytes16_to32,
 signature_bytes64, geo_or_null]
```

Framing, 504-byte payload budget and 512-byte memo are unchanged. An endpoint-only
service costs exactly the endpoint text length/CBOR header instead of a null;
adding price replaces its null with a zatoshi integer. All valid combinations,
with/without geo, have exact 504/505-byte boundary tests and Rust/browser parity.

## Endpoint proof and hosted API

The existing offline/static proof and domain-separated preimage are unchanged.
A valid proof establishes only seller Ed25519 identity ↔ HTTPS origin control;
not KYC, business legitimacy, quality, fulfillment or order API support.
Both priced and no-price endpoints use the same bounded SSRF-safe origin proof
fetch. Endpoint absence causes no proof fetch.

API metadata is unsigned and host-derived:
`has_endpoint`, `has_fixed_price`, `purchase_available`, `endpoint_verification`.
Verification statuses: `not_applicable`, `verified`, `unverified`, `unavailable`.
Proof outage disables interaction/payment without modifying canonical Offers.
`purchase_mode`, `manual`, and `not_required` are no longer emitted.

```json
{"zadp":3,"has_endpoint":false,"has_fixed_price":false,"purchase_available":false,"endpoint_verification":{"status":"not_applicable"}}
```
```json
{"zadp":3,"commerce":{"endpoint":"https://seller.example"},"has_endpoint":true,"has_fixed_price":false,"purchase_available":false,"endpoint_verification":{"status":"verified","origin":"https://seller.example"}}
```
```json
{"zadp":3,"price":{"amount":"0.01","currency":"ZEC"},"commerce":{"endpoint":"https://seller.example"},"has_endpoint":true,"has_fixed_price":true,"purchase_available":true,"endpoint_verification":{"status":"verified","origin":"https://seller.example"}}
```

Excerpts omit signed identity/times and proof expiry/lease. Buy additionally needs
valid signature, active board-admitted record, permitted host visibility, a current
proof and a supported reference order capability (`text.uppercase` or
`text.summarize`). Those reference capabilities implement the seller order flow;
proof alone never establishes ordering support. `/capabilities` is unsigned
seller metadata, not Offer authority; generic endpoints need not serve `/orders`.

## CLI and Web

CLI: neither flag for discovery; `--endpoint https://seller.example` for no-price
service; add `--price-zec 0.01` for a fixed-price service. Price alone is rejected.
Web has an optional seller endpoint checkbox and optional fixed price within it.
No manual-settlement product mode remains. A verified no-price service gets
View service to its HTTPS origin with `_blank`, `noopener noreferrer` and no
referrer. No orders, invoices, payment destinations or discovery credentials are
created/forwarded. Unverified endpoints have neither Buy nor promoted service link.

This implementation does not deploy, publish, broadcast, activate charging or
change custody. Upgrade paired Core/API and Web before separately approved real
publication. Buyer GPS and local matching are unchanged.

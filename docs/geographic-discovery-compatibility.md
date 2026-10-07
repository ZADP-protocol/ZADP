# Geographic discovery: compatibility gate

Status: proposed, **not implemented**. The geographic discovery request requires
this review before any version change. Current OFFER encoding/signing, indexer
schema, UI and live mainnet services remain unchanged.

## Why the requested stop condition applies

The current [protocol versioning rule](protocol.md#versioning) says:

> Unknown fields MUST NOT enter the signed canonical CBOR set without a version bump.

The current encoder/decoder in
[`memo.rs`](../crates/zadp-protocol/src/memo.rs) uses an exact 13-element
`WireOffer` tuple and wire version 1. There is no reserved extension slot.
Trailing CBOR and nonzero memo padding are rejected. An appended geo tuple cannot
be decoded by the existing reader. Adding it under the same version would change
the defined wire representation despite retaining all earlier tuple positions.

[`canonical.rs`](../crates/zadp-protocol/src/canonical.rs) signs a known-field
OFFER map under `ZADP1\0offer\0`. Geo must be authenticated by that signature;
unsigned JSON display fields would allow a host to change an offer's service
area without invalidating its signature. Existing Serde OFFER readers ignore
unknown JSON fields, and do not sign them. Adding geo only to JSON would therefore
fail the requested authenticated, chain-reconstructible discovery semantics.

Reusing the description or padding as a hidden geographic format would introduce
new implicit wire semantics and cannot provide the requested typed optional
field safely. Weakening the versioning rule or fixed-tuple checks without review
is also a protocol change, not backward-compatible implementation work.

The user explicitly requested: if a version bump appears necessary, stop and
document why before changing it. Consequently no geographic code, schema,
canonical signing field, protocol constant or memo version has been changed.

## Recommended decision for approval

Introduce a versioned geo-capable OFFER variant while retaining the complete
legacy OFFER reader/writer/signature path:

- Existing offers without geo retain protocol 1 / wire 1 and their exact signing
  bytes. Existing published memos remain valid and readable by upgraded nodes.
- Geo-bearing offers use protocol 2 / wire 2, a distinct OFFER signing domain
  (proposed `ZADP2\0offer\0`), and a compact 14-element memo tuple: the previous
  fields followed by `[lat_e5, lon_e5, radius_m]`. This is a proposal, not an
  accepted protocol specification or implementation.
- Geo remains optional. Do not change the existing board descriptor version,
  fingerprint, UIVK, accounts, receiver or birthday merely to implement geo.
  Upgraded admission must explicitly support both OFFER versions with the same
  existing board policy. Any descriptor change would require separate review.
- Old nodes can continue accepting legacy offers, but reject the new variant.
  Geo publishers must know their intended indexers support it. Backward reading
  compatibility does not mean old nodes can index newly encoded geo offers.
- Validate offer integers without floats: latitude ±9,000,000; longitude
  ±18,000,000; offer radius 1..500,000 meters. Buyer radius may be zero and is
  bounded separately at 500,000 meters. Reject partial fields.
- At these bounds the proposed compact geo triple adds at most 16 payload bytes
  (one array header plus three integers of at most five bytes each). The total
  payload must still fit 504 bytes / the framed 512-byte memo. A previously full
  offer must be shortened before adding geo; no other field limits increase.
  Confirm the exact impact with encoder and Rust/browser parity regressions
  before accepting the variant.

Using protocol 2/wire 2 is the recommended explicit boundary, not permission to
bump a global version constant and invalidate all earlier offers. The exact
version/signature-domain design needs approval before implementation.

## Subsequent implementation scope after that decision

1. Add the optional signed integer area model and dual-version memo tests,
   including fixed legacy byte/signature fixtures and maximum memo budget.
2. Persist geo as indexed OFFER data. Existing SQLite already stores complete
   OFFER JSON, so typed optional JSON persistence may avoid a schema migration.
   Review whether explicit nullable columns are needed; do not alter a live
   database as part of a development test. No buyer location table or history.
3. Implement Haversine circle intersection in meters: center distance <= buyer
   radius + offer radius. Nearby returns matching active geo offers ordered by
   distance; Anywhere includes global/no-geo offers. Distance is derived search
   metadata, never a signed OFFER field. UPDATE/CLOSE wire messages are currently
   unimplemented and must not be silently introduced by this feature; retain
   first-wins, expiry, reorg and host moderation behavior.
4. Add structured `POST /offers/search` for UI, agents and API clients. Buyer
   coordinates are temporary request-body data, not URLs, logs, database state,
   chain content, orders or seller disclosures. Audit host/proxy body logging;
   honestly disclose that the search operator receives the query coordinates.
5. Add optional Offer area controls with an explicit public-data warning and
   approximate centers, plus buyer Nearby/Anywhere controls and manual input.
   Request browser GPS only on a deliberate click; no background permission,
   reverse geocoding, map dependency or tracking. Graceful denial keeps global
   discovery usable. All UI and documentation remain English.
6. Update protocol/API, architecture, publisher/indexer/UI documentation,
   threat model, roadmap, examples and concise README use case. Verify all layers
   with local fixtures, including restart persistence and search privacy.

## Verification and mainnet boundary

The existing protocol tests for fixed tuple length/padding/trailing CBOR and
unsupported versions passed during this review. These are current-format tests,
not evidence that geographic discovery is implemented. No new external board,
mainnet publication, transaction, database migration or runtime restart occurred.

Implementation may continue after the versioned-extension decision. Any later
mainnet geo publication remains a separate explicitly approved action with
signed board/receiver verification and an actual unsigned fee preview.

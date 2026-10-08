# Geographic discovery compatibility and privacy plan

Status: implemented reference path with exact legacy-byte regression and local privacy tests.
This supersedes the earlier server-search proposal and compatibility pause.

- BoardDescriptor remains v1. Its fields, signing domain, address/UIVK, policy,
  serialization and fingerprint remain unchanged. A dedicated board-version
  constant prevents an OFFER version update from affecting descriptors.
- Offer/wire v1 and `ZADP1\0offer\0` remain byte-compatible. A deterministic
  legacy memo and signature preimage was captured before modifying the encoder;
  the regression suite compares exact bytes against that fixture.
- Offer v2 signs optional geo and optional commerce with `ZADP2\0offer\0`.
  Wire v2 is a corrected 13-element tuple containing nullable commerce
  `[endpoint, price_zat]` and nullable geo. This replaces the experimental
  14-element v2 before first recorded mainnet use; v1 remains unchanged.
- Integers: latitude ±9,000,000, longitude ±18,000,000, offer radius 1..500,000 m.
  Buyer radius 0..500,000 m. Geo must be complete; maximum payload remains 504
  bytes. See [exact optional-commerce encoding](optional-commerce.md).
- Store geo inside existing serialized OFFER JSON. No SQLite migration, buyer
  data table or live database/runtime changes. First-wins/reorg/moderation and
  expiry remain unchanged; UPDATE/CLOSE messages remain unimplemented.
- Public GET /offers exposes signed integer geo metadata. No POST /offers/search,
  latitude/longitude query parameters or other buyer-location endpoint.
- A pure local JS module provides integer conversion, Haversine distance,
  intersection and ordered matching for browsers and Node/agent clients. Buyer
  position exists only in memory; GPS acquisition, radius changes and local
  filtering cause no network request and write no browser storage/history.
- Seller Offer area is optional public discovery metadata. Explicit warning,
  manual approximate coordinates and click-only GPS; no addresses or maps.
- Preserve existing commerce as an optional seller capability or attaching buyer coordinates
  to orders. Reclassify real paid commerce/SLA as post-MVP work.

Implementation order: legacy baseline; versioned protocol/signatures/memos;
indexer/API regression; seller publisher/UI; pure local buyer matching; privacy
and regression tests; documentation; isolated demo and full workspace checks.
Stop if any legacy bytes, descriptor, live fingerprint, unsafe migration or
backend-location requirement is encountered. Do not publish, broadcast or alter
the existing mainnet deployment. A first geographic mainnet OFFER is a separate
human-approved publication after implementation review and unsigned fee preview.

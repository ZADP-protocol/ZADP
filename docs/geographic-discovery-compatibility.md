# Geographic discovery compatibility and privacy plan

Status: implemented reference path with exact legacy-byte regression and local privacy tests.
This supersedes the earlier server-search proposal and compatibility pause.

- BoardDescriptor remains v1. Its fields, signing domain, address/UIVK, policy,
  serialization and fingerprint remain unchanged. A dedicated board-version
  constant prevents an OFFER version update from affecting descriptors.
- Offer/wire v1 and `ZADP1\0offer\0` remain byte-compatible. A deterministic
  legacy memo and signature preimage was captured before modifying the encoder;
  the regression suite compares exact bytes against that fixture.
- Offer v2 uses `ZADP2\0offer\0`. Optional geo is signed as an integer triple
  `[lat_e5, lon_e5, radius_m]` under canonical map key `geo`. Wire v2 is the
  existing 13-field tuple followed by that triple or null: 14 elements. Offer
  and wire versions must agree. Geo is rejected on v1; no-geo drafts default v1.
- Integers: latitude ±9,000,000, longitude ±18,000,000, offer radius 1..500,000 m.
  Buyer radius 0..500,000 m. Geo must be complete. The maximum geo overhead is
  16 payload bytes; v2 without geo adds one null byte. The payload limit remains
  504 bytes with unchanged individual text limits and whole-second timestamps.
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
- Preserve existing commerce without extending it or attaching buyer coordinates
  to orders. Reclassify real paid commerce/SLA as post-MVP work.

Implementation order: legacy baseline; versioned protocol/signatures/memos;
indexer/API regression; seller publisher/UI; pure local buyer matching; privacy
and regression tests; documentation; isolated demo and full workspace checks.
Stop if any legacy bytes, descriptor, live fingerprint, unsafe migration or
backend-location requirement is encountered. Do not publish, broadcast or alter
the existing mainnet deployment. A first geographic mainnet OFFER is a separate
human-approved publication after implementation review and unsigned fee preview.

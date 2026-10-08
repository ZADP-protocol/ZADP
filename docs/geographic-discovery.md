# Geographic discovery

Publish a service to a public approximate area through a signed Zcash OFFER;
discover it by matching downloaded offers with a private local point. Geographic
discovery is a primary MVP feature. Commerce remains separate and optional; discovery needs no endpoint or price.

## Formats and compatibility

- Offer v1 and wire v1 remain byte-compatible, with a captured golden JSON,
  canonical signing preimage and full 512-byte memo regression fixture.
- Offer v2 uses `ZADP2\0offer\0` plus the existing canonical map. Optional signed
  geo is `{"lat_e5":3777493,"lon_e5":-12241942,"radius_m":50000}`.
- Integer coordinate limits: latitude ±9000000, longitude ±18000000;
  offer radius 1..500000 m. Geo is either absent or a complete valid triple.
- Wire v2 is a 13-element discovery tuple with nullable commerce and geo;
  see [optional commerce](optional-commerce.md). Mainnet v2 has not yet been
  published in the recorded deployment. Upgrade readers and rebuild old
  experimental v2 fixtures. Framing stays 512 bytes with 504 payload bytes.
- BoardDescriptor v1 domain, address, UIVK and fingerprint remain unchanged.
  One board can accept both offer versions. Full JSON SQLite persistence needs
  no migration. Old software rejects v2; upgrade before publishing it. Rebuild
  a previously advanced/rejecting old index separately from the signed birthday.

[Normative signing/wire details](protocol.md),
[compatibility decision](geographic-discovery-compatibility.md).

## Human discovery

Anywhere shows all active visible offers, including global/no-geo offers.
Click Use my location or enter manual coordinates to use Nearby. Location access
never occurs on page load. Denial is non-blocking. Search radius defaults to
25 km after acquisition; presets 5/25/50/100/250 km and radius zero are supported.
Zero means the public offer area includes the buyer point. Clear location resets
Anywhere; reload starts without buyer location.

The same pure matcher implements:

```text
Haversine(buyer point, public offer-area center)
    <= buyer search radius + public offer radius
```

Meters are used internally, with mean Earth radius 6371008.8 m. Antimeridian
crossing and polar points work; numerical rounding is clamped. Buyer degree
input is rounded to integer e5 in transient local state (roughly meter scale).
Nearby filters only geographic offers and sorts by distance to the published
area center. Cards do not claim a seller is physically nearby. Public centers
are self-declared discovery/service areas, not verified premises or addresses.

## Seller area

Enable geographic discovery in My offer, review the public-data warning, and
enter an approximate center and a service radius. Seller Use my current location
is a separate click-only action; it never borrows the buyer point. It requests
browser location, rounds latitude and longitude locally to **3 decimal degrees**,
and fills visible form fields for review/editing. It does not publish the exact
device position, save the draft automatically or broadcast an offer. Publication
still requires the normal separate confirmation.

For example, browser 37.774929 / -122.419416 becomes 37.775 / -122.419 in the form,
then integer e5 3777500 / -12241900 in the draft. Decimal halfway values round away
from zero on both signs; tiny values rounded to zero display as 0.000. The
reference uses Intl decimal halfExpand rounding, not Math.round on negative ties.
Raw browser coordinates are checked against normal bounds before rounding.

This is approximately 100-meter-scale coordinate precision, varying with latitude,
especially for longitude; it is not fixed accuracy or anonymity. Rounded centers
can still reveal a small sensitive area. Manually entered or edited coordinates
retain seller-selected precision within the existing e5/form limit (five decimal
places), without automatic reduction to three decimals. Geo remains optional.
Other clients may intentionally publish different precision. A conforming client
must not assume all offer centers have three decimals or e5 multiples of 100.

A custom radius uses integer meters 1..500000. Unsigned drafts may intentionally save or
export this public area. Memo budget checks select v2 when geo is present or commerce is absent.
The protected operator preview passes the complete e5 triple to the real CLI;
there is no browser signing/publication/broadcast route.

The CLI similarly requires all three flags together:

```text
--geo-lat-e5 3777493 --geo-lon-e5 -12241942 --geo-radius-m 50000
```

An offer area becomes public via UIVK discovery. Zcash shielding does not hide
indexed metadata. A city/region center may be safer than precise premises.

## Machine discovery

GET /offers exposes the same signed integer geo JSON to human and agent clients.
There is no coordinate query or POST /offers/search. Unknown query fields are
rejected; API tracing omits request URIs and bodies. No buyer-location database
columns, migration or order fields are added.

Use the same pure module from Node, without an SDK or network dependency:

```sh
curl --fail --silent http://127.0.0.1:8098/offers > /tmp/public-offers.json
# Synthetic public demo point only; an agent supplies its private point in memory.
printf '%s' '{"lat_e5":3777493,"lon_e5":-12241942,"radius_m":25000}' | \
  node examples/geo/discover.js /tmp/public-offers.json
```

Only the public offer feed is fetched and optionally saved. Matching happens in
this process, and output contains matching public offers, never buyer coordinates.
The example receives its private integer search circle on stdin, not in
coordinate command arguments, and never saves or prints it. Real agents should
supply that input programmatically from memory or directly call `fromDegrees` /
`geoAreasIntersect`; do not paste private coordinates into saved shell commands.
No location is sent to the API. Other language clients can use the same formula
and fixtures.

## Reproducible offline demo

Use a fresh temporary database; these are synthetic testnet descriptors, signed
offers and decoded memo blocks, **not real chain publication**. Fixture addresses
are deterministic public test material and must never be funded. No wallet,
account, spending credential, network scan or transaction is used.

```sh
cargo run -p zadp-api --example geo_fixture -- \
  --database /tmp/zadp-geo-demo.sqlite --bind 127.0.0.1:8098
```

In a separate terminal:

```sh
python3 ui/server.py --port 8097 --indexer-port 8098
```

1. Open http://127.0.0.1:8097. Anywhere shows San Francisco/Boston service areas plus
   one global offer. All are explicitly local simulation, not payable services.
2. Enter manual buyer coordinates 37.77493 / -122.41942 and choose 25 km.
   Nearby shows San Francisco only; radius zero also includes San Francisco. Switch Anywhere to
   restore all three; clear location or reload to discard the buyer point.
3. Inspect details for the published approximate area; no “seller distance”.
4. Create a seller draft with optional area and inspect the v2 byte budget;
   saving a no-geo draft never copies buyer location.
5. Restart the fixture with the same database: persisted v1/v2 offers remain.
   The fixture initializes only an empty database and never overwrites a running
   reconstruction. It does not renew expiry; a fresh demo DB is needed after
   the seven-day synthetic lifetime.

The actual protocol signer, memo decoder, board admission, SQLite repository and
HTTP router are used. Live CompactTxStreamer encryption remains covered by the
existing offline encrypted scanner regressions. Current mainnet daemon binaries,
board descriptor, wallet/accounts and indices are preserved.

## Privacy verification and limitations

The single matcher is bundled in the existing `ui/offers.js` browser asset;
`ui/geo.js` is its CommonJS export entry for agents/tests. No extra browser asset
route or running UI server restart is needed. Geolocation API calls exist only
at the buyer button via BuyerLocation.request
and the seller area button. There is no watchPosition or automatic acquisition.
Acquisition, mode/radius changes and local filtering trigger zero requests.
Explicit Refresh retains public feed/health and optional read-only wallet-status
requests without buyer coordinates; independently active commerce polling is
unchanged and carries no discovery point.

The actual app test intercepts fetch, logging and storage with synthetic GPS:
no automatic permission request, location transmission, persistent buyer state
or seller-draft contamination. No XHR, analytics, maps, history/coordinate URL,
localStorage/sessionStorage/cookie/IndexedDB buyer path is implemented. Seller
public drafts and private commerce recovery capabilities retain their separate
existing storage behavior. The application does not infer GPS from those values.

This is an application boundary, not a guarantee against malicious extensions,
a compromised client or the browser/OS location provider. Feed requests can
still reveal ordinary IP/network metadata. Later direct orders may voluntarily
include delivery details; discovery never adds them automatically.

## Tests and release gate

- `cargo test --workspace`: v1 golden bytes, v2 absent/present geo, range/partial
  rejection, each signature tamper, wire round trip, memo fit/overflow, persistent
  admission/restart/replay/host visibility, API boundaries and prior scanner tests.
- `node ui/test_geo.js`: known distances, circle boundaries, 0/5/25/50 km,
  poles/antimeridian, denial, actual app network/storage/log interception, automatic
  seller rounding (positive/negative/ties/bounds), visible review values and
  preservation of manually entered e5 precision.
- `node ui/test_memo_rust_parity.js`: 86 real Rust/browser byte-budget cases.
- Existing offer, purchase, commerce, Python boundary and independent-process
  tests plus fmt, clippy, documentation links and mock demo smoke remain required.

Source implementation is ready for manual local review. A first real geo offer
still requires review/deployment of upgraded readers/writer, a protected backup
before any future runtime change, a public approximate area, optional valid commerce,
valid signed-board policy, authoritative fee preview and explicit approval.
No mainnet geo transaction is prepared or broadcast by this development task.

## Compact mobile discovery

The mobile header opens a menu for Services, My offer, Test service, Buy from
seller and Connection. Search occupies its own row above board selection.
Anywhere shows active offers without location controls. Nearby without a point
asks for a click-only browser location or manually entered coordinates, and shows
no results until a point is chosen. Acquiring a point automatically selects Nearby.
Nearby with a point shows a 25 km default radius and Clear location. Switching to
Anywhere hides radius and clearing controls without persisting or transmitting the
point. Clear discards the point; reloading also forgets it. Detailed matching help
is collapsed below the short device-local privacy notice.

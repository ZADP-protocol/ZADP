# ZADP — Zcash Agent Discovery Protocol

A protocol and reference implementation for discovering seller-signed service offers through Zcash shielded memos.

**Discovery is open. Settlement can remain private.**

**ZADP publishes availability, not transactions.**

This repository contains the interoperable protocol, read-only indexer, generic API, reference clients and tests. An official hosted web product and its operations live in separate private repositories. Neither is required to implement or operate ZADP.

## Boards and signed offers

A board is defined by a signed [BoardDescriptor v1](docs/board-descriptor.md): its network, receiving address, incoming viewing capability, scan birthday and admission policy. The owner retains spending authority. An indexer needs only incoming viewing capability, which cannot spend funds or confer board ownership. An intentionally public board capability lets independent operators reconstruct the same offer board.

Sellers use a separate Ed25519 identity to sign canonical Offers. **Offer v1 / wire v1** compatibility is preserved. **Offer v2 / wire v2** supports optional commerce and signed geographic service areas. Offer IDs, board binding, issue/expiry times and replay checks are part of validation. Publication is not a verified claim about service quality or a seller's physical position.

The compact memo has a 512-byte envelope and a 504-byte payload budget. Names, descriptions, identifiers, optional commerce, geographic fields and signatures share that budget. See [protocol and wire format](docs/protocol.md) and [optional commerce](docs/optional-commerce.md).

## Local geographic discovery

A seller can publish “Bike repair — San Francisco area”, available within 25 km of approximate coordinates 37.775, -122.419. Canonical fields are `lat_e5`, `lon_e5`, and `radius_m`.

A buyer's chosen location remains in volatile client memory. The reference client uses local Haversine matching: distance to the **published offer-area center** must not exceed buyer search radius plus offer radius. Buyer GPS is never an API query, transaction field or persistence key. A public offer-area center is not a verified seller location. See [geographic discovery](docs/geographic-discovery.md).

## Independent indexing and self-hosting

The indexer scans Zcash compact blocks, trial-decrypts board outputs, decodes memos, verifies signatures and board policy, and maintains canonical SQLite state. Expired offers remain historical records but disappear from active discovery. Separate boards have separate persistent state. Generic host visibility can restrict a particular API without changing canonical protocol validity.

```sh
cargo build --workspace
cargo run -p zadp-api -- \
  --network testnet \
  --board /absolute/path/to/example-board.json \
  --lightwalletd https://your-testnet-lightwalletd.example:443 \
  --database /absolute/path/to/index.sqlite \
  --listen 127.0.0.1:8080
```

Use a valid signed descriptor for the same network as your transport. The API offers `GET /offers`, `GET /offers/{id}`, and board metadata/views. No wallet or spending key is required by the scanner. See [scanning](docs/zcash-scanning.md), [multi-board operation](docs/multi-board.md) and [self-hosting concepts](docs/hosting.md).

## Reference clients and optional settlement

The reusable [browser modules](client/browser/README.md) provide parsing, memo budgeting and local geo matching. A [minimal browser client](examples/browser/README.md), a [local geo CLI](examples/geo/discover.js), Rust protocol examples and test vectors show how to build another client without the private web product.

Commerce is optional. A discovery-only v2 Offer has no purchase endpoint or service price. Commerce-enabled Offers may advertise an HTTPS seller endpoint; orders, payment correlation and results belong directly to seller and buyer, not discovery. Reference commerce services and [isolated offline tests](docs/commerce-process-boundaries.md) demonstrate this boundary. They do not authorize a real payment.

## Status and validation

Alpha software: protocol/model details can still evolve. The first funded mainnet publication and independent reconstruction were verified; this is discovery evidence, not evidence of a live geographic business or completed buyer purchase. See [mainnet acceptance](docs/mainnet-acceptance.md).

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
node client/browser/test_geo.js
node client/browser/test_offers.js
node client/browser/test_memo_budget.js
node client/browser/test_memo_rust_parity.js
python3 scripts/check_doc_links.py
```

Read [development](docs/development.md), [threat model](docs/threat-model.md), [security](SECURITY.md), [contributing](CONTRIBUTING.md) and the [public release checklist](docs/PUBLIC_RELEASE_CHECKLIST.md).

## License

MIT OR Apache-2.0, at your option. See [MIT](LICENSE-MIT), [Apache](LICENSE-APACHE) and [license notice](LICENSE). Third-party dependencies retain their own licenses. Repository visibility is a separate owner decision; this cleanup does not make any repository public.

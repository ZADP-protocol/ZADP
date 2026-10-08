# Core development and independent operation

Use the Rust toolchain compatible with Cargo.lock, Node.js for the reference browser modules, and Python 3 for reference process tests. The current workspace builds with stable Rust; dependencies and feature combinations are pinned in Cargo.lock. OpenSSL is used only to create ephemeral certificates in the offline HTTPS commerce fixture.

```sh
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
node client/browser/test_geo.js
node client/browser/test_offers.js
node client/browser/test_memo_budget.js
node client/browser/test_memo_rust_parity.js
python3 -m unittest discover -s services/text -p 'test_*.py'
cargo build -p zadp-commerce --bins --examples
python3 -m unittest discover -s examples/commerce -p 'test_*.py'
python3 scripts/check_doc_links.py
cargo run -p zadp-demo
```

The [minimal browser reference](../examples/browser/README.md) and [client modules](../client/browser/README.md) require no private product repository. Start a generic read-only API with a valid signed descriptor and matching network; configure database and bind address explicitly. Use loopback by default.

Offline examples use deterministic public fixtures and ephemeral identities. Never fund fixture addresses or use fixture identities for a real wallet. Local tooling must not read spending keys into logs or chat. A real publication is a separately reviewed exact wallet proposal and approval, not a consequence of tests passing.

The branded hosted product and instance operations have [separate repository boundaries](repository-boundaries.md). They are not dependencies of core tests or builds.

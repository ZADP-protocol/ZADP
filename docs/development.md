# Development and safe local verification

The repository contains real mainnet discovery/publication code, a retained Rust
mock commerce demo, and an experimental local payment-gated Python text service.
The first live OFFER was mined and independently reconstructed; a live buyer
payment and paid result have not been demonstrated. See [evidence](mainnet-acceptance.md).

## Prerequisites

- Current stable Rust and Cargo, including rustfmt and Clippy. Use Cargo.lock;
  dependency MSRVs can advance, and the project has no separate tested MSRV.
- Node.js 22 for dependency-free UI helpers and Rust/JavaScript parity tests.
- Python 3.10+ on Linux for local UI/service tools; protected wallet boundaries
  require Unix owner/permission checks. Use a virtual environment for QR support:
  `python3 -m pip install -r ui/requirements.txt`.
- A signed public board descriptor and an explicitly selected compatible
  CompactTxStreamer/lightwalletd service for real discovery. Zebra JSON-RPC is
  not the compact-block gRPC endpoint.

## Offline mock demo

```sh
cargo run -p zadp-demo
```

This starts a temporary loopback mock API, publishes a synthetic signed offer,
checks policy, simulates payment and returns a mock result. It never accesses a
wallet or broadcasts. It exits after the demonstration; it is not a persistent
mainnet API. [Examples](../examples/README.md) describe the separate real tools.

## Read-only API and UI

With an independently trusted signed public descriptor:

```sh
cargo run -p zadp-api -- --network mainnet --board /public/board.json \
  --lightwalletd https://YOUR-COMPACT-SERVICE:443 \
  --database /local/board.sqlite --listen 127.0.0.1:8080
python3 ui/server.py
```

Open http://127.0.0.1:8090. The API supports `GET /healthz` and
`GET /offers?capability=text.shorten`. It has no detail/search/pagination route;
the persistent listing is capped at 1,000 active offers. Capability filtering
is exact, not substring search. `/healthz` is liveness, not synchronization
readiness. UI search runs over the returned offers.

Use `--network testnet` only with a testnet descriptor and testnet service.
Both descriptor and remote chain/consensus identity are validated. A database
pins its complete signed descriptor fingerprint; use a fresh database for a
changed descriptor. `--once` scans to tip and exits without serving HTTP.
The optional [operator UI](../ui/README.md) adds filtered wallet/QR and unsigned
publication previews; default UI configuration has no wallet access.

## Tests and quality checks

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo run -p zadp-demo
python3 -m unittest discover -s ui -p 'test_*.py'
python3 -m unittest discover -s services/text -p 'test_*.py'
node ui/test_offers.js
node ui/test_memo_budget.js
node ui/test_memo_rust_parity.js
python3 scripts/check_doc_links.py
```

The Rust oracle generates ephemeral Ed25519 identities offline, not wallet
accounts. Encrypted discovery fixtures contain disposable synthetic cryptographic
material. No test should read an operator wallet or require a real payment.
`cargo run -p zadp-api -- --help` and `cargo run -p zadp-seller -- --help` show
actual commands. See [dependency review](dependencies.md) for upgrade limits.

## Mainnet precautions

Keep seeds, spending keys, private seller viewing authority, authentication,
invoice storage and runtime configuration outside Git and agent context.
Indexers only receive the board's deliberately public incoming viewing key.
Do not recreate a working wallet or replay the existing accepted publication.
Never infer spend authorization from a successful sync or invoice. A mainnet
publication requires a fresh library-derived fee preview and explicit human
confirmation; buyer payment needs its own fee/proposal review and approval.
For inspection only, use `publish --dry-run --preview-output /local/preview.json`;
this may sign an Ed25519 OFFER, but cannot sign/broadcast a Zcash transaction.
Do not pass `--yes` during unattended verification.

## Troubleshooting

- **Index unavailable:** check the loopback indexer, its logs and UI port setting.
  Invalid/nested API JSON is rejected rather than rendered partially.
- **No offers:** scanning starts at the signed birthday; unrelated, rejected or
  expired offers are absent. Empty results have no network provenance in the
  existing HTTP schema; the UI labels the network unavailable in that case.
- **Slow scan:** inspect the persistent cursor and compare with an actual public
  chain tip. Do not treat Zebra estimatedheight as current canonical tip.
- **Descriptor/database mismatch:** preserve the old database; reconstruct into
  another database with the intended verified descriptor.
- **Consensus branch mismatch:** update/review compatible pinned libraries; never
  bypass the network check.
- **Wallet locked:** Zallet getwalletstatus.locked is a synchronization lock.
  It is not evidence that a keystore needs unlocking; do not bypass it.
- **TLS/cookie/config errors:** check permissions and ownership locally. Never
  paste credentials or disable certificate verification. Keep the local TLS
  certificate's matching SAN and use its explicit CA file.
- **Broadcast timeout:** check the recorded txid on chain before any human retry.
- **Paid test unavailable:** a fixed local TLS service and protected bridge
  configuration are required. The published example.com endpoint is a placeholder.

# Examples

The offline commerce demo is intentionally mocked:

```sh
cargo run -p zadp-demo
```

It starts a temporary loopback API, exercises seller/buyer roles, and exits.
It is separate from the verified real mainnet discovery implementation.

- `cargo run -p zadp-api -- --help`: persistent read-only board scanner/API.
- `cargo run -p zadp-seller -- --help`: identity/descriptor/publication commands.
  Do not invoke publication against a funded wallet without a fresh preview and
  explicit human approval. `--dry-run` never signs a Zcash transaction/broadcasts.
- `node ui/test_memo_rust_parity.js`: actual Rust encoder vs UI byte estimates,
  offline ephemeral Ed25519 fixtures, including exact 504-byte boundaries.
- `cargo run -p zadp-zcash --example network_probe -- https://YOUR-SERVICE:443`:
  optional read-only mainnet compact transport probe, not acceptance evidence.
- `python3 ui/server.py`: English local discovery/draft UI (indexer required).
- `python3 -m unittest discover -s services/text -p 'test_*.py'`: synthetic
  payment-gate tests; no wallet initialization or real payment.

See [development](../docs/development.md), [live proof](../docs/mainnet-acceptance.md)
and [paid-service design](../docs/paid-service-flow.md). Buyer settlement and public
hosted operations remain incomplete; the example.com mainnet offer is a placeholder.

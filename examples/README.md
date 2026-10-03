# Examples

Phase 1 ships a single orchestrated vertical slice:

```bash
cargo run -p zap-demo
```

That binary exercises both seller and buyer roles against the in-process mock Zcash adapter and local REST API.

Future split binaries (`demo-seller`, `demo-buyer`) can be added once multi-process demos are useful; the domain crates already separate those concerns.

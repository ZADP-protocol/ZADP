# Reproducible Zallet beta.3 scan fixes

These patches fix recent-account scan range panics and stale pre-birthday
`fully_scanned_height` reporting. They do not change accounts, birthdays,
credentials, synchronization locks or spending policy.

Pinned upstream sources:

- Zallet: `987382f67e622915228686e9f956c6a9c9a7514c`
- librustzcash: `1f6bb2072e7fcb142b0d90ff7b267a8699a84818`

In a fresh librustzcash checkout at that revision, apply both patches in order:

```sh
git apply /path/to/ZAP/integrations/zallet/patches/0001-guard-update_chain_tip-below-birthday.patch
git apply /path/to/ZAP/integrations/zallet/patches/0002-fully-scanned-height-skip-prebirthday.patch
cargo +stable test -p zcash_client_sqlite --features "orchard transparent-inputs" -j1 fully_scanned_height
cargo +stable test -p zcash_client_sqlite --features "orchard transparent-inputs" -j1 block_fully_scanned
cargo +stable test -p zcash_client_sqlite --features "orchard transparent-inputs" -j1 update_chain_tip_skips_when_tip_below_birthday
```

Verified: six fully-scanned regressions, one existing block test, and two
below-birthday guard tests passed. Tests cover empty queues, absent accounts,
stale ranges, birthday boundaries and gaps; progress is never invented.

Run `python3 integrations/zallet/wire.py /path/to/zallet /path/to/librustzcash`
to replace all 13 existing librustzcash overrides in Zallet's root manifest
with the same local source cohort. Then build the actual Zaino backend:

```sh
cargo +stable build --manifest-path /path/to/zallet/backends/zaino/Cargo.toml --release -j1
```

The root launcher alone does not rebuild the backend. The release executable
is `target/release/zallet-zaino`; the build may take a long time (RocksDB).
The verified installed binary SHA-256 was
`e538f9044e31a9c1acd1520965765f5ead9e9bb5f68136c209e4b563395378ba`;
build hashes may differ between environments.

Runtime installation is deliberately separate: stop only Zallet, make a
consistent protected SQLite backup, retain the previous executable, restart
with the existing configuration, and verify synchronization and balances.
The existing runtime has already been fixed; do not repeat scan-queue maintenance.
No wallet databases, identity files or credentials are stored here.

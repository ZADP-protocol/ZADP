# Actual Zcash scanning

The production scanner uses official `zcash_client_backend` 0.24, `zcash_keys` 0.16, `zcash_primitives` 0.30, `zcash_protocol` 0.10 and Orchard 0.15 (exact resolutions in Cargo.lock). APIs were checked against downloaded official library source and the official [librustzcash repository](https://github.com/zcash/librustzcash). Both mainnet and testnet are selected explicitly; mainnet is the documented demo path.

```sh
cargo run -p zadp-api -- --network mainnet --board board.json --lightwalletd https://YOUR-SERVICE:PORT --database board.sqlite --listen 127.0.0.1:8080
curl 'http://127.0.0.1:8080/offers?capability=llm.inference'
```

Use an actual CompactTxStreamer-compatible lightwalletd service (backed by a correctly synchronized node, such as Zebra). Zebra RPC itself is not this gRPC API. TLS with public CA roots is required except explicit loopback HTTP. The endpoint is operator-selected; no default hosted provider is assumed. `GetLightdInfo` verifies the network and a supported consensus branch. `GetLatestBlock`, `GetBlock`, and `GetTransaction` are official protocol calls, not invented RPCs. See the [lightwalletd protocol](https://github.com/zcash/lightwalletd/blob/master/walletrpc/service.proto).

The scanner starts at the signed birthday, trial-decrypts Orchard and Ironwood compact actions locally, and fetches full transactions only when the board receiver matches. Compact ciphertext alone does **not** contain the whole memo. Full transaction parsing checks txid, branch, height and block stability; full output decryption provides the 512-byte memo and value. Notes to other diversified receivers are ignored. There is no spending-key type/credential in the scanner.

`BoardSource` is the read-only boundary. `RealZcashScanner` implements it; deterministic fixtures implement it in tests. Existing `ZcashAdapter` and `MockZcashAdapter` remain available for the commerce demo. Domain admission and repository logic operate independently of network transport.

## Persistence and reorganizations

SQLite atomically commits accepted offers, rejection codes and block cursor. The first block must equal the birthday, then height/previous-hash continuity is mandatory. Same-hash duplicates are idempotent. Restart checks the stored chain against the source and finds the common ancestor; orphaned offers/rejections/cursors cascade away, then replacement blocks are replayed. Accepted output identity and seller/id and seller/nonce have uniqueness constraints. Failed blocks never advance the cursor. Files use WAL and FULL synchronization.

Confirmed here means the descriptor's threshold measured from the **locally scanned** tip. First mined inclusion is immediately visible as `seen`; there is no mempool discovery. `chain` includes transaction, pool/output, inclusion block, scanned height, confirmations, `confirmation_status`, and `offer_status: active`. Expired offers are excluded at query time. `--once` catches up and exits. `/healthz` indicates HTTP process availability, not completed sync; clients must inspect scanned height and operator logs. Rejected messages retain bounded reason labels and transaction/output identity, never unrelated memo plaintext or key material.

This is a light-client source, not independent full consensus/proof validation. A dishonest service can censor, delay or supply a fabricated chain. Use a trusted self-hosted service or independently compare providers and node tips. Deep reorganizations below the birthday reconstruct from the birthday. A second indexer using the identical descriptor and canonical chain obtains the same admissions; historical offer contents remain reconstructible even after TTL expiry.

## Read-only network check

`cargo run -p zadp-zcash --example network_probe -- https://YOUR-SERVICE:PORT` checks the mainnet chain/consensus branch and downloads the current compact block, without any wallet access or broadcast. On 2026-10-03 this check against `https://zec.rocks:443` returned height 3505016 and protocol-order hash `872eab86b2aeaea17bbcac236c86b2e9596f5f494279a0b745786e0000000000`. This proves transport access, not discovery of a real ZADP publication. TLS provider selection is explicit because transitive Zcash/wallet dependencies may enable both rustls providers.

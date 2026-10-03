# Explicit mainnet publication

**Implementation status:** real network transport, decryption and publication code exist; the funded mainnet acceptance demonstration has not been performed. No transaction id or successful production broadcast is claimed.

The MVP wallet integration targets local [Zallet PCZT RPC](https://zcash.github.io/zallet/rpc/index.html), verified against official Zallet source revision `f49a9a98b631cb1ba04084742f7b728eed1e40f4`. Zallet is evolving; use a version exposing these documented methods. This is not a generic zcashd RPC wallet adapter. Prepare a synchronized, funded local Orchard-family account and obtain its account UUID from your wallet locally. Keep wallet seeds/spending keys within the wallet process, outside the repository and agent context.

Create a seller Ed25519 identity using `identity-init --output seller.identity`. Identity and RPC authentication files must be regular, owner-only files (0600), owned by the current user, without symlinks. The authentication file contains local Zallet `username:password`; avoid entering secrets as command arguments. `ZADP_IDENTITY_FILE` and `ZADP_WALLET_AUTH_FILE` may contain file paths. Protected file handling currently supports Linux.

```sh
cargo run -p zadp-seller -- publish \
  --network mainnet --board board.json \
  --identity /secure/local/seller.identity \
  --capability llm.inference --title 'ZADP Demo Inference' \
  --price-zec 0.01 --endpoint https://example.com/zadp \
  --ttl-seconds 86400 \
  --wallet-rpc http://127.0.0.1:8232 \
  --wallet-auth-file /secure/local/zallet.wallet-auth \
  --wallet-account YOUR-ACCOUNT-UUID \
  --lightwalletd https://YOUR-SERVICE:PORT \
  --receipt demo.publication.json
```

First run with `--dry-run`: it builds an unsigned wallet proposal and shows the fee without proving, signing or broadcasting. The displayed service price is not paid during publication. The listing value defaults to the descriptor's configurable minimum, or pass `--listing-value-zat`. The wallet's supported fee calculation provides `fee_zat`; ZADP contains no invented constant network fee.

The CLI verifies the wallet and network-service block hash agree, builds a signed board-bound OFFER and <=512-byte binary memo, requests `pczt_create` with `FullPrivacy` and Orchard-family funding, and validates `pczt_inspect`. It displays board/address, OFFER, listing amount, estimated fee and total. Default confirmation is `Proceed? [y/N]`; `--yes` explicitly bypasses the prompt. It then calls `pczt_prove`, `pczt_sign`, `pczt_extract`, locally parses/decrypts the extracted transaction and checks exact destination/value/memo, privacy, fee and txid before official lightwalletd `SendTransaction`. Wallet keys never go to lightwalletd or an LLM. The local wallet remains trusted for change ownership and spend authorization.

A proposal cannot broadcast from any automated test. Tests never access a funded wallet. If broadcast times out, acceptance may be unknown: consult the printed txid before retrying, or you could pay twice. A receipt is written before transmission with `broadcast_status: prepared; verify on chain`; it is evidence of the intended transaction, not proof of network acceptance. TTL may expire while wallet processing takes time; confirm clock, wallet sync and eventual block inclusion.

## Required live acceptance (still pending)

1. Provision and pin the dedicated signed mainnet board descriptor.
2. Dry-run and review the real fee/destination locally; explicitly invoke and confirm publication.
3. Record the accepted txid, descriptor fingerprint and inclusion height.
4. Run the scanner until that block is processed. Query `/offers?capability=llm.inference` and verify the exact signed offer/txid plus confirmation metadata.
5. Start another scanner with the same descriptor and a fresh database; verify identical offer identity and inclusion metadata (scanned heights/confirmations can differ).
6. Archive public evidence only; never wallet credentials/seeds. A local encrypted fixture is not a substitute for these steps.

Until all steps succeed, the mainnet milestone is incomplete. Purchase settlement, AI purchasing, reputation and hosted marketplace are outside this milestone.

# Explicit mainnet publication

**Implementation status:** real network transport, decryption and publication code exist; the funded mainnet acceptance demonstration has not been performed. No transaction id or successful production broadcast is claimed.

## Wallet version and verification status

The MVP wallet integration targets local [Zallet PCZT RPC](https://zcash.github.io/zallet/rpc/index.html). This is not a generic zcashd RPC wallet adapter.

| Reference | Revision | Role |
| --- | --- | --- |
| Released **Zallet `v0.1.0-beta.3`** | `987382f67e622915228686e9f956c6a9c9a7514c` | Install / runtime target used for source and RPC-help compatibility checks |
| Later main-branch pin (docs history) | `f49a9a98b631cb1ba04084742f7b728eed1e40f4` | Earlier source pin; newer than the beta.3 tag |

**Verified (source / RPC surface against beta.3):** argument order and field names for `getwalletstatus`, `pczt_create`, `pczt_inspect`, `pczt_prove`, `pczt_sign`, and `pczt_extract` match ZADP seller expectations. Public board-provisioning helpers (`z_getnewaccount`, Orchard-only `z_getaddressforaccount`, `z_exportviewingkey` with `ivk=true`) exist in that release.

**Still pending:** a funded proposal, dry-run fee review against a live wallet, explicit confirm, broadcast, mined inclusion, and independent scanner discovery. Do not treat source compatibility as mainnet acceptance.

Zallet is evolving; prefer a build that exposes these methods. Keep wallet seeds and spending keys inside the wallet process, outside the repository and agent context.

## Chain backends: scanner vs wallet

- **ZADP board scanning / HTTP indexer** needs only a compatible **public lightwalletd** (or equivalent compact-block service) URL plus the signed `board.json`. A local full node is optional for scanning.
- **Zallet-backed publication** additionally needs whatever **backing full-node connection** the chosen Zallet backend requires (for example a local `zebrad` with indexer features for `zallet-zebra`, or an authenticated JSON-RPC chain view such as Zaino). That node is separate from the lightwalletd URL passed to `zadp-seller publish` / `zadp-api`.
- Before funding or publication: **both** the wallet (`getwalletstatus` not sync-locked) and its backing node tip must be ready, and the canonical block hash at the wallet node-tip height must agree with lightwalletd as checked by the publish CLI.

## Human backup before provisioning

If Zallet generated the mnemonic, the operator must record it and run **`zallet confirm-backup`** before `z_getnewaccount` / address derivation will proceed (`keystore.require_backup`). The keystore must also be unlocked for account creation and viewing-key export. Agents must never receive seed material, spending keys or authentication-file contents. Protected local file paths can be passed to the CLI; credentials stay inside local processes.

## Publish CLI

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

The CLI verifies the wallet and network-service block hash agree, builds a signed board-bound OFFER and <=512-byte binary memo, requests `pczt_create` with `FullPrivacy` and Orchard-family funding (`fund_source` `orchard`), and validates `pczt_inspect`. It displays board/address, OFFER, listing amount, estimated fee and total. Default confirmation is `Proceed? [y/N]`; `--yes` explicitly bypasses the prompt. It then calls `pczt_prove`, `pczt_sign`, `pczt_extract`, locally parses/decrypts the extracted transaction and checks exact destination/value/memo, privacy, fee and txid before official lightwalletd `SendTransaction`. Wallet keys never go to lightwalletd or an LLM. The local wallet remains trusted for change ownership and spend authorization.

A proposal cannot broadcast from any automated test. Tests never access a funded wallet. If broadcast times out, acceptance may be unknown: consult the printed txid before retrying, or you could pay twice. A receipt is written before transmission with `broadcast_status: prepared; verify on chain`; it is evidence of the intended transaction, not proof of network acceptance. TTL may expire while wallet processing takes time; confirm clock, wallet sync and eventual block inclusion.

## Required live acceptance (still pending)

1. Provision and pin the dedicated signed mainnet board descriptor.
2. Dry-run and review the real fee/destination locally; explicitly invoke and confirm publication.
3. Record the accepted txid, descriptor fingerprint and inclusion height.
4. Run the scanner until that block is processed. Query `/offers?capability=llm.inference` and verify the exact signed offer/txid plus confirmation metadata.
5. Start another scanner with the same descriptor and a fresh database; verify identical offer identity and inclusion metadata (scanned heights/confirmations can differ).
6. Archive public evidence only; never wallet credentials/seeds. A local encrypted fixture is not a substitute for these steps.

Until all steps succeed, the mainnet milestone is incomplete. Purchase settlement, AI purchasing, reputation and hosted marketplace are outside this milestone.

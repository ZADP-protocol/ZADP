# Portable board descriptor

A board is a dedicated Orchard-only Unified Address. It is not a public block-explorer search. Indexers download compact blocks and trial-decrypt with its **Unified Incoming Viewing Key (UIVK)**. Only the exact receiver in the descriptor is accepted; other diversified addresses under that key are excluded.

The JSON has two top-level fields: `board` and `signature`. `board` contains `zadp`, `board_id`, `name`, `network` (`mainnet` or `testnet`), `address`, `incoming_viewing_key`, `birthday_height`, `policy`, and `operator_key`. Unknown fields are rejected. The signature is base64 Ed25519 over `ZADP1\0board\0` followed by deterministic CBOR of the typed body. Maps sort by encoded key bytes; floats/tags are forbidden. The descriptor fingerprint is SHA-256 of deterministic CBOR of the complete signed descriptor. Pin the operator key/fingerprint out of band: a self-signed descriptor alone does not establish a trusted operator.

Policy fields: `admission` (`open`/`authorized`), `authorized_sellers` (`ed25519:`-prefixed base64 Ed25519 public keys), `max_offer_ttl_seconds`, `min_listing_value_zat`, `max_active_offers_per_seller`, `max_publications_per_seller_per_window`, `rate_window_seconds`, `confirmed_after_blocks`, `max_clock_skew_seconds`. All fields are explicit; see `BoardPolicy::default` for suggested values. The listing value is a board charge, **not the network fee**. Authorized mode uses an operator-signed allowlist; portable per-seller authorization certificates are future work.

## Read-only discovery is independent of custody

A UIVK lets an indexer decrypt incoming notes and read OFFER memos/values. It
cannot authorize a spend, create a transaction signature, or take control of
funds. Independent indices need the signed public descriptor and chain data,
not wallet authentication or spending keys. The web UI can use an indexer's
HTTP API without holding any viewing key itself.

The board UIVK is intentionally public. A personal seller UIVK is not: sharing
it would disclose that account's incoming activity. Do not export a UFVK or
reuse a personal account for the public board.

## Provisioning

Use a **dedicated** local wallet account (separate from the seller spend account) to derive an Orchard-only UA and corresponding UIVK. The scanner must never receive a spending key, seed, or wallet credential. The first dedicated mainnet board was provisioned locally for the verified discovery test; the CLI does not automatically create or fund production wallets. Do not paste wallet secrets into an agent/chat.

### Human backup and sync (Zallet)

Against released **Zallet `v0.1.0-beta.3`** (`987382f67e622915228686e9f956c6a9c9a7514c`):

1. Confirm the mnemonic backup with **`zallet confirm-backup`** before deriving new accounts or addresses from a wallet-generated seed.
2. Unlock the keystore when calling account or viewing-key RPCs.
3. Wait until wallet sync is ready (`getwalletstatus` not sync-locked) and the backing full node for Zallet is available. Board **scanning** itself only needs public lightwalletd + `board.json`; publication needs the wallet’s chain backend as well (see [mainnet-publishing.md](./mainnet-publishing.md)).

### Supported public Zallet RPCs (beta.3)

After backup confirmation (method templates below; the signed descriptor contains actual public receiving/viewing values, never spending secrets):

1. `z_getnewaccount` — create a dedicated board account (and a separate seller account for later `publish --wallet-account`).
2. `z_getaddressforaccount` with `receiver_types` **`["orchard"]`** — Orchard-only UA. Default/empty receiver lists include Sapling and transparent receivers and are unsuitable for the board address.
3. `z_exportviewingkey` with **`ivk` = `true`** — export the account **UIVK** (`uivk…`). Default `ivk=false` returns a UFVK (`uview…`); do not put a UFVK in the portable descriptor.
4. Capture **`birthday_height` at provisioning time** (for example from `getwalletstatus.node_tip.height`, or tip−1). Beta.3 `z_listaccounts` / `z_getaccount` responses do not expose birthday height; the birthday must be no later than the first publication.

`board-sign` validates network, Orchard activation birthday, operator signature key, Orchard-only address shape, and that the UIVK controls the board receiver (UA/UIVK binding). Indexers need only the resulting public `board.json` plus a compatible lightwalletd URL.

Create a protected local operator identity with `cargo run -p zadp-seller -- identity-init --output operator.identity`. Use `identity-show --identity operator.identity` to obtain its public key. Populate all body fields in `board-body.json` using wallet-derived **public** receiving/viewing information, then:

```sh
cargo run -p zadp-seller -- board-sign --body board-body.json --identity operator.identity --output board.json
```

A database pins the entire descriptor fingerprint; a policy/descriptor change requires a new database and reconstruction. This avoids silently reinterpreting prior admissions.

## Privacy

A public UIVK deliberately makes incoming board output values and memos visible to everyone who scans. It does not authorize spending and is narrower than a full viewing key. Anyone can detect other incoming diversified addresses associated with the same incoming key, even though the ZADP scanner rejects them. Use a dedicated board account, never reuse a personal account. Publications are public discovery content, not a confidential seller inbox. Network-service requests for matching full transactions may reveal scanner interest. Shielding does not hide published OFFER content from descriptor holders.

## Multi-board hosting and external ownership

See [Multi-board indexing](multi-board.md) for the incremental registry, per-board
SQLite isolation, shared CompactTxStreamer transport, read-only external board
registration and independent host moderation. External owners retain seed and
spending authority. The host receives only the signed descriptor/UIVK and does
not become the board owner. Public UIVKs permit independent reconstruction;
privately shared UIVKs are excluded from public board metadata endpoints.

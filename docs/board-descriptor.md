# Portable board descriptor

A board is a dedicated Orchard-only Unified Address. It is not a public block-explorer search. Indexers download compact blocks and trial-decrypt with its **Unified Incoming Viewing Key (UIVK)**. Only the exact receiver in the descriptor is accepted; other diversified addresses under that key are excluded.

The JSON has two top-level fields: `board` and `signature`. `board` contains `zadp`, `board_id`, `name`, `network` (`mainnet` or `testnet`), `address`, `incoming_viewing_key`, `birthday_height`, `policy`, and `operator_key`. Unknown fields are rejected. The signature is base64 Ed25519 over `ZADP1\0board\0` followed by deterministic CBOR of the typed body. Maps sort by encoded key bytes; floats/tags are forbidden. The descriptor fingerprint is SHA-256 of deterministic CBOR of the complete signed descriptor. Pin the operator key/fingerprint out of band: a self-signed descriptor alone does not establish a trusted operator.

Policy fields: `admission` (`open`/`authorized`), `authorized_sellers` (`ed25519:`-prefixed base64 Ed25519 public keys), `max_offer_ttl_seconds`, `min_listing_value_zat`, `max_active_offers_per_seller`, `max_publications_per_seller_per_window`, `rate_window_seconds`, `confirmed_after_blocks`, `max_clock_skew_seconds`. All fields are explicit; see `BoardPolicy::default` for suggested values. The listing value is a board charge, **not the network fee**. Authorized mode uses an operator-signed allowlist; portable per-seller authorization certificates are future work.

## Provisioning

Use a separate local Zcash wallet/account to derive an Orchard-only UA and corresponding UIVK, and record a birthday no later than the first publication. The scanner must never receive its spending key, seed, or a wallet credential. This implementation does not create or fund a production board wallet. Do not paste wallet secrets into an agent/chat.

Create a protected local operator identity with `cargo run -p zadp-seller -- identity-init --output operator.identity`. Use `identity-show --identity operator.identity` to obtain its public key. Populate all body fields in `board-body.json` using real wallet-derived public receiving/viewing information, then:

```sh
cargo run -p zadp-seller -- board-sign --body board-body.json --identity operator.identity --output board.json
```

The command validates the network, Orchard activation birthday, signature key, address type, and that the UIVK can derive the receiving address. Indexers need only `board.json` plus a compatible network service URL. A database pins the entire descriptor fingerprint; a policy/descriptor change requires a new database and reconstruction. This avoids silently reinterpreting prior admissions.

## Privacy

A public UIVK deliberately makes incoming board output values and memos visible to everyone who scans. It does not authorize spending and is narrower than a full viewing key. Anyone can detect other incoming diversified addresses associated with the same incoming key, even though the ZADP scanner rejects them. Use a dedicated board account, never reuse a personal account. Publications are public discovery content, not a confidential seller inbox. Network-service requests for matching full transactions may reveal scanner interest. Shielding does not hide published OFFER content from descriptor holders.

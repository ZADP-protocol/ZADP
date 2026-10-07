# Multi-board indexing and policy boundaries

## Minimum migration

Preserve ZADP/1 memo framing, signing and the existing signed BoardDescriptor.
The descriptor already binds `board_id`, `name`, `network`, `address`,
`incoming_viewing_key`, `birthday_height`, signed board `policy` and
`operator_key`. The Ed25519 descriptor identity is separate from Zcash spending
keys: a signature authenticates the announcement, not proof of spending-key
possession or legal ownership. Trust/pin the expected identity out of band.

A `BoardContext` wraps that unchanged descriptor and an isolated SQLite index.
The existing mainnet board can remain Board #1 with its original database,
address, birthday and descriptor. Legacy `--board` mode remains available.
No live service or wallet migration is required to develop this feature.

A local host registry selects boards and database paths. Multiple contexts use
one CompactTxStreamer connection, not one Zebra node per board. Each context
trial-decrypts with its own UIVK and retains its own cursor/reorg history.
Dedicated UIVKs are required: diversified addresses on one account are not
cryptographically isolated boards. Network mismatch fails before scanning.

## Authority and decentralization

- External owner: seed, spending authority, account control and descriptor identity.
- Indexer: signed descriptor and UIVK only. No wallet/auth/spending-key input.
- Host: registry acceptance and hosted presentation; no ownership transfer.

An intentionally public UIVK lets independent indexers reconstruct the same
board from the same descriptor and chain data. A privately shared UIVK must not
be logged or returned by public board metadata endpoints. It reveals incoming
traffic for its account, not merely offers, so use dedicated accounts. The
scanner additionally restricts accepted outputs to the descriptor receiver.
Removing a board from one host cannot revoke a previously shared viewing key.

## Separate decisions

1. Protocol: decoding, fields, signature, expiry at block time and board binding.
2. Signed board admission: endpoint rules, listing minimum, authorization, TTL,
   active/rate limits. Existing chain-order admission semantics remain unchanged.
3. Host moderation: visible, hidden or quarantined, with a bounded reason.
4. Host registry: active, paused or rejected; hosted or external provenance.

A valid publication rejected by board policy remains distinguishable from an
invalid publication. Host moderation never rewrites raw/canonical admission or
protocol validity. Raw records and moderation are local operator data; the public
API only returns visible, board-admitted, active offers on active boards.

Before opting an existing index into the new binary, make a consistent local
SQLite backup and preserve the prior executable for rollback. Do not change the
wallet or signed descriptor. Existing databases receive additive publication-audit
and moderation tables.
Historical accepted offers are backfilled; old rejected memos cannot be
reconstructed from rejection reasons alone. Rescanning into a fresh database
is needed for complete historical audit; never reset the live index silently.
Offer identity is scoped to the board. Existing `(seller, id)` first-wins behavior
inside each board remains intact; moderation uses that exact seller/id pair.

## Host onboarding fee

An optional hosted-board creation fee is host policy only. Registry approval
records an explicit local operator decision; it is not blockchain payment proof
or production billing. External-board registration does not require spending
keys or hosted-account creation. A missing hosting fee may prevent host registry
activation, but never makes a ZADP publication invalid. This change does not
create accounts, accept payments or automate wallet operations.

## Scale and non-goals

Work grows roughly with shielded outputs times viewing keys. Connection reuse
is implemented; shared block/full-transaction caching and batch decryption are
future optimizations. Correct per-board isolation comes first.

No board-of-boards, reputation, escrow, governance, AI buying, automatic content
moderation, production billing or settlement redesign is introduced.

## Operator configuration and API

Run the existing single-board arguments unchanged, or use:

```sh
cargo run -p zadp-api -- --network mainnet --registry host-registry.json \
  --lightwalletd https://zec.rocks:443 --listen 127.0.0.1:8081
```

The new port above keeps an existing live single-board service intact. A local
registry file uses this shape; paths are relative to that file:

```json
{
  "hosted_policy": {
    "enabled": false,
    "creation_fee_zat": 0,
    "fee_recipient": null
  },
  "boards": [
    {
      "descriptor": "board-a.json",
      "database": "board-a.sqlite",
      "source_type": "external",
      "status": "active",
      "hosting_fee_approved": false
    },
    {
      "descriptor": "board-b.json",
      "database": "board-b.sqlite",
      "source_type": "external",
      "status": "active",
      "hosting_fee_approved": false
    }
  ]
}
```

Only use signed descriptors you are authorized to index. Registry loading rejects
unknown fields, malformed/signature-invalid descriptors, mixed networks,
duplicate board IDs and reused UIVKs. No seed or spending key field is accepted.
External owners provision and sign their own descriptors outside the indexer.
The host does not create accounts or manage keys in this milestone.

Read-only routes:

- `GET /boards`: metadata and each board's cursor/status, without addresses/UIVKs.
- `GET /boards/{board_id}`: one public metadata summary.
- `GET /boards/{board_id}/offers`: visible active offers on that board.
- `GET /offers?board_id=...&capability=...&q=...`: filtered hosted view.
- `GET /offers`: aggregated hosted view, bounded to 1,000 results.
- `GET /healthz`: HTTP process liveness, not proof all boards are caught up.

The legacy single-board `board_router` remains available. The CLI now serves the
multi-board router even when configured with one board; `/offers` retains its
response shape and chain provenance. `q` is a bounded case-insensitive substring
filter over title, capability and description; pagination is future work. The
local UI can filter indexed offers by board and shows the board ID on cards and
details; it does not list boards with no currently visible offers. Its separate
wallet/invoice integration remains pinned to the configured local seller/board.

Host moderation is an explicit **local operator command**, never a public route.
Create a local JSON array of overrides:

```json
[
  {
    "board_id": "board-a",
    "seller": "ed25519:REPLACE_WITH_PUBLIC_SELLER_ID",
    "offer_id": "REPLACE_WITH_OFFER_ID",
    "status": "hidden",
    "reason": "Manual demo host decision"
  }
]
```

Then apply it without scanning or wallet access:

```sh
cargo run -p zadp-api -- --network mainnet --registry host-registry.json \
  --moderation-only --moderation visibility.json
```

`visible` removes the visibility restriction; `quarantined` also excludes the
record from hosted responses. The bounded reason stays in the local moderation
table. Overrides are persistent and keyed by board database, seller and offer ID;
they persist across reorgs and apply if the same identity is re-admitted. There
is no raw publication/audit, moderation-write or descriptor-download HTTP route.
Operator code can inspect `canonical_records` and `publication_audit` locally.

ZADP defines what was validly published. Board operators define what belongs on
their board. Hosts define what they choose to display. Registry rejection and
hosting fees are not protocol consensus.

## Signed geographic offers

The reader accepts Offer/wire v1 and v2 under the existing BoardDescriptor v1.
V2 optional geo is signed public offer-area metadata, exposed as integer e5
fields in GET /offers. Full OFFER JSON persistence requires no schema migration;
existing board fingerprints, addresses, UIVKs and policy remain unchanged.
No buyer coordinates are accepted or stored. Matching occurs in the client.
Older readers reject v2: upgrade before geo publication; if an old reader already
passed/rejected geo blocks, reconstruct separately from the signed birthday.
Never overwrite an active index or silently replay a publication.
[Geo compatibility and demo](geographic-discovery.md).

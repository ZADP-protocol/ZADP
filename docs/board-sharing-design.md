# Board viewing-capability sharing: architecture scope

Roadmap design work, updated 2026-10-17. This is **not an implemented federation
or key-distribution service**, and changes no Protocol 1.0 messages or policies.

## What already works

A pinned, signed BoardDescriptor carries the board network, receiving scope,
incoming viewing capability, birthday and admission policy. An independent
indexer with that descriptor and a compatible chain source can reconstruct
Offers without asking sellers to publish again. Multi-board indexing already
keeps board databases and cursors isolated.

Sharing a dedicated board UIVK grants incoming visibility, not spending power.
It can expose incoming memos/values, including other diversified receivers under
that key; the reference scanner's exact-receiver filter does not remove that
cryptographic visibility. Never distribute a personal wallet UIVK, full viewing
key, seed or spending key. See [BoardDescriptor](board-descriptor.md).

## Design questions to resolve

- **Explicit sharing:** distinguish intentionally public descriptors from private
  invitations. Define who may receive a private descriptor; do not export it
  through public board, search or MCP metadata by default.
- **Discovery and integrity:** choose a simple distribution mechanism for signed
  descriptors, with out-of-band operator-key/fingerprint pinning. A directory or
  download URL alone must not establish trust.
- **Versions and changes:** define how clients recognize descriptor revisions,
  policy changes and replacement boards without silently accepting a new key,
  address, network or birthday. Publishing a UIVK cannot make it secret again;
  removing a download does not revoke copies already held.
- **Independent recovery:** document bounded historical rescan, cursor/reorg
  handling, duplicate identity handling and reconstruction comparisons using
  fresh isolated databases. An indexer's outage must not require republication.
- **Multiple boards:** distribute an opt-in collection of descriptors without
  merging board authority or databases. Each board retains its own signed
  admission policy; operators can additionally apply different hosted visibility
  rules for their jurisdictions. This architecture does not determine legality.
- **Boundaries:** signatures bind an Offer to its board. Sharing a descriptor does
  not copy or rebind Offers into another board or allow an indexer to rewrite a
  signed board policy. Local visibility is separate from on-chain preservation.
- **Abuse and privacy:** bound descriptor counts, fetch sizes, rescan cost and
  network destinations; prevent arbitrary fetch/SSRF through user-supplied URLs.
  Do not distribute secrets or imply confidential public-board contents.

## Acceptance before implementation is called complete

Document the trust/distribution model and migration behavior; demonstrate two
independent indexers reconstructing one shared board; test several boards with
separate policies and hosted views; verify that private descriptors stay out of
public API/MCP responses. No automatic synchronization or remote wallet access
is required merely to share discovery state.

See [roadmap](roadmap.md), [multi-board indexing](multi-board.md) and
[repository boundaries](repository-boundaries.md).

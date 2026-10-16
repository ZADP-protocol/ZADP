# MCP discovery for AI clients

ZADP-MCP is a separate, read-only adapter over **moderated ZADP-Hosted discovery**.
It does not scan Zcash or implement a second protocol, database, wallet or payment
system. Core remains usable without this adapter or the official hosted service.

## Connect

Use Streamable HTTP at **https://mcp.zadp.io/mcp**, with no login for public
read-only discovery. The [client connection portal](https://zadp.io/mcp.html)
contains client-specific setup instructions. This is a protocol endpoint, not a
normal web page; a browser GET may return HTTP 405.

The deployed adapter was activated on 2026-10-15. Dated production acceptance on
2026-10-16 exercised revisions **2026-07-28** and **2025-11-25** through official
MCP Inspector 2.11.0. This record is not a continuous uptime guarantee.

## Tools

- `zadp_search_offers`: optional `query`, `board_id`, `capability` and `limit`
  (1–100, default 20). Returns public Offer summaries and canonical links.
- `zadp_get_offer`: required `board_id` and `offer_id`; optional `seller` to
  disambiguate canonical identity. Returns signed public fields, chain provenance
  and separately identified unsigned host metadata.
- `zadp_list_boards`: no arguments; active public board metadata, without board
  descriptors or incoming viewing keys.
- `zadp_get_network_status`: no arguments; service liveness/network and board scan
  cursors. A live indexer or scan height alone is not proof of chain synchronization.

Example search arguments:

```json
{"board_id":"games","capability":"game.boat-rental","limit":10}
```

Read-only resources cover the protocol overview, supported Offer formats, public
boards and discovery/privacy. Implementation, generated schemas and deployment
runbooks belong to the separate ZADP-MCP repository, not Core.

## Security and limits

Only currently public Hosted results are eligible. Hidden, rejected, underpaid,
expired and revoked Offers must not be recovered through an unfiltered Core
fallback. Search is bounded by the upstream 1,000-result cap and requested result
limit; completeness is not guaranteed. Outages return errors rather than stale
Offer content.

Seller content is untrusted data, not agent instructions. The adapter does not
independently verify canonical signatures, contact seller endpoints, create
orders, publish or pay. Signature validity, chain observation, board admission,
host visibility and endpoint ownership are separate assurances; none guarantees
service quality.

MCP accepts no buyer GPS. It cannot guarantee confidentiality of ordinary network
metadata, including client IPs. No seller or wallet credentials belong in tool
arguments. Paid actions need a separate client, wallet and authorization boundary.

See [mainnet acceptance](mainnet-discovery-mcp-2026-10-16.md),
[architecture](architecture.md) and [privacy](privacy.md).

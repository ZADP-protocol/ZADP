# Mainnet evidence summary

Public milestone summary updated 2026-10-17 (Europe/Helsinki). This is
non-normative acceptance evidence, not a deployment runbook, active inventory
promise or authorization to repeat a transaction.

## Protocol and reconstruction evidence

- **First publication, 2026-10-05:** legacy wire 1 Offer
  `01M46R4A8RQQCTG12FC9XG066S`, tx
  `d85939f3c7f1b993cfa05edeeeb4f7cad9d81fc21663acac233e8cfc52ac723c`,
  height 3507498. Two fresh indices reconstructed the same signed memo/output.
  Listing 1000 zat, exact publication fee 25000 zat. Placeholder endpoint;
  no commerce acceptance follows from this publication.
- **First geo, 2026-10-10 Helsinki:** legacy wire 2 discovery-only Offer
  `01M4H88HGMFHVH2VXA0AFBTM7E`, tx
  `07d7cb70c1bfd2b9bde2e995b20e579c3389050af32c3945ba33f41c96f38ce8`,
  height 3512182. Independent indices matched signature/geo and hosted Nearby
  checks kept buyer location local. [Original acceptance](mainnet-acceptance.md)
  preserves the original seven-day descriptor and exact public geometry.
- **Canonical wire 3 mainnet geo, 2026-10-10:** Tokyo
  `01M4KN0HWXC1YNQT4149PZEVD7`, tx
  `3bfafcf955305b6375cc8233af8d53095fd5fc5448e46582c0c20dceea49fd55`,
  height 3513285; Shanghai `01M4KN0JDXGCEQ33Y64MXY7KHW`, tx
  `dd2c21bf9c7d5459e8e3f40d36a475d99f6eef36e645b53c2ca69105c7d43387`,
  height 3513288. Independent A/B reconstruction matched signed state.
  These are warned live-demo publications, **not buyable inventory**: seller
  order creation remains disabled. They are not relabelled legacy evidence.

## Hosted product evidence

Real signed chain Offers reach the hosted read-only discovery API and globe.
Browser acceptance checked local Nearby matching and no buyer GPS in network
requests/storage. Web-only synthetic fixtures remain clearly marked, unpurchased
presentation data, never canonical index inventory or chain statistics.
The hosted configuration reviewed for that milestone used seven isolated
boards with a 90-day maximum; that is dated instance configuration, not a
Protocol 1.0 duration constant.

## Seller fulfillment evidence

A real human buyer using a separate wallet paid for the deterministic
`text.summarize` reference service, Offer `01M4J8687QJBZHQHEZHDEFVT5Y` (**legacy
wire 1**). Payment tx
`bfdf85f3a2cd22bc739d4ce65e870db3aa8300e429a03eb7ade2c25f021c2952`,
height 3513314. Seller correlated the exact payment, required three confirmations
and reached completed with `execution_count=1`. Service amount was 1000 zat.
The user's wallet screenshot showed total 11000 zat; the 10000-zat difference
is an **inferred fee**, not an independently obtained wallet proposal fee.
No input/result/claim token or private payment memo is reproduced here.

This proves that particular human purchase, not autonomous agent payment,
service quality generally, custody, refunds or delivery atomicity.
Detailed sanitized records remain in the private
[Server human acceptance](https://github.com/ZADP-protocol/ZADP-Server/blob/main/docs/human-mainnet-purchase.md).

## Agent-prepared, human-paid order — 2026-10-11

A fresh order against the legacy `text.summarize` Offer
`01M4J8687QJBZHQHEZHDEFVT5Y` requested 1000 zat. The agent verified the Offer
and seller-origin proof, checked order availability and prepared one idempotent
order. PaymentController required approval; the user paid from their separate
phone wallet. Authenticated seller polling observed
`waiting_payment → paid → processing → completed`, a verified payment and a
returned result (11 confirmations at the first completed observation against
a three-confirmation policy).

This closes agent preparation → human wallet payment → seller fulfillment for
that legacy service. It does not prove canonical wire-3 commerce or autonomous
ReviewedWalletAdapter settlement. Payment txid, actual network fee and
`execution_count` were not exposed by that attempt's checked API and are not
claimed. This supersedes the earlier unpaid-preparation-only milestone without
reclassifying the earlier attempt as paid.

## Real agent-controller wallet purchase — 2026-10-14

PaymentController returned ALLOW under a bounded, human-authorized one-service
policy and the real ReviewedWalletAdapter paid the legacy summary-service
invoice. Service amount 1000 zat plus actual fee 10000 zat; payment tx
`4b8b78fb090d286f383f4a60415cd673a3fabda9ab3108b80b188d1d67fc9f88`.
Seller payment verification, confirmation gating, correct result and
`execution_count=1` were observed. The buyer/seller accounts were separate, but
under the same wallet/operator. See [sanitized acceptance and limits](agent-mainnet-acceptance-2026-10-14.md).
This supersedes the earlier real-wallet-unproved status for that specific path.

## Paid discovery publications and production MCP — 2026-10-16

Three canonical wire-3, discovery-only gaming Offers were published for 90 days
with approximate Miami geography. Each paid 245858 zat for publication plus an
actual 10000-zat network fee; total **0.00767574 ZEC**. These are fictional game
examples, not real rental listings or completed seller purchases.

Official MCP Inspector exercised the production read-only adapter with revisions
2026-07-28 and 2025-11-25: **56 checks passed**, including all four tools,
searches, exact signed fields, provenance, expiry and canonical links. MCP did
not publish or pay. See [public transaction records and acceptance limits](mainnet-discovery-mcp-2026-10-16.md).

## Remaining acceptance boundary

Canonical wire-3 commerce, independent wallet owners and a hardened isolated
agent remain separate acceptance work. Offline settlement, agent-prepared human
payment, real controller-driven purchase and paid Offer publication prove
different paths; none grants unrestricted autonomous spending authority.
See [roadmap](roadmap.md), [MCP](mcp.md) and [privacy](privacy.md).

Detailed operator records, release SHAs, backups and private-host policy belong
to Server. This public summary does not disclose protected configuration or
private incoming viewing authority and grants no permission to publish/pay.

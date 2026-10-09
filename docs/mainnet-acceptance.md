# First live mainnet offer: verified 2026-10-05

The human-authorized offer `01M46R4A8RQQCTG12FC9XG066S` was mined at
height 3507498 in transaction
`d85939f3c7f1b993cfa05edeeeb4f7cad9d81fc21663acac233e8cfc52ac723c`.

Two independent fresh persistent indices scanned from the signed public board
birthday 3507246 and admitted the same signed offer, transaction, Ironwood
output and listing value. The first index served it through `GET /offers`.
At acceptance, the first index observed five confirmations and the second
three. See [public proof](mainnet-acceptance-proof.json).

Actual publication cost: 1,000 zat listing value + 25,000 zat network fee =
26,000 zat (0.00026 ZEC). The endpoint `https://example.com/zadp` is a
placeholder. This verifies publication and discovery, not paid fulfillment.
Do not replay or retry this publication.

The reviewed [Zallet library patches](../integrations/zallet/README.md) were
installed before this acceptance; the existing wallet was preserved.

## First real geographic discovery-only Offer v2

Mined at 2026-10-09T21:15:05Z (2026-10-10 in Europe/Helsinki):

- Title: AI consulting — San Francisco area.
- Description: Demo service for ZADP geographic discovery. Not a real commercial service.
- Offer ID: `01M4H88HGMFHVH2VXA0AFBTM7E`.
- Transaction: `07d7cb70c1bfd2b9bde2e995b20e579c3389050af32c3945ba33f41c96f38ce8`.
- Block: `3512182`; mainnet; Offer v2 / wire v2.
- Board: `ai-services`; original descriptor fingerprint
  `e23cfe406a9a644ec753d023da415ef3e0d0531cb97f9d700deba6e632d969a4`.
- Signed geo: lat_e5 `3777500`, lon_e5 `-12241900`, radius_m `25000`.
- Issued: `2026-10-09T21:14:16Z`; expires: `2026-10-16T21:14:16Z`.
- Memo 512 bytes; payload 312 bytes; publication 1000 zat + exact fee 10000 zat.
- Commerce absent. This demonstrates discovery, not a bike-repair business or purchase.

Two independent indexes reconstructed the same signed Offer and hosted browser
acceptance verified local Nearby matching without GPS network/storage traffic.
Expired active discovery remains excluded normally. The separate hosted historical
proof fixture retains these immutable public facts, never an active Offer or Buy.

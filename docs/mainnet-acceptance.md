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

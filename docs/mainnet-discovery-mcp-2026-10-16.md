# Paid mainnet discovery and MCP acceptance — 2026-10-16

Dated operator acceptance, summarized for public documentation on 2026-10-17.
[Sanitized machine-readable record](mainnet-discovery-mcp-2026-10-16.json).
This is evidence of specific completed publications and read-only discovery,
not an assertion of current inventory, service availability or autonomous buying.

## Publication and payment

Three fictional boat-game Offers were published on the `games` board using
canonical wire 3. Each has capability `game.boat-rental`, an approximate Miami
service-area center (25.76170, -80.19180), a 10 km radius and a 90-day lifetime.
They contain **no service price or seller endpoint**: discovery only, not real
boat rentals, bookings or purchasable seller services.

Each publication paid **245858 zat**, plus an actual **10000-zat network fee**.
Total for all three: **767574 zat (0.00767574 ZEC)**. The hosted reference price
was $3 per 90-day publication at that day's rate; it is historical operator
pricing, not a protocol constant or current quote. Publication payment is not
payment for a boat or an advertised service.

The existing local operator/wallet path prepared each transaction, checked the
human-authorized bounds, broadcast it once and observed it through moderated
Hosted discovery. MCP did not sign, publish, create an order or pay.

### Sunset Cruiser - Demo

- Offer: `01M52HXFAERA512758W6FZ0TB1`
- Transaction: `66b903ca80d401e3a048763d3a8a8360feb2790fc160e3f81b9da7c76dbaf516`
- First indexed block: 3519902
- Signed expiry: `2027-01-14T14:30:04Z`
- [Canonical Offer page](https://zadp.io/offers/games/693df02733de06511630e98c33960fa36973ce095a9a1b1062bcfee5b22ee2b9/01M52HXFAERA512758W6FZ0TB1)

### Island Hopper - Demo

- Offer: `01M52HZXTQEEK9Z3MPDK2J4357`
- Transaction: `56f2c4566436573568fb60067471e4aed471c89ff3f307d38201a9232f8c57cd`
- First indexed block: 3519903
- Signed expiry: `2027-01-14T14:31:25Z`
- [Canonical Offer page](https://zadp.io/offers/games/693df02733de06511630e98c33960fa36973ce095a9a1b1062bcfee5b22ee2b9/01M52HZXTQEEK9Z3MPDK2J4357)

### Reef Explorer - Demo

- Offer: `01M52J62YBGZDYWD2BWJN863QK`
- Transaction: `3c11bfd5053b11fa01b67266f1fef19417a9e686757c261b074b592bdd21af69`
- First indexed block: 3519905
- Signed expiry: `2027-01-14T14:34:47Z`
- [Canonical Offer page](https://zadp.io/offers/games/693df02733de06511630e98c33960fa36973ce095a9a1b1062bcfee5b22ee2b9/01M52J62YBGZDYWD2BWJN863QK)

Canonical pages follow current hosted visibility and expiry rules. A historical
receipt does not require the hosted service to continue exposing an Offer.
No hardcoded historical Offer body is added to the website or MCP fallback.

## Production MCP acceptance

Official **MCP Inspector 2.11.0** exercised all four tools at
`https://mcp.zadp.io/mcp`, using revisions **2026-07-28** and **2025-11-25**.
**56 checks passed**: tool discovery, board/status calls, title/board/capability
searches, seller-scoped exact lookup, unchanged signed fields, transaction
provenance, 90-day expiry, discovery-only eligibility, separate assurance
metadata and canonical page HTTP 200.

Signatures were compared with the locally produced signed Offers. The MCP
adapter itself does not independently verify Ed25519 signatures. Service
liveness was not treated as proof of network synchronization.

One initial Inspector exact-lookup invocation exited unsuccessfully; its stderr
was not retained. The cause remains unknown. A direct repeat and the complete
56-check rerun passed; this is not claimed as a diagnosed/fixed server defect.

No buyer request/result, recovery capability, wallet credential, viewing key or
private deployment configuration is included. No seller purchase, moderation
change or new payment is implied by this documentation update.

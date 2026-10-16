# Privacy and data flow

Current application boundaries, not a claim of anonymity, KYC or end-to-end
order secrecy from the seller. [Protocol](protocol.md) defines signed data;
[threat model](threat-model.md) defines adversaries and residual risks.

## Data locations

| Data | Zcash / board data | Discovery index DB / API | Buyer device | Seller service DB |
| --- | --- | --- | --- | --- |
| Offer title/capability/description | Shielded memo; readable with board incoming capability | Accepted public signed metadata | Downloaded metadata | Approved configured Offer |
| Seller Ed25519 identity | Public key in signed Offer | Public key/signature | Public identity | Public identity only; no private identity key on server |
| Service area | Optional signed public e5 center/radius | Public signed area | Rendered/matched locally | May be in configured Offer; no buyer point inferred |
| Buyer GPS | Absent | Absent | Volatile browser memory only | Absent from reviewed automatic flow |
| Buyer search radius/location | Absent | Absent | Local matching/camera state; no GPS persistence | Not automatically transmitted |
| Order input | Not in payment/discovery memo | Absent | User input sent directly to seller; no browser input persistence | Plaintext in private SQLite |
| Result | Absent | Absent | Returned directly using recovery capability | Plaintext in private SQLite |
| Payment memo | Shielded payment output, not public board Offer; visible to recipient/viewing holders | Absent from canonical discovery DB | Exact seller invoice / wallet request | Private payment correlation |
| Claim/recovery token | Absent | Absent | Private browser recovery storage in hosted flow | Authentication hash/recovery metadata; no public record |
| Spending key | Never a protocol/memo field | Absent | Separate trusted wallet process, never browser/model | Absent from hosted fulfillment |

Buyer Nearby matching needs no coordinate-bearing discovery request. City search,
camera movement and local cartography need no external geocoder/tiles/telemetry.
Coordinates do not enter application URLs, logs or browser persistent storage.
Browser/OS location providers, extensions, compromised clients and voluntarily
supplied delivery details are outside that guarantee. Servers still observe IP,
request timing and ordinary access metadata. Server-side text/capability filtering
may reveal those query terms; local GPS matching is not a full search-anonymity claim.

Public Offer metadata is readable by holders of the board incoming viewing
capability. Shielded transport does not make an intentionally public board
confidential. The capability is not spending authority; a dedicated board account
prevents unrelated personal traffic from becoming an accidental discovery surface.
Public service-area centers can reveal sensitive places, even after rounding.

## Direct seller interaction

Order input, result, claim authorization and payment correlation travel directly
between buyer and seller. The discovery API does not receive them or proxy orders.
Proof fetches receive only a public origin/document; no order/GPS is included.
Explicit capability/order requests reveal buyer network metadata to that seller.
A no-price verified service link offers no ZADP payment flow.

The seller necessarily sees the order input and its incoming payment. Seller
SQLite currently stores input/result privately **in plaintext**. File permissions
and service isolation do not hide them from the seller/operator or its backups.
If discovery and seller share a host/operator, logical API separation does not
protect against that operator inspecting seller state. Do not claim otherwise.

## Retention and recovery

The hosted Seller target is input 7 days, result 30 days and detailed metadata
90 days after terminal transition, plus minimal lifetime replay/idempotency
records and rolling backup limits. This is **target policy, not active deletion**.
Automated redaction/purge, terminal timestamps and result-expiry recovery semantics
still require implementation. Do not promise deletion or erase payment uniqueness
with ad-hoc SQL. See the private deployment's
[Seller retention policy](https://github.com/ZADP-protocol/ZADP-Seller/blob/main/docs/retention-policy.md).

Claim tokens are private retrieval capabilities. Hosted browser storage permits
same-browser recovery; anyone with access to that storage may retrieve the result.
Forgetting local recovery does not cancel or refund payment. Never include tokens,
raw order input/result, wallet credentials or private viewing authority in public
evidence, traces, screenshots, URLs or Git.

## Wallet and policy boundary

Seller Ed25519 identity signs metadata offline. Zcash spending keys remain in
trusted wallet custody, not indexers, seller fulfillment, hosted browser or model.
Agents request payment through PaymentController. Human approval binds exact
invoice/recipient/memo/service amount, actual wallet fee, total, expiry and proposal.
No fee constant, quote, successful test or domain proof authorizes a broadcast.
See [mainnet evidence](mainnet-evidence.md) for human purchases, agent-prepared
human-paid fulfillment, a [real agent-controller purchase](agent-mainnet-acceptance-2026-10-14.md)
and paid publication acceptance. The controller test used separate accounts in
one wallet/operator; wire-3 commerce and independent wallet owners remain separate. The [MCP adapter](mcp.md)
accepts no buyer GPS or wallet authority; ordinary client network metadata is
not guaranteed confidential.

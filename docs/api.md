# ZADP HTTP API reference

Core behavior reviewed **2026-10-15** against main `4a2847461b60eaa02fe416b168039baaf1813bc0`.
This reference describes implemented handlers, not normative signing/wire
semantics or a guarantee about a particular deployment. See [Protocol 1.0](protocol.md).
Hosted moderation and deployment policy live in separate repositories.

## Separate discovery and seller APIs

Discovery reads indexed, signed Offers from Zcash boards. It returns board
metadata, active board-admitted Offers and endpoint ownership status. Optional
Hosted discovery applies its own visibility rules on top. It does not
create seller orders, receive service payments or return service results.

The seller commerce API runs at the advertised HTTPS origin. Buyers send order
input directly there, obtain a Zcash payment request, and retrieve results with a
private claim token. Discovery does not receive order/payment/result payloads.

Synthetic demo inventory is a separate Web fixture layer. It is not returned by
the discovery API and has no signatures, transactions or Buy action.

## Hosted addresses and authentication

- Discovery: `https://api.zadp.io`.
- Same-origin Web discovery proxy: `https://zadp.io/api`.
- Current reference seller: `https://seller.zadp.io`.

The public zadp.io discovery service does not currently require a login.
Independent deployments may require authentication or expose fewer routes. TLS
and access controls are reverse-proxy responsibilities, not Offer fields. Core
does not install HTTP Basic Auth. Bind internal services to loopback.

Seller proof/capability documents and order creation do not require discovery
credentials. Order retrieval/cancellation require their own Bearer claim token.
Never forward discovery credentials to a seller. The reference seller
permits one configured browser Origin; CLI clients may omit Origin. CORS permits
GET, POST, DELETE and Content-Type, Authorization, Idempotency-Key headers for
that Origin.

## Discovery routes

The multi-board Core router implements:

- `GET /healthz`: JSON process liveness.
- `GET /boards`: `{"boards": [...]}` public metadata.
- `GET /boards/{board_id}`: one board metadata object.
- `GET /boards/{board_id}/offers`: `{"offers": [...]}` for that board.
- `GET /offers`: `{"offers": [...]}` aggregated active canonical view.
- `GET /offers/{id}`: one active board-admitted Offer object.

**Hosted distinction:** these are the standalone Core routes. Hosted operators
choose their proxy allowlist and must apply the same visibility policy to all
public Offer routes. Report intake and private moderation are separate Hosted
functionality, not Core discovery writes. Use the [hosted API guide](https://zadp.io/api.html)
for the official service's exposed contract.

### Filtering and limits

`/offers`, `/boards/{board_id}/offers`, and `/offers/{id}` accept:

- `board_id`: exact board ID; unknown boards return `404`.
- `capability`: exact capability filter.
- `q`: case-insensitive substring across title, capability and description;
  maximum 256 UTF-8 bytes.

Unknown query fields return `400`. Conflicting path/query board filters return
`400`. `q` is text search, not geographic search. Aggregated lists are capped at
**1,000** results. There is no pagination, cursor, count/total field, ordering
contract or `limit` parameter. A truncated result may omit matching Offers.
Exact Core ID lookup runs before this cap; an ID shared across boards requires
an unambiguous board filter.

Core returns active boards and unexpired, board-admitted Offers. Hosted adds
its own visibility/payment rules before public results.
Expiry is enforced at `now >= expires_at`. Expired records may remain in canonical
storage; these routes are not a history API. Host approval/visibility and protocol
validity are separate. Endpoint downtime does not remove a visible discovery Offer.

Read-only public discovery example (add credentials only if your chosen host requires them):

```sh
curl --fail --silent --show-error \
  'https://api.zadp.io/offers?board_id=ai-services&capability=text.summarize'
```

### Health response

`/healthz` returns `{"status":"ok"}` and optionally `node_version` and
`lightwalletd_version`, when public backend versions are configured. This is
liveness, not proof of connectivity, wallet readiness, compact-cache readiness
or synchronization to the independently observed public network tip.

### Board metadata

Each board contains `board_id`, `name`, `network`, `owner_identity`,
`descriptor_fingerprint`, `birthday_height`, `max_offer_ttl_seconds`,
`scanned_height` (integer or null), `source_type` (`hosted`/`external`) and
`status` (`active`/`paused`/`rejected`). Optional `display_name` and `description`
are unsigned host presentation metadata.

No receiving address, incoming viewing key, wallet account or private config is
exposed here. `/boards` can include non-active registered boards, whose Offers
are excluded from active discovery. Board names and inventory are deployment-specific; obtain them from `/boards`.
Clients cannot register/create boards through HTTP.

## Offer response model

Each list item flattens signed Offer fields alongside unsigned index/host metadata;
it is not wrapped in an `offer` property. Core `/offers/{id}` returns the same
object without the list envelope.

Protocol 1.0 uses canonical model/wire **3**. New publication uses only 3;
pre-release legacy 1/2 remain readable without rewriting their signed data.
The production feed includes historical wire 1/2 and canonical wire-3 mainnet
Offers. [Evidence](mainnet-evidence.md) distinguishes those milestones.

Signed fields include `zadp`, `type` (`offer`), `id`, `seller`, `board_id`,
`capability`, `title`, optional `description`, `issued_at`, `expires_at`, `nonce`,
`signature`, optional `geo`, and version-specific commercial fields. Offer times
are UTC RFC 3339 strings. Seller identity uses `ed25519:...`; nonce/signature use
the protocol encodings. See [Protocol 1.0](protocol.md) for canonical/wire rules.

Wire-3 primitive excerpts (not complete signed Offers):

```json
{"zadp":3}
```
```json
{"zadp":3,"commerce":{"endpoint":"https://seller.example"}}
```
```json
{"zadp":3,"price":{"amount":"0.01000000","currency":"ZEC"},"commerce":{"endpoint":"https://seller.example"}}
```

No endpoint means discovery only. Endpoint without price enables domain ownership
verification and, when verified, a service link without payment. Price requires
endpoint, with current proof and supported capability/host policy before Buy.
Money is decimal string/ZEC only. Legacy wire 1 is flat price/endpoint, wire 2
nests both in commerce; neither legacy encoding is reinterpreted.
Unsigned API fields `has_endpoint`, `has_fixed_price`, `purchase_available`
are derived from signed primitives; no marketplace mode field is emitted. Availability is false unless signature, activity, board
admission, visibility, proof and supported reference order capability all pass.
See [wire-3 cleanup decision](commercial-modes-v3.md).

### Geo and buyer privacy

Optional signed geo uses integers:

```json
{"geo":{"lat_e5":3777500,"lon_e5":-12241900,"radius_m":25000}}
```

Divide lat/lon by 100,000 for degrees. This is a seller-declared approximate
service-area center, not a verified seller position. There are no latitude,
longitude, radius or Nearby query parameters, nor `/offers/search`. Buyer GPS and
matching remain in browser runtime memory, never in API requests, URLs, logs or
persistent buyer-location storage.

### Chain observations

Optional `chain` contains `network`, `board_id`, `block_height`, `block_hash`,
`transaction_id`, `pool`, `output_index`, `listing_value_zat`, `scanned_height`,
`confirmations`, `confirmation_status`, `offer_status`. Confirmation counts are
relative to the indexer's scanned height, not an independently verified public
tip. This metadata is not seller-signed. `listing_value_zat` is the observed board
publication output value, not the service price.

### Endpoint ownership status

`endpoint_verification.status` is one of:

- `not_applicable`: endpoint absent; no proof fetch or purchase.
- `unverified`: pending, invalid or mismatched proof; Buy disabled.
- `unavailable`: fetch/service failure; Buy disabled.
- `verified`: valid proof from the matching seller and origin.

Optional fields: `origin`, `expires_at` (proof expiry), `valid_until` (short host
attestation lease). These are unsigned, changeable hosted metadata.
Verification is asynchronous; the first response may say `unverified`. Checks
refresh at approximately 60 seconds, with at most 512 cached origins and four
concurrent fetches. Discovery does not wait for sellers. Saved verification is
not an indefinite authorization to buy.

The only proof fetch is `<normalized HTTPS origin>/.well-known/zadp`, derived
from signed visible commerce Offers. Public-address/DNS protections, pinned DNS,
no redirects/proxies, bounded timeouts and a 16 KiB proof limit apply. There is no
general-purpose URL-fetch API. See [endpoint ownership](endpoint-ownership.md).

## Seller commerce routes

These belong to the seller origin, **not** `api.zadp.io` or `/api`:

- `GET /healthz`: text `ok`, liveness only.
- `GET /.well-known/zadp`: offline-signed endpoint proof; `404` if absent/expired.
  Fields: `zadp:1`, `type:"commerce-endpoint"`, `seller`, normalized `origin`,
  UTC `issued_at`, `expires_at`, `signature`. Signing domain:
  `ZADP-ENDPOINT-PROOF/1\0`. The server need not hold the seller identity key.
- `GET /capabilities`: unsigned seller service metadata. Fields: `zadp_service:1`,
  `offer_id`, `capability`, `content_types`, `request_schema`, `response_schema`,
  `transport`, `limits`, `semantics`, `media`, `authority`. This is not a new signed
  protocol descriptor. Reference services are `text.uppercase` and deterministic
  extractive `text.summarize`; the current hosted seller serves `text.summarize`,
  without an external LLM.
- `POST /orders`: create/recover an idempotent order.
- `GET /orders/{id}`: authenticate, reconcile payment and retrieve state/result.
- `DELETE /orders/{id}`: cancel before accepted payment; no refund/transfer reversal.

### Order creation

Headers: `Content-Type: application/json`,
`Idempotency-Key: <random 64 lowercase hexadecimal characters>`.

```json
{"offer_id":"<discovered Offer ID>","request":{"text":"Text supplied directly to the seller."}}
```

Only these fields are accepted. Input must be non-empty and at most 4,000 UTF-8
bytes; HTTP body limit is 8,192 bytes. The same key/request recovers the same order;
the same key with different content returns `409`. Preserve the key privately for
recovery, never in URLs/logs. Reference limits: 100 active and 10,000 total orders.
Creation writes seller state; it is not a read-only discovery request.

### Order responses and authentication

Creation, checking and cancellation return:

- `order_id`, `offer_id`, `status`, `network`, `simulation`.
- `amount_zat` (integer), `amount_zec` (eight-place string), `recipient`, `memo`.
- `payment_request` (ZIP-321 URI), `payment_qr_svg`.
- `expires_at` (**Unix seconds**, unlike Offer/proof RFC 3339 timestamps).
- `confirmations_required`, `confirmations`, `payment_check`, `transitions`.
- `claim_token`: creation/idempotent recovery only.
- `result`: completed order with freshly verified payment evidence only.

GET/DELETE require `Authorization: Bearer <claim_token>`. Unknown orders and
incorrect/missing tokens share `401`. Send tokens only to the verified seller
origin. The reference browser generates QR locally from a validated URI and
ignores seller-supplied SVG.

Validate Offer/order binding, network, recipient, amount, memo and expiry before
payment. The invoice amount excludes the buyer wallet's network fee. An order or
QR does not authorize broadcast: the human or explicit payment policy approves
a separate wallet action.

### Payment and execution

Normal polling: `waiting_payment → paid → processing → completed`. Other terminal
states: `expired`, `cancelled`, `failed`. Transition history starts with `created`.
Reference execution advances on authenticated GET polling, not an independent
unattended worker or webhook.

`payment_check`: `not_checked`, `observer_unavailable`, `waiting_payment`,
`confirming`, `payment_evidence_changed`, `payment_already_used`, `verified`.
A 200 response can report observer unavailability; HTTP success does not prove
payment. Observation requires exact amount/recipient/memo, intended seller
account, non-change shielded output, mined evidence and required confirmations.
One output cannot settle two orders. Result release stops if current evidence
cannot be verified, even if stored status was previously `completed`.

Payment window ends at the earlier of configured lifetime and signed Offer expiry.
Evidence mined within the inclusive window can wait longer for confirmations;
expired/cancelled orders do not reopen. Deterministic reference execution/result
storage is atomic; this does not establish exactly-once external side effects for
arbitrary future services.

## Errors and absent routes

Application errors normally use `{"error":"..."}`. Discovery uses `400` for
invalid/conflicting filters, `404` for unknown/invisible items, `500` with a generic
message for internal failures. Its missing-item message currently says `Unknown
board`, including missing Offer lookup; do not depend on that wording. Framework
parsing and reverse-proxy errors may be text instead of this JSON shape.

Seller errors include `400` invalid/unavailable creation, `403` wrong Origin,
`409` idempotency/cancellation conflict, `401` claim authentication and `503`
check failure. Oversized bodies return `413`; invalid JSON/content type produces
framework-level errors. Error text never constitutes a payment/signing instruction.

No hosted wallet, payment, order/result, board-creation, publication, renewal,
revoke/update, history, messaging, escrow or refund routes exist. Local operator
publication tooling is a separate protected boundary. The older Rust mock/demo
router has `/v1/invoices` and `/v1/fulfill`; these are not hosted discovery or
reference seller order routes.

## Maintenance and verification

Update this snapshot when handlers, serializers or proxy allowlists change:

- [Discovery handlers](../crates/zadp-api/src/boards.rs) and
  [query/response models](../crates/zadp-api/src/lib.rs).
- [Board metadata](../crates/zadp-indexer/src/registry.rs) and
  [chain observations](../crates/zadp-indexer/src/repo.rs).
- [Endpoint verifier](../crates/zadp-api/src/endpoint_verification.rs).
- [Seller handlers](../crates/zadp-commerce/src/http.rs),
  [order state/model](../crates/zadp-commerce/src/lib.rs),
  [capabilities](../crates/zadp-commerce/src/capability.rs).

The earlier authenticated deployment snapshot is superseded. Handler-level
behavior should be verified with offline repository tests; current hosted
operation and acceptance records belong to Server. Related documents:
[multi-board](multi-board.md), [commerce](commerce.md),
[commercial modes](commercial-modes-v3.md),
[process boundaries](commerce-process-boundaries.md).

## Evidence and privacy

The checked-in [mainnet evidence](mainnet-evidence.md) records human purchase,
agent-prepared human-paid fulfillment, a real agent-controller wallet purchase
with stated same-operator limits and paid discovery-publication milestones.
[Production MCP acceptance](mainnet-discovery-mcp-2026-10-16.md) separately records
read-only discovery. Unpaid preparation is not fulfillment, and publication
payment is not a seller service purchase.
`purchase_available` is host eligibility: Web additionally checks fresh seller
`orders_available`, supported transport and unchanged signed terms before POST.
No discovery field authorizes wallet spending. See [privacy](privacy.md) and
[mainnet evidence](mainnet-evidence.md).

## Additive close control

See [offer_revoke](offer-revoke.md) for signed control framing, exact preview/approval,
read-only seller lifecycle binding, HTTP 410 and reorg behavior. Active discovery
excludes accepted revocations; original Offer bytes remain in canonical history.
This API description is not evidence of a mainnet revocation or authority to
perform one. Current activation and transaction records belong to the operator.


## Discovery client and aggregate budgets (not protocol rules)

Discovery defaults: 5 requests/sec per IPv4 address or IPv6 /64, burst 30;
aggregate 100/sec, burst 200. Environment variables:
`ZADP_API_RATE_PER_SECOND`, `ZADP_API_RATE_BURST`, `ZADP_API_CLIENT_RATE`,
`ZADP_API_CLIENT_BURST`, `ZADP_API_CLIENT_PERIOD_SECONDS` (default 1).
`/healthz` is exempt. Exhaustion returns 429 JSON `{"error":"rate_limited"}`,
`Retry-After` seconds until refill and `Cache-Control:no-store`.

Direct mode (default) uses the actual socket peer and ignores all forwarded
headers. `ZADP_TRUST_LOOPBACK_PROXY=true` uses one strictly parsed
`X-Zadp-Client-IP` only from a loopback socket peer. Caddy MUST overwrite that
header with `{remote_host}` on every discovery/report proxy. Missing, repeated,
malformed or non-loopback proxy identity returns 403 without exposing its value.
Ordinary X-Forwarded-For never selects a bucket. Local untrusted processes are
outside this trust model; loopback itself does not authenticate Caddy. A CDN
requires a separately reviewed edge trust policy; none is inferred automatically.

`ZADP_RATE_MAX_CLIENTS=10000` bounds the map; new identities receive 429 when
full. `ZADP_RATE_IDLE_SECONDS=600` expires idle entries with lazy minute sweeps.
IPv4-mapped IPv6 is normalized to IPv4. IPs are RAM-only, never added to logs,
responses or databases. Active clients can retain a bucket; process restart
clears all budgets. No persistent IP banning. Shared Wi-Fi/NAT shares a budget;
limits are configurable. Aggregate protection is still needed for distributed
abuse, and each replica has independent state. A rejected client bucket does
not consume aggregate tokens.

The generic `from_env_with_defaults` helper lets other HTTP adapters define their
own budget prefixes and defaults. Report policy and defaults remain exclusively
in ZADP-Hosted; Core introduces no report/moderation endpoints or policy.
Configuration is validated before serving. Routers embedded without socket
ConnectInfo share an unknown-client bucket; trusted proxy mode fails closed.

Deployment must pair the new binary, explicit proxy configuration and matching
Caddy header overwrite. Do not enable proxy trust with an old Caddy config.
Production is not changed by this implementation. Aggregate 100/sec is an initial
ceiling, not a measured capacity guarantee. Seller payments/orders are untouched.

The standalone Core discovery API has no report or administrator routes. Optional
`POST /reports`, private moderation controls and hosted visibility policy belong
to [ZADP-Hosted](https://github.com/ZADP-protocol/ZADP-Hosted).

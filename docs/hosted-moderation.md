# Hosted moderation and abuse intake v1

This is host presentation policy, not Protocol 1.0 admission policy. Offer wire 3,
legacy wire 1/2, signatures, memo bytes, BoardDescriptor and payment authority
are unchanged. Normal valid Offers are automatically visible under the existing
board and seller host policy. Chain ingestion stores valid Offers even when the
host rejects their content. Revocations and expiration remain independent.

## Deterministic rules

Apply to title, description, capability and signed endpoint text:

- `approved`: structurally/protocol/board-valid, no signal below.
- `rejected`: recognizable email address; grouped international telephone with
  explicit `+` prefix and 9–15 digits; ID explicitly labelled SSN/social security,
  passport, national ID or government ID followed by `:`/`#` and an identifier.
  Listing text excludes all email addresses, including business mailboxes: use
  the HTTPS service endpoint for contact instead.
- `rejected`: case/whitespace-normalized explicit phrases maintained in
  `moderation.rs`: “stolen credit card numbers for sale”, “buy stolen passwords”,
  “child sexual abuse material for sale”. These narrow rules are not a complete
  illegal-content classifier.
- `pending_review`: labelled local contact number or obfuscated `(at)`/`[at]`
  email; third and later equivalent listings by the same seller in 24 hours.
  Equivalence ignores case/whitespace in text but includes price, endpoint and geo.
  Ordering is host-deterministic `(height,txid,pool,output_index)`, not a change
  to protocol chain ordering. All outputs in a block are stored before assessment.

There is no name/home-address classifier, fuzzy spam classifier, subjective
suspicion score or AI. Bare numeric product codes, prices, hex txids and Zcash
addresses are not contact signals. Unformatted phone numbers, disguised contact
information, other languages and prohibited content outside these explicit
patterns can evade detection. Reports and human review cover these limitations.
The browser consumes Core's `client/browser/moderation.js`; both implementations
run the same outcome fixtures. Client validation is assistance, not authority.

## Persistence and reorgs

SQLite migrations run transactionally on repository open. Existing canonical
Offers get automatic assessment before public reads; neither JSON nor history
is rewritten. `automatic_moderation(seller,id,outcome,signals,rule_version)` has a
cascading FK to the canonical Offer. Reorg removes orphaned assessments; replay
reconstructs them. `host_moderation` remains separate and unchanged in shape.
An explicit visible/restore decision can override an automatic false positive;
hidden/quarantined overrides suppress it. Board/seller host-policy restrictions
still apply afterward and cannot be bypassed by restoration.

`offer_reports` stores report_id, offer_id, board_id, seller (target identity),
reason, explanation, creation time and open/resolved/dismissed status.
`moderation_audit` appends timestamp, target, action and operator reason for
visibility and report-processing decisions. Reports and operator decisions
survive reorgs for traceability; an orphaned Offer is unavailable for review until
it reappears. Reports never alter canonical admission or enter Zcash.

## Public API

`POST /reports` JSON (4096-byte limit):

```
{"board_id":"hello","offer_id":"...","seller":"ed25519:...",
 "reason":"spam","explanation":"Optional, at most 2000 UTF-8 bytes",
 "idempotency_key":"client-generated-uuid"}
```

Reasons: `illegal_content`, `scam_fraud`, `personal_information`, `spam`, `other`.
Idempotency keys: 16–80 ASCII alphanumeric/hyphen characters. Exact retries return
one receipt; key reuse with different content is rejected. Only an active,
publicly visible target is accepted. Public receipt contains just report_id and
`received`; neither explanations nor reporter identifiers are exposed. No IP,
cookie or reporter identity is persisted. No report-count takedown is performed.

Public intake uses a separate shared budget of 1 request/second, burst 5, inside
the existing global discovery budget (default 10/second, burst 30). Forwarded
headers cannot bypass either bucket. Storage is bounded to 50,000 reports per
board and 100 open reports per target, including all statuses in the total cap.
No automatic purge is performed. Operators must review/archive before the cap;
a flood can temporarily exhaust intake, but cannot hide Offers automatically.

Every public hosted listing, board view, search and exact lookup uses the same
repository visibility predicate before response creation. Hidden/rejected/pending
records are excluded and exact lookup gives 404. Public responses are no-store.
Prior downloaded data cannot be recalled from a reader's device.

## Private operator interface

Disabled by default. Explicitly start `zadp-api` with
`--moderation-admin-listen 127.0.0.1:18081` and supply
`ZADP_MODERATION_ADMIN_TOKEN` from a protected environment file (32–256 printable
characters). Non-loopback bind is rejected. Never route this listener via Caddy.
Use an SSH tunnel and open its local `/` page. Enter the token in the password
field; it stays in page memory and is sent only as a Bearer header. No local
storage, URL token, cookie or CORS authorization. All data reads and writes
require the token; the empty HTML shell contains no private data.

`GET /review`: bounded queues, pending/open first; `GET /offer` with board_id,
seller,offer_id reviews a specific historical target. `POST /decisions`: target,
action (`approve`, `hide`, `restore`, `reject`) and nonempty reason.
`POST /reports`: board_id,report_id,status (`resolved`, `dismissed`),reason.
Unauthorized requests return 401. An operator can temporarily hide a concrete
high-risk report during review, restore it, or reject it; no report total drives
this action. Existing local moderation-file CLI and root-managed host-policy
commands remain supported and take precedence where configured.

Keep private endpoints away from public ingress. Back up all board databases
including WAL via the existing SQLite-aware backup tooling before migration.
Reports contain private explanations and need restricted backup access and an
operator-approved retention/archive process. No reporter data is public.

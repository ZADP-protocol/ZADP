# Smallest paid service milestone

## What exists and what remains

Real discovery/publication has [live mainnet evidence](mainnet-acceptance.md).
`services/text` already implements private durable invoices, authenticated
read-only seller payment observation and deterministic first-three-sentence
fulfillment. Tests use synthetic transactions. The UI can prepare instructions
and check the local gate; it has no buyer signing/broadcasting route.

A new discoverable service OFFER, public deployment or an explicitly agreed
local-only demonstration, buyer transaction preview/policy approval and actual
shielded payment/result acceptance remain pending. Do not replay the existing
placeholder OFFER. This document is a design, not spend authorization.

## Buyer state machine

`DISCOVERED -> OFFER_REVIEWED -> INVOICE_RECEIVED -> PROPOSAL_PREVIEWED
-> POLICY_CHECKED -> HUMAN_APPROVED -> SUBMITTED -> CONFIRMING -> RESULT`

Side states: `DENIED`, `EXPIRED`, `UNKNOWN_SUBMISSION`, `OBSERVER_UNAVAILABLE`,
`PAYMENT_MISMATCH`, `MANUAL_REFUND_REVIEW`. Unknown submission must never trigger
an automatic second spend; persist/query its txid first. Human approval binds
network, seller receiver, exact amount/memo, service input hash, invoice expiry,
verified offer/board fingerprint and the library-derived buyer fee/total.
Approval becomes invalid when any bound field changes.

## Seller state machine

`OPEN -> PAYMENT_OBSERVED -> CONFIRMED -> FULFILLED`

The current endpoint computes this from authenticated wallet observations at
retrieval time rather than maintaining every intermediate enum. Mempool or
insufficient confirmation evidence stays `awaiting_payment`. Reorg/missing
payment evidence withholds later retrieval even after a result was persisted.
Previously delivered information cannot be recalled after a reorg.

## Current experimental API and data

These APIs are separate from the Rust mock commerce objects and board discovery
API; they are not a standardized ZADP/1 purchase wire format.

1. `GET /offers?capability=text.shorten`: select and independently verify the
   unexpired signed OFFER, trusted descriptor and chain provenance. The UI
   currently trusts its local indexer's signature acceptance, not browser crypto.
2. `POST /v1/invoices` with `{ "offer_id": "...", "text": "..." }`:
   returns `invoice_id`, random `access_token`, `offer_id`, integer `amount_zat`,
   `currency`, `pay_to`, exact text `memo`, Unix `expires_at`,
   `confirmations_required`, `status`.
3. Buyer controller prepares an **unsigned** wallet proposal using that exact
   receiver/value/memo. No such buyer controller is implemented yet. Preview
   inputs/outputs/change/privacy/network fee/total through supported wallet APIs,
   never an invented fee constant; enforce ceilings and explicit human approval.
4. A separate wallet signs/broadcasts after approval. No spending key enters an
   AI process, browser, indexer or seller web endpoint.
5. `POST /v1/fulfill` with only `invoice_id` and `access_token`: authenticated
   seller observer checks payment; result contains `status`, and on fulfillment
   `invoice_id` and `output`. Client-supplied payment evidence is rejected.

The local UI uses `/api/purchase/invoice` and `/api/purchase/fulfill` through a
fixed loopback HTTPS bridge. It checks invoice receiver, offer identifier and
amount against protected configuration. It does not fetch arbitrary advertised
URLs or follow redirects. Invoice access stays only in browser memory and is
lost on reload; secure buyer receipt/session recovery is a remaining design task.

## Correlation and idempotency

- Memo `ZADP-PAY/1:<invoice_id>` uses a random 128-bit invoice identifier. This
  is a text payment memo, distinct from the binary discovery OFFER memo.
- Store exact integer zatoshis, seller account/receiver, input, creation/expiry,
  token hash, deterministic result and unique `txid:pool:output_index` privately.
- Require an exact non-change shielded output to that account/address/value/memo,
  mined within the 30-minute invoice window and at least three confirmations.
- One payment output cannot satisfy two invoices. Repeat fulfill calls return
  the same persisted result after evidence revalidation. Invoice creation is
  **not idempotent**: retrying creates another invoice. Before deployment add
  an authenticated idempotency key and recovery semantics; do not auto-pay both.
- The observer refuses unsynchronized/changing tips or a truncated 1,000-item
  history. It fails closed if required address/memo fields are absent.
- The local service limits active invoices to 100 and total stored invoices to
  10,000; reaching storage capacity needs operator review, not automatic deletion.

## Failures and safety acceptance

Wrong network, receiver, amount, memo, change/transparent output, insufficient
confirmations, late mining, invalid token or inconsistent evidence must not
release output. Underpayments/overpayments are not aggregated and incorrect or
late payments are not automatically refunded. Block timestamps are consensus
metadata, not exact arrival clocks; the next iteration needs a reviewed expiry
and refund policy before public commerce.

The current service checks receiver prefix at configuration time and exact string
identity in observations. A future buyer must cryptographically decode the UA,
check network and supported shielded receiver before proposing payment; prefix
checking alone is insufficient. Production also needs buyer authentication,
anti-abuse, retention/deletion policy, transaction pagination/recovery and a
publicly trusted HTTPS endpoint. Do not advertise loopback as publicly reachable.

Run offline tests first. For the first real test, review a fresh OFFER publication
preview, then separately approve the buyer transaction preview. Record txid,
three confirmations, exact seller evidence and deterministic output without
publishing private invoice input/token, seller viewing authority or wallet data.
Shielding protects payment details from parties without viewing authority; the
seller sees its incoming payment, and endpoints may still learn IP/request metadata.

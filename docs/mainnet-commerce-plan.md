# First independent mainnet commerce payment — preparation only

Baseline: multi-board `05975d2`; local commerce `c50c1e3`. The independent-process
HTTPS acceptance is offline, using synthetic testnet evidence. The earlier live
mainnet OFFER discovery and local self-payment are separate milestones; neither
proves a paid independent buyer-to-seller uppercase service.

## Preserve the current deployment

Keep the existing mainnet board, signed descriptor, seller identity/accounts,
receiving addresses, birthdays, wallet and indexer unchanged. Do not create or
fund a real external board. Do not republish the expired placeholder offer or
retry previous transactions. Wallet synchronization alone is not authorization
for any payment. No fee amount in this plan is a wallet-generated quote.

## Current human direction

No permanent public HTTPS seller origin has been selected. Keep the endpoint
configurable and continue local operation. Do not publish a new mainnet OFFER
with a placeholder, loopback URL or temporary test endpoint. Choose the real
HTTPS origin only when the seller is ready for external access.

The buyer must use a separate existing wallet/account from the seller/board
wallet. In particular, do not reuse the seller wallet as the buyer or count its
self-payment/change as independent commerce. Private buyer wallet configuration
belongs in a local payment controller, never in an order payload or discovery.
Do not create, expose or request seed/spending keys. Until the buyer wallet is
configured, stop at a non-broadcast payment request/QR or preview preparation;
never invent a wallet fee quote. No broadcast without explicit approval.

## Information still required

- The intended permanent seller HTTPS origin, reachable by the independent buyer, with
  valid TLS and browser CORS. Local test certificates are not production TLS.
- A separate existing buyer wallet/client and its payment-controller boundary,
  independent of the existing seller/board wallet.
  No seed, spending key, password or auth file is needed in chat or discovery.
- The exact small service price to include in the new active signed OFFER and
  subsequent private order. No real service price or network fee is approved by
  running the mock test. A funded synthetic fixture address must never be used.

## Manual checklist: one seller, one offer, one tiny purchase

Preparation only. An operator executes these steps later, in order, with explicit
approval at the publication and payment gates. Check off a step only when its
exit condition is verified. Do not run a transaction command as part of this
checklist review.

### 1. Deploy the seller commerce service behind HTTPS

- [ ] Select a permanent HTTPS origin; never substitute a placeholder, loopback
  or temporary test origin in a new mainnet offer.
- [ ] Verify the existing board descriptor, seller receiver and public sync
  status. Preserve existing wallet identities, accounts, birthdays and addresses.
  Make a protected consistent SQLite backup before any runtime change.
- [ ] Prepare an owned/private seller config and a separate orders database
  outside the repository. Use the existing seller account/receiver, mainnet,
  the agreed tiny price, exact HTTPS UI origin and authenticated read-only Zallet
  observer. No board UIVK or board account is a seller payment observer.
- [ ] Review deployment before activating it. Run `zadp-commerce --config
  /private/seller.json --bind 127.0.0.1:8094` behind a TLS reverse proxy. The
  origin must expose the reference API at its root; non-root base paths are not
  supported. Require valid TLS, exact-origin CORS and order rate limits.
- [ ] Disable logging of Authorization/Idempotency-Key headers, request/response
  bodies and private order URLs. Keep seller storage separate from discovery.
  Health checks establish liveness only; verify observer readiness separately.

Exit: independent buyer can reach the intended seller over valid HTTPS, with no
mock observer. No new board or wallet is created or funded.

### 2. Publish a new mainnet offer pointing to that endpoint

- [ ] Prepare one active signed OFFER with real endpoint, seller identity,
  capability `text.uppercase`, agreed exact price and suitable expiry.
- [ ] Validate current signed board policy, OFFER signature and encoded memo
  budget. Prepare the unsigned CLI publication fee preview.
- [ ] Obtain explicit approval of that concrete publication before any signing
  or broadcast. Do not reuse approval of an earlier placeholder publication.
- [ ] After the approved publication, verify mining, policy/signature acceptance
  and matching endpoint/price/provenance in discovery. Never retry automatically.

Exit: the new active offer is accepted and discoverable. This step is deferred;
no new offer was published during preparation.

### 3. Create an order from a separate buyer wallet/session

- [ ] Use an independent browser profile or CLI recovery file. Verify locally
  that the intended payment controller uses a separate existing buyer wallet/
  account, not the seller/board wallet. Never create/request/expose seeds or keys.
- [ ] Discover the offer, select **Buy directly from seller**, enter `hello world`,
  and prepare one order directly at the advertised HTTPS seller endpoint.
  Preserve the private recovery capability. The order client itself is not a wallet.
- [ ] If no separate buyer wallet is configured, stop at step 4. Do not reuse
  the seller wallet. After an interrupted creation, recover the same order with
  the same idempotency key and input rather than creating another purchase.

Exit: one private order exists; discovery received only discovery requests.

### 4. Show the ZIP-321 payment request / QR

- [ ] Verify **REAL MAINNET PAYMENT REQUEST**, exact seller receiver, agreed
  amount, unique order memo, expiry and required confirmations.
- [ ] Display the complete ZIP-321 URI and its QR. Scan/import it in the separate
  buyer wallet and verify recipient, amount and memo match. An address-only QR
  is insufficient. Keep request/receipt data private.
- [ ] Have the buyer wallet prepare an unsigned fee preview when available.
  Until then, show the request/QR only; never invent a network fee quote.

Exit: request fields are verified, no payment sent. A simulation QR must never
be funded; a testnet request is not a mainnet payment.

### 5. Human approves the tiny mainnet payment

- [ ] Present exact service amount, wallet-derived network fee, total cost,
  receiver, memo and network to the human. Confirm the order is still payable.
- [ ] Obtain explicit approval for that one concrete buyer-to-seller payment.
  The buyer wallet/controller may then sign and send it once. The UI, seller
  observer and discovery host do not sign or send transactions.
- [ ] Keep the receipt locally. If sending status is uncertain, check the original
  transaction; do not send a replacement automatically or pay the order twice.

Exit: one approved independent buyer payment has been sent. Approval of this
preparation is not approval to execute this step.

### 6. Seller detects payment

- [ ] Poll **Check seller order**, or use the buyer CLI with the saved recovery
  file. The current service reconciles on authenticated polling, not a background
  worker. Seller observer verifies exact account, shielded receiver, non-change
  output, amount, memo, mining window and configured confirmations.
- [ ] Observe confirming first when below the threshold. No result is released
  on a mismatched, late, reused, reorganized or unavailable payment observation.

Exit: authenticated seller evidence is verified and the order reaches PAID.

### 7. Service executes once

- [ ] Continue polling through PAID → PROCESSING → COMPLETED.
- [ ] Verify one execution in protected seller state. Repeated polls/recovery or
  seller restart must not duplicate execution. Back up consistently before any
  runtime restart/change; never modify the order database to force a transition.

Exit: exactly one deterministic execution and one durable result.

### 8. Buyer retrieves the result

- [ ] Retrieve **HELLO WORLD** directly from the seller with the private claim
  capability; verify the same result from a recovered buyer session.
- [ ] Keep result/recovery data private. Missing or reorganized payment evidence
  may withhold a cached result; do not interpret it as permission to pay again.

Exit: the independent buyer received the result, without discovery as a proxy.

### 9. Verify the discovery host received no order/payment/result data

- [ ] Inspect buyer network traffic: order creation/check/result requests target
  the seller origin directly. No `/api/` discovery order proxy is used.
- [ ] Inspect discovery request routes and persistent state without exposing
  credentials: no order ID, claim/idempotency key, private payment memo/input or
  result. Discovery may store public OFFER price/endpoint and its publication
  transaction provenance; these are not private buyer payment data.
- [ ] Confirm discovery/indexer downtime or reconstruction does not own or erase
  seller orders. The isolated regression performs these disruptive checks only
  on disposable fixtures, never on the live mainnet database.
- [ ] Save minimized acceptance evidence locally. Publish no wallet configuration,
  credentials, private seller authority, buyer receipt or order capability.

Exit: discovery remains a read-only offer index and commerce remains buyer–seller.

## Current stopping point and reproducibility

Permanent HTTPS origin, separate buyer wallet/controller and tiny price remain
unconfigured/unapproved. Preparation ends here: no new offer, payment, runtime
migration or wallet change. The existing mainnet setup is preserved.

The local acceptance is repeatable with fresh private temporary directories:

```sh
cargo build -p zadp-commerce --bins --examples
python3 -m unittest discover -s examples/commerce -p 'test_*.py'
node ui/test_commerce.js
```

Run the process test twice to verify fresh setup and cleanup, not only recovery
of one previous fixture. It exercises valid local HTTPS, separate buyer/seller/
UI/indexer processes, confirmation gating, one execution, seller restart and
indexer reconstruction with unchanged private commerce state. Evidence is
synthetic offline testnet data; it does not establish live funded-testnet or
mainnet readiness. See [process boundaries](commerce-process-boundaries.md) and
[the seller API/configuration contract](commerce.md) for details.

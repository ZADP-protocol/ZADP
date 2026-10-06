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

## Ordered gate for one seller and one offer

1. Verify public wallet/node synchronization and readiness through protected
   local processes. Verify the existing signed board descriptor and intended
   seller receiver. Any future runtime change needs a protected consistent
   SQLite backup; do not expose secrets or silently migrate an existing database.
2. Prepare a seller-only mainnet configuration using the existing seller account
   and receiver, a separate protected commerce database, and the read-only Zallet
   observer. Check HTTPS/CORS and observer consistency. Do not activate changes
   against the live setup until the concrete deployment has been reviewed.
3. Prepare one new active signed OFFER with the real endpoint and agreed small
   price. Check current signed board policy, endpoint, lifetime and memo budget.
   Generate the existing CLI unsigned publication fee preview. Obtain explicit
   approval of this new publication before signing/broadcasting it. Never replace the absent real endpoint with a
   placeholder or temporary test origin. It uses the
   existing board; no new board creation or funding is required.
4. Confirm the new OFFER is mined/accepted and discovered with correct endpoint,
   seller identity, price and chain provenance. Use an independent buyer session.
5. Create exactly one private order at that seller endpoint. Review the full
   ZIP-321 URI, existing seller receiver, exact amount, correlation memo, expiry
   and confirmation policy. Do not send an address-only payment.
6. First verify locally that the buyer controller uses a distinct wallet/account,
   not the seller or board wallet; do not expose that configuration to discovery.
   If no independent buyer wallet is configured, stop at the payment request/QR
   stage. Otherwise have that buyer wallet/payment controller generate its unsigned
   transaction/fee preview. Report requested service value, wallet-derived
   network fee and total cost for explicit human approval. No invented fee, no
   automatic reuse of a self-payment, and no approval inferred from sync status.
7. Only after that concrete approval may the one real buyer-to-seller transaction
   be signed/sent. Record its private receipt locally, never in discovery. Do not
   retry if transport status is uncertain; first check the original transaction.
8. Wait for configured confirmations, verify seller non-change payment evidence,
   observe PAID/PROCESSING/COMPLETED and retrieve HELLO WORLD from a fresh buyer
   session. Check seller execution count one and discovery state contains no
   order/capability/input/result/payment correlation data. Public evidence must
   be carefully minimized; disclose no wallet credentials or private authority.

A real fee preview is currently blocked on the HTTPS origin, independent buyer
wallet/controller and agreed price. This document is preparation, not a
transaction, publication, activation or spending approval. No new mainnet change
was made by the offline process-isolation acceptance.

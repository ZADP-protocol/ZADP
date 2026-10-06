# Local ZADP UI

Run `python3 ui/server.py` and open http://127.0.0.1:8090.
The existing read-only mainnet indexer must listen on 127.0.0.1:8080.
Alternative ports: `--port 8091 --indexer-port 8081`.

The interface is English-only.

No npm packages or build step are needed. The server binds to loopback only,
serves fixed assets, and proxies only GET /offers and GET /healthz to a
fixed loopback indexer. Optional protected local operator configuration enables
read-only wallet status and unsigned CLI previews; credentials stay in the server
and are never returned to the browser. No signing or broadcast route exists.
Offers are rendered as text, never HTML. Drafts stay in browser localStorage
and can be downloaded as unsigned JSON; they are not protocol publications.
Wallet readiness is unavailable unless explicitly configured. Its local node tip
is not advertised as a verified public network tip.
The mainnet placeholder endpoint is explicitly labelled and cannot be purchased.

Test the local proxy and service with `python3 -m unittest discover -s ui -p 'test_*.py'`.

## Free local test service

The Test service tab calls `POST /api/test-service/summarize`. This deterministic
extractive demo returns the first three sentences; it uses no AI provider,
stores no input and makes no payment or blockchain transaction. Requests are
limited to 16 KiB JSON and 4,000 input characters, with cross-origin browser
submissions rejected. The server never fetches offer endpoints.

It is not the service advertised by the existing mainnet placeholder OFFER.
A separate [payment-gated local service](../services/text/README.md) now implements
invoice correlation and authenticated seller-side payment observation. Its tests
use synthetic observations; a real paid mainnet test remains pending approval.

## Unsigned memo size preview

The draft form counts the CBOR-encoded UTF-8 payload against its 504-byte
budget. It includes the board ID, 26-character ULID, 32-byte public identity,
16-byte nonce, 64-byte signature and a 24-hour lifetime. Positive ZEC amounts
must have at most eight decimal places; endpoints must use HTTPS without
embedded credentials. Oversized or invalid drafts cannot be saved/exported.
Optional descriptions use the same shared byte budget.

This is a protocol-v1 size preview, not a signed-board policy check, fee quote
or publication approval. The Rust publication encoder remains authoritative.
Run `node ui/test_memo_budget.js` for boundary and Unicode tests.

Cross-language verification: `node ui/test_memo_rust_parity.js` runs the actual
Rust memo encoder offline with ephemeral Ed25519 identities. It compares
Unicode, CBOR length boundaries, price integer boundaries, board IDs, timestamps,
and exact 504-byte/overflow cases. No wallet files or chain access are used.

Protocol field limits are UTF-8 bytes: title 128, capability 64, board ID 64.
These are enforced in addition to the shared 504-byte payload limit.

## Protected operator integration

Install `python3 -m pip install -r ui/requirements.txt` in a virtual environment.
Run `python3 ui/server.py --operator-config /protected/ui-operator-config.json`.
Keep this owner-only regular file outside Git. Required fields: `network`,
`wallet_rpc` (loopback HTTP), `wallet_auth_file`, `account_uuid`, `receive_address`,
`seller_binary`, `seller_identity`, `board` and `lightwalletd`. Use the already
verified seller account and receiver; this integration never creates a wallet.

Connection shows filtered seller spendable/pending balances, synchronization
height and a locally generated address-only QR. It returns no RPC authentication,
account IDs, seeds, spending keys or private viewing authority.

My offer's **Preview publication costs** invokes the seller CLI with `--dry-run`
and `--preview-output`. The CLI verifies the signed board and actual memo, and
uses current Zcash library policy to prepare an unsigned proposal. It returns
listing value, network fee, total cost, board fingerprint and encoded payload
size. Ed25519 offer signing is distinct from Zcash transaction signing; the
latter and broadcasting never occur in this route. Fees must be rechecked for
any later publication; a preview is not an approval or a guaranteed future fee.

Optional `text_service` configuration contains a fixed loopback HTTPS `endpoint`,
`tls_ca`, `offer_id` and integer `price_zat`. The UI never fetches arbitrary offer
URLs. **Prepare order** creates a private persistent invoice for
the entered text, displaying network, exact amount, receiver, required text memo,
expiry and confirmation threshold. Buyer network fees are additional and are not
quoted by an invoice. The access token stays in browser memory; the private
recovery key restores access after reload. **Check payment** is read-only with respect
to funds and releases output only after the seller observer verifies payment.

This is an unpublished local test, separate from the placeholder mainnet offer.
There is no buyer payment-signing path and no live paid acceptance claim. Obtain
separate human approval before any mainnet transfer. Keep invoice inputs, tokens,
SQLite files, TLS private keys and wallet/operator configuration out of Git.


The UI derives network labels from validated chain metadata rather than assuming
mainnet. With no offers, the current API supplies no network metadata and the
label is unavailable. Invalid nested offer/price/chain data is rejected. Details
show network, txid, inclusion height, confirmations, seller and expiry; signature
status is explicitly the indexer's acceptance, not browser-side cryptographic
verification. Run `node ui/test_offers.js` for these response boundaries.


## Recoverable local purchase flow

The English Test service panel prepares one active order, restores it after a
reload, polls authenticated payment status every 15 seconds while visible, shows
confirmation progress and releases the result only when the service verifies
payment. Errors hide stale results. Expired orders cannot show payment instructions.
Once a payment has been seen, instructions remain hidden through outages/reloads;
the hint never authorizes fulfillment. New orders require completion or expiry.

A random 256-bit recovery capability and a payment-seen hint are stored in this
browser's localStorage. Neither input text nor wallet credentials are stored there.
Anyone with access to this browser origin can recover the order; private input
remains in the service's protected local SQLite database. Web Locks serialize
order preparation across tabs. Storage/lock failures prevent invoice creation.
Creation retries use the same key; the service persists its hash and returns the
same invoice/token, rejecting changed input. Invoice access tokens remain in memory.
Starting a new order forgets browser access to the previous one, not service data.
The UI has no signing or broadcasting route and cannot prevent someone manually
sending the same payment twice from an external wallet.

Run `node ui/test_purchase.js` for recovery, duplicate-click, interrupted creation,
expiry, malformed-response and unavailable/reorg behavior. For an isolated browser
simulation, create a state file containing `{"status":"awaiting_payment"}` and run:

```sh
python3 ui/dev_purchase_fixture.py --state-file /tmp/zadp-fixture-state.json
```

Open port 8093. Change only the fixture state file to
`{"status":"confirming","confirmations":2}`, `{"status":"fulfilled"}`,
`{"status":"expired"}` or `{"status":"unavailable"}` and check the order.
The separate simulator has no wallet credentials or payment methods. Its invoices
are explicitly marked LOCAL SIMULATION, use a non-payable fixture receiver, and
must never receive funds. No simulator route is included in the real UI server.

## Multiple boards

Discovery cards and details include their board ID. The Board selector filters
currently indexed offers without merging offers that share an ID across boards.
The indexer performs host visibility filtering before returning offers; browser
filtering is a browsing convenience, not a moderation or signature boundary.
Wallet status, publication previews and the paid local test remain bound to the
protected operator configuration rather than the selected discovery board.
See [Multi-board operator guide](../docs/multi-board.md).

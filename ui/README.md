# Local ZADP UI

Run `python3 ui/server.py` and open http://127.0.0.1:8090.
The existing read-only mainnet indexer must listen on 127.0.0.1:8080.
Alternative ports: `--port 8091 --indexer-port 8081`.

The interface is English-only.

No npm packages or build step are needed. The server binds to loopback only,
serves fixed assets, and proxies only GET /offers and GET /healthz to a
fixed loopback indexer. It has no wallet credentials or publication routes.
Offers are rendered as text, never HTML. Drafts stay in browser localStorage
and can be downloaded as unsigned JSON; they are not protocol publications.
The UI does not claim to know wallet balances or Zebra synchronization status.
The mainnet placeholder endpoint is explicitly labelled and cannot be purchased.

Test the local proxy and service with `python3 -m unittest discover -s ui -p 'test_*.py'`.

## Free local test service

The Test service tab calls `POST /api/test-service/summarize`. This deterministic
extractive demo returns the first three sentences; it uses no AI provider,
stores no input and makes no payment or blockchain transaction. Requests are
limited to 16 KiB JSON and 4,000 input characters, with cross-origin browser
submissions rejected. The server never fetches offer endpoints.

It is not the service advertised by the existing mainnet placeholder OFFER.
Actual payment verification and paid fulfillment are still unimplemented.

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

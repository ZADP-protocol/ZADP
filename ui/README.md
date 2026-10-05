# Local ZADP UI

Run `python3 ui/server.py` and open http://127.0.0.1:8090.
The existing read-only mainnet indexer must listen on 127.0.0.1:8080.
Alternative ports: `--port 8091 --indexer-port 8081`.

The interface is English-only.

No npm packages or build step are needed. The server binds to loopback only,
serves three fixed assets, and proxies only GET /offers and GET /healthz to a
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

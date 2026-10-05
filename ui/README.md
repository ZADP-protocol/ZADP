# Local ZADP UI

Run `python3 ui/server.py` and open http://127.0.0.1:8090.
The existing read-only mainnet indexer must listen on 127.0.0.1:8080.
Alternative ports: `--port 8091 --indexer-port 8081`.

No npm packages or build step are needed. The server binds to loopback only,
serves three fixed assets, and proxies only GET /offers and GET /healthz to a
fixed loopback indexer. It has no wallet credentials or publication routes.
Offers are rendered as text, never HTML. Drafts stay in browser localStorage
and can be downloaded as unsigned JSON; they are not protocol publications.
The UI does not claim to know wallet balances or Zebra synchronization status.
The mainnet placeholder endpoint is explicitly labelled and cannot be purchased.

Test the local proxy with `python3 -m unittest discover -s ui -p 'test_*.py'`.

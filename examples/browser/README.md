# Minimal discovery reference browser

Start a separately configured read-only indexer on loopback port 8080, then:

```sh
python3 examples/browser/server.py --port 8090 --indexer-port 8080
```

Open http://127.0.0.1:8090. The page lists active offers, allows Anywhere and locally chosen Nearby matching, and keeps buyer coordinates only in runtime memory. It has no map provider, storage, checkout, wallet, publication or hosted-product dependency. The proxy accepts only fixed public discovery paths, rejects coordinate query strings, and does not log requests.

The page trusts its chosen indexer's signature validation; use the protocol library when independently verifying a retrieved Offer. This is an interoperability example, not the official hosted visual application.

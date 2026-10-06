# Independent commerce components — checkpoint c50c1e3

The automated acceptance uses independently running processes, not in-process
API calls or a shared Python application. All blockchain/payment observations are
synthetic offline testnet fixtures. No testnet or mainnet transaction is sent.
The real Rust indexer validates a signed, board-bound OFFER memo and writes its
normal persistent SQLite schema. Blocks are supplied offline; this is not a live
network scanning or funded-testnet acceptance claim. Existing encrypted scanner
regressions separately test note decryption into independent indices.

## Processes and storage

1. `zadp-commerce`: seller-owned private order database and protected mock
   observation file; loopback HTTP backend, separate OS process.
2. `https_fixture.py`: separate local TLS reverse proxy at the advertised HTTPS
   seller endpoint. Only orders/health are forwarded. No discovery or spending.
3. `fixture_indexer`: separate Rust process using `SqliteOfferRepository` and
   the read-only `board_router`. Its directory contains only a synthetic signed
   public descriptor, public offer, board SQLite database and normal SQLite
   sidecars. It never opens seller configuration or buyer/seller order storage.
4. `ui/server.py`: separate process serving the existing UI, with no operator
   configuration. Its fixed discovery upstream points to the isolated indexer.
5. `buyer.py`: separate client processes/session, with a private recovery file
   and input file in a buyer-only directory. Uses the same seller API as the
   browser; does not send the claim token, input, payment correlation or result
   to discovery. Preparing/checking an order never sends a payment.

The test selects free loopback ports to coexist with the existing mainnet
services and the interactive 8095 demo. Network boundaries are:

```text
Buyer -> UI/discovery /api/offers -> Rust indexer /offers -> board database
Buyer -> advertised HTTPS seller /orders -> seller backend -> orders database
Seller -> protected mock snapshot (real deployment: authenticated read-only wallet)
```

The public OFFER contains the service price and endpoint. Private per-order price,
correlation/payment data, input, result and capability stay between buyer and
seller. No seller order route is available through the discovery API or UI.
The HTTPS fixture uses an ephemeral self-signed test certificate with loopback
IP SAN. The buyer explicitly trusts that certificate as its local test CA;
validation remains enabled and an untrusted connection is tested to fail.
The proxy also forwards browser CORS preflight to the seller, which allows the
configured UI origin. This certificate/proxy is not a public production endpoint.

## Reproduce and acceptance assertions

```sh
cargo build -p zadp-commerce --bins --examples
python3 -m unittest discover -s examples/commerce -p 'test_*.py'
```

The test creates private temporary seller, buyer and discovery directories and
starts/stops its own processes only. It discovers the actual indexed offer
through the separately served UI discovery route, creates one order directly
with the seller HTTPS endpoint, rejects an invalid claim and waits at one
confirmation. At three synthetic confirmations it observes PAID, PROCESSING,
COMPLETED and retrieves uppercase output. Discovery database contents and public
files remain identical; credential, order ID, memo, input and output are checked
absent from its database dump.

It then stops both UI and indexer. A fresh buyer process can still retrieve the
result. A consistent owner-only order database backup precedes a seller restart;
retrieval still works and execution count stays one. Finally it deletes/rebuilds
only the disposable fixture index, verifies the reconstructed offer, and checks
that the seller order/result is unaffected. This is a regression test for
commerce state remaining independent of board cursors, rebuilds and availability.

This test is in CI. Existing anonymous order, replay/reuse, expiry, payment-field,
reorg, browser recovery and QR tests remain required. Passing this bounded suite
is a local architecture checkpoint, not a claim of complete production stability.
See [the gated mainnet plan](mainnet-commerce-plan.md) before any real transition.

## Transition is deferred until a real endpoint and independent wallet exist

The seller endpoint remains configurable; local service operation continues.
Do not publish a new mainnet offer with a placeholder or temporary test endpoint.
The independent buyer CLI in this test is an order client, not a funded wallet.
A separate real buyer wallet/account must be configured outside the existing
seller/board wallet before real payment preview and explicit human approval.
No seed/spending keys should be created or requested in chat. See the updated
[mainnet gate](mainnet-commerce-plan.md); stop at non-broadcast preview/QR
preparation while those deployment/wallet prerequisites are absent.

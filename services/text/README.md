# Payment-gated deterministic text service

This seller endpoint shortens text to its first three sentences. It runs locally,
uses no external AI provider, and contains no transaction-signing or broadcasting
code. It is separate from the mainnet discovery API and the free UI demo.

Implemented flow:

1. `POST /v1/invoices` with `offer_id` and `text` returns an invoice, exact zat
   amount, seller receiver, expiry, `ZADP-PAY/1:<invoice_id>` text memo and a
   random access token. Keep the token private.
2. The buyer pays from a separate wallet (not implemented in this endpoint).
3. `POST /v1/fulfill` with only `invoice_id` and `access_token` queries the local
   authenticated Zallet observer. Clients cannot submit payment evidence.
4. An exact shielded non-change output to the configured seller account/address,
   matching value and memo, mined during the invoice window and with at least
   three confirmations, releases the deterministic result.

Invoices/results persist in owner-only SQLite. Active invoice capacity is 100;
a 10,000-row total cap requires operator retention review rather than silently
deleting private data. Results are idempotent and each
retrieval rechecks chain evidence, so lost confirmations withhold cached results.
Unsynchronized/unavailable wallets fail closed. One payment output cannot be
consumed by multiple invoices. Underpayment, overpayment, wrong receiver/memo,
transparent outputs and late mining are rejected. Late/incorrect payments are
not automatically refunded; operators need a separate human refund policy.

## Verification and limits

Protected file checks require Linux/Unix ownership semantics.

Run `python3 -m unittest discover -s services/text -p 'test_*.py'`.
Tests use trusted synthetic wallet observations and temporary private databases.
They prove the HTTP/domain gate, not a real testnet or mainnet payment.
The observer uses beta.3 `getwalletstatus` and
`z_listtransactions(account_uuid, start_height, end_height, offset, limit)`.
It reads at most 1,000 transactions and refuses truncated results.
It never returns wallet RPC bodies or credentials to clients.

A real buyer payment, discovery integration with a new signed offer and a publicly
reachable HTTPS seller endpoint remain pending. The local UI can prepare invoices
and check fulfillment through a fixed TLS-verified loopback bridge. The existing mainnet OFFER still points to a
placeholder; do not treat this service as that published offer or replay it.

## Local operator configuration

Run `python3 services/text/server.py --config /protected/seller-config.json
--database /protected/invoices.sqlite --tls-cert /protected/localhost.crt
--tls-key /protected/localhost.key --port 8092` (one command line).
Configuration must be an owner-only regular JSON file outside this repository:

- `network`: explicit `testnet` or `mainnet`
- `offer_id`: the intended service's signed offer ID
- `account_uuid`, `receive_address`: verified dedicated seller receiver/account
- `price_zat`: exact positive integer service price
- `wallet_rpc`: authenticated loopback Zallet RPC URL, without URL credentials
- `wallet_auth_file`: protected local authentication file path

This command does not create a wallet, account, address or offer. Do not run it
against the funded runtime until its intended offer and receiving address have
been verified. The human selected a mainnet fee preview followed by separate approval.
No new mainnet payment or publication has been authorized by this implementation.

The loopback server is a development endpoint. TLS is mandatory; local tests use an explicitly trusted certificate with matching
localhost/127.0.0.1 SANs. Production needs publicly trusted HTTPS termination,
rate limiting, request authentication/anti-abuse, retention/privacy policy and
robust payment/refund semantics. Input is stored privately with the invoice;
unlike the free demo it is not transient. No invoice data belongs in Git.

## Explicit local self-payment test CLI

`zadp-seller pay-test` can pay a fresh protected invoice from the existing
seller wallet back to its verified external receiving address. This exercises
actual shielded payment observation and fulfillment, but is not an independent
buyer integration or a public service-discovery test.

```sh
cargo run -p zadp-seller -- pay-test \
  --operator-config /protected/operator-config.json \
  --invoice /protected/invoice.json \
  --journal /protected/payment-journal.json \
  --max-fee-zat 10000
```

The default only builds and inspects a wallet-created FullPrivacy proposal.
After separate human authorization, `--execute` proves, signs, verifies the
extracted shielded transaction and submits it once. The fee cap is a limit,
not a fee estimate; actual policy comes from the local wallet. A protected
recovery journal is created before signing and blocks repeat execution, even
if submission fails or times out. Reconcile its transaction ID with the wallet
and chain rather than deleting the journal or automatically retrying.

The CLI verifies invoice amount/offer/memo/expiry, network-correct unified
receiver, synchronized chain identity, exact final decrypted output and fee.
Incoming viewing authority is used only inside the local process and never
printed or saved. Auth files, invoice tokens and journals remain outside Git.
Zallet may classify same-account payments as change even when explicitly
addressed. Normal fulfillment continues to reject change. For this local test
only, start the service with `--self-payment-journal /protected/payment-journal.json`.
The owner-only journal must identify one already-broadcast transaction, invoice
and exact amount on the configured network. Only that pinned transaction may
satisfy the self-payment invoice, and the result is marked `self_payment_test`.
Wrong memos, amounts, receivers, accounts, timing or confirmations still fail. The invoice amount stays in the same wallet and
only the network fee reduces its net balance. Invoice settlement still needs
three confirmations and the service's exact payment checks.

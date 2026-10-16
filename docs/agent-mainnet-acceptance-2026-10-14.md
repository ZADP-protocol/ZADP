# Agent-controller mainnet purchase — 2026-10-14

Public summary of a specifically human-authorized operator test, reviewed for
this documentation update on 2026-10-17.
[Sanitized evidence](agent-mainnet-acceptance-2026-10-14.json).

## Completed path

The agent discovered legacy wire-1 `text.summarize` Offer
`01M4J8687QJBZHQHEZHDEFVT5Y`, independently checked its signature and seller-origin
proof, checked live capabilities and created one idempotent direct seller order.
The actual PaymentController returned **ALLOW** under an exact single-service
policy (1000-zat single/daily service limit and approval threshold). It used the
real **ReviewedWalletAdapter**, not a mock or `with_auto_approve` helper.

The adapter bound the invoice, separate buyer/seller accounts, recipient, memo,
reviewed unsigned proposal/hash and one-attempt journal. Payment:

- Service amount: **1000 zat (0.00001000 ZEC)**.
- Actual wallet-derived fee: **10000 zat (0.00010000 ZEC)**.
- Purchase transaction total: **11000 zat (0.00011000 ZEC)**.
- Transaction: `4b8b78fb090d286f383f4a60415cd673a3fabda9ab3108b80b188d1d67fc9f88`.

Seller state reached `waiting_payment → paid → processing → completed`.
Eight confirmations were observed against three required; payment was verified
and the deterministic result matched the requested operation. Seller DB inspection
showed `execution_count=1` before and after repeated completion polling.

## Scope and limits

This establishes a real agent-controller wallet payment and confirmation-gated
fulfillment for that legacy service under the authorized test policy. The buyer
and seller accounts were distinct but belonged to the **same wallet/operator**.
It does not establish independent wallet owners, canonical wire-3 commerce,
signed invoices, a hardened OS-isolated agent or fresh independent chain
reconstruction. General unattended spending is not authorized by this record.

The purchase total above excludes separate test-account funding costs and a
separate discovery publication. No buyer input, returned content, claim token,
payment memo, wallet credential or private viewing/spending key is published.
Historical acceptance is not a promise of current seller availability.

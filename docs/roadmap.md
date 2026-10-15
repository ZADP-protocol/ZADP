# ZADP roadmap

Current engineering priorities, not normative protocol semantics or payment
approval. See [Protocol 1.0](protocol.md) and [mainnet evidence](mainnet-evidence.md).

## DONE

- Protocol 1.0 canonical wire/model 3; frozen historical wire 1/2 reading.
- Signed boards, per-board SQLite isolation and multi-board read-only discovery.
- Mainnet geo publication and independent reconstruction.
- Hosted globe, self-hosted cartography and browser-local Nearby matching.
- Offline-signed endpoint ownership proof with restricted SSRF-safe verification.
- Hosted multi-Offer Seller with isolated order databases and explicit order gates.
- Real human buyer→seller mainnet purchase and exactly-once deterministic fulfillment.
- Offline agent order→InvoiceView→PaymentController→mock settlement acceptance.

## NOW

- Public-release readiness, including current dependency advisories and full-history review; owner approval remains separate.
- Production policy boundaries: protocol validity, admission, host visibility,
  endpoint verification, supported capability and live orders availability.
- Seller retention/redaction implementation with replay-safe tombstones and recovery.
- Keep checked-in PaymentController acceptance evidence distinct from later
  operator runs. A real-wallet claim needs a separate buyer account, exact fee
  proposal, approval and sanitized fulfillment evidence. Unpaid preparation is not acceptance.
- Capability-model expansion beyond deterministic text examples.

## NEXT

- `offer_update` remains unimplemented. Additive seller-signed `offer_revoke` is implemented and offline-tested; deployments and mainnet acceptance require separate operator evidence ([design](offer-revoke.md)).
- Broader seller capabilities and request/response interoperability.
- Board federation / descriptor-sharing design; no automatic sharing protocol today.
- Further hosted abuse controls belong in ZADP-Hosted; Core stays independently runnable.
- More independent clients, indexers, wallet adapters and interoperability evidence.

## DEFERRED

Escrow, reputation, DAO/token systems, hosted custody and cross-chain abstractions.
No roadmap item authorizes funds, publication, deployment, wallet provisioning or
an automatic sweep. Hosted pricing, charging and rollout are operator policy,
not state configured or enabled by this roadmap.

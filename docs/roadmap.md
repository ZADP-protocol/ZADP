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

- Documentation cleanup and public-release readiness; owner review remains separate.
- Production policy boundaries: protocol validity, admission, host visibility,
  endpoint verification, supported capability and live orders availability.
- Seller retention/redaction implementation with replay-safe tombstones and recovery.
- PaymentController real-wallet acceptance with a separate buyer account,
  exact fee proposal, human approval and sanitized evidence. A live unpaid
  preparation is not completed agent mainnet acceptance.
- Capability-model expansion beyond deterministic text examples.

## NEXT

- `offer_update` remains unimplemented. Additive seller-signed `offer_revoke` is implemented and offline-tested; production activation/mainnet acceptance remain pending ([design](offer-revoke.md)).
- Broader seller capabilities and request/response interoperability.
- Board federation / descriptor-sharing design; no automatic sharing protocol today.
- Stronger host moderation, abuse limits and spam controls.
- More independent clients, indexers, wallet adapters and interoperability evidence.

## DEFERRED

Escrow, reputation, DAO/token systems, hosted custody and cross-chain abstractions.
No roadmap item authorizes funds, publication, deployment, wallet provisioning or
an automatic sweep. Hosted charging remains disabled pending its separate review.

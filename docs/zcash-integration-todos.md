# Zcash Integration TODOs (future real adapter)

Phase 1 ships `MockZcashAdapter` only. **Do not invent real Zcash RPC method shapes** in application code until validated against current zcashd / zebrad / lightwalletd / librustzcash APIs.

## Required capabilities (real adapter)

- [ ] Shielded transaction publishing carrying ZADP offer payloads
- [ ] Memo encoding/decoding (`zadp1-cbor-v1` + framing)
- [ ] Read-only scanning for indexer (viewing authority only)
- [ ] Shielded payments from buyer wallet
- [ ] Payment observation for sellers
- [ ] Strict separation of viewing vs spending authority

## Unresolved questions

1. **Memo capacity:** Confirm usable memo bytes for the chosen pool (Sapling vs Orchard) and any library framing overhead. Is 512 bytes the hard ceiling for our payload?
2. **Board addressing:** How is the “ZADP board” represented on-chain (fixed UA, multipool receiver, ZIP convention)? Who holds the board viewing key?
3. **Scan model:** Full node, light client, or hosted scanner? Reorg handling for offer finality?
4. **Payment correlation:** Canonical way to bind `invoice_id` inside encrypted memo so seller can observe without leaking to third parties.
5. **Fee / change / UA:** How invoices express `pay_to` for shielded receivers across wallet implementations.
6. **Library choice:** Prefer `librustzcash` / `zebra` ecosystem crates vs wallet RPC — evaluate maintenance and key-management fit.
7. **Testnet ops:** Faucet, confirmations required before fulfillment, chain tip lag SLOs.
8. **Spam resistance:** Economic cost to publish offers when memos are cheap to broadcast.

## Non-goals for the real adapter wrapper

- Do not redesign domain types when swapping mock → real.
- Do not give indexer a spending key.
- Do not pass any Zcash secrets into LLM/agent processes.

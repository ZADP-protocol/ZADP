# Two-minute demonstration (no new mainnet spend)

Use the already recorded [public proof](mainnet-acceptance-proof.json). Do not
replay its transaction. The existing OFFER expires: a later live API may omit
it, so distinguish an archived verified proof from today's active catalog.

- **0:00–0:15:** ZADP discovers signed service offers through shielded Zcash memos.
  Independent indexers read a dedicated board UIVK without spending authority.
- **0:15–0:35:** Show seller CLI help and the signed OFFER/public receipt. Explain
  the separate Ed25519 identity signature and human-approved wallet publication.
  Use the archived receipt or an explicitly labelled unsigned preview, not a spend.
- **0:35–0:55:** Show recorded txid, inclusion height 3507498 and publication cost
  26,000 zat. This is real mainnet publication, not buyer settlement.
- **0:55–1:15:** Show the two independent empty-database reconstruction records:
  same offer, transaction, signature/policy acceptance and listing value.
- **1:15–1:35:** Show `GET /offers` and UI provenance: network, block height,
  confirmations, seller, expiry and index acceptance. If expired, show archived
  proof with an explicit date rather than claiming a current active listing.
- **1:35–1:50:** Demonstrate the free local deterministic text service, or an
  unpaid invoice withholding a result. No real payment occurs in this demo.
- **1:50–2:00:** Next milestone: reviewed buyer transaction preview, policy/human
  approval, shielded payment and seller-observed deterministic delivery.

The mainnet endpoint example.com is a placeholder. Do not click it as a working
service. No hosted production marketplace, escrow, AI/MCP buying or private
board-content claim is made. Public board UIVK holders can read OFFER content.

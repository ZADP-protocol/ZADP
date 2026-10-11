# Additive Offer revocation

ZADP Protocol 1.0 keeps the existing canonical Offer model unchanged. Revocation
is represented by a separate signed append-only control message. Existing Offer
wire/model 3 data is not reinterpreted or invalidated. Historical wire 1/2 remain
read-compatible, with identical signing domains, tuples and memo bytes.

## Transport and signing

`decode_zadp_memo` returns `ZadpMessage::Offer` or `OfferRevoke`. The original
`decode_offer_memo` is retained for Offer-only consumers; it rejects control
messages as unsupported rather than interpreting them as Offers.

The existing 512-byte framing stays `f5 ZADP | discriminator | u16 BE length |
CBOR | zero padding`; payload maximum remains 504 bytes. Discriminators 1/2/3
retain their existing Offer meaning. **0x81** is the new revoke-v1 message
identifier, not Offer wire 4. Unknown message discriminators fail closed.

Revoke JSON fields: `zadp: 1`, `type: "offer_revoke"`, `board_id`, `offer_id`,
`seller`, `issued_at`, `nonce`, `signature`. IDs are nonempty, at most 64 UTF-8
bytes, without control characters. Issue time is nonnegative whole UTC seconds.
Nonce is canonical Base64 of 16–32 bytes. No reason or replacement fields exist.

Signature: Ed25519 over `ZADP1\0offer_revoke\0` followed by deterministic CBOR:

```
[1, "offer_revoke", board_id, offer_id, seller_key_bstr32,
 issued_at_epoch_seconds, nonce_bstr]
```

Wire payload (seven fields):

```
[1, board_id, offer_id, seller_key_bstr32, issued_at_epoch_seconds,
 nonce_bstr, signature_bstr64]
```

Type is unambiguously implied by 0x81 on transport and explicitly signed. Decoder
checks lengths, padding, signature and canonical re-encoding. With maximum
64-byte references/32-byte nonce, it remains well below 504 bytes.

## Board acceptance and replay

BoardDescriptor v1 is unchanged. Revoke uses the descriptor's existing minimum
listing value, authorized-seller admission, clock skew and publication rate
window. There is no hardcoded new control price. A future cheaper-control policy
requires a separately reviewed policy extension; it is not implemented here.

Target `(board_id, seller, offer_id)` must already be accepted earlier in chain
order, including transaction/output order within a block. The same seller must
sign. `issued_at` must not precede the target's signed issue time. Seller nonces
are shared across accepted Offers/revokes. Each accepted revoke consumes one
publication-rate slot but no active-Offer slot. The **first accepted revoke
wins**; later revokes of that target or nonce/output replays are rejected and
audited. Unknown/wrong seller/board targets are rejected, not queued for later.
Already-expired accepted Offers can be revoked and remain in history.

## Storage, history and reorg

`offer_revocations` stores signed JSON, target seller/id, nonce, txid, pool,
output index, transaction index, height, block time and value. Primary key is
seller/id; nonce and output identifiers are unique. Foreign keys reference the
original Offer and indexed block with `ON DELETE CASCADE`. The additive schema
upgrade preserves original Offer JSON and descriptor fingerprint enforcement.

Active canonical discovery is `expires > now AND NOT EXISTS accepted revoke`.
Host hide/quarantine is applied separately. Lifecycle precedence is `revoked`,
then `expired`, then `active`; internal history returns original Offer JSON plus
`revoked_at` (accepted block time) and `revocation_txid` metadata. No new public
history endpoint is introduced. Publication audit distinguishes Offer/control.
Rewind removing a revoke removes its row/audit/rate slot and restores the Offer
if unexpired and host-visible. Replay closes it deterministically again.

## Seller CLI and local UI

```
zadp-seller offer-revoke --network mainnet --board BOARD.json \
  --index CANONICAL.sqlite --offer-id OFFER_ID --identity PROTECTED_IDENTITY \
  --wallet-auth-file PROTECTED_AUTH --wallet-account ACCOUNT_UUID \
  --lightwalletd HTTPS_ENDPOINT --dry-run \
  --preview-output preview.json --proposal-output proposal.private.json
```

Target resolution opens the index **read-only**, checks the descriptor
fingerprint, original signature/board/seller and absence of accepted revoke.
Preview contains public signed revoke, board recipient/control value, exact
wallet-inspected fee/total, network, seller/target, 15-minute deadline and PCZT
SHA256. Only the public preview may be shared; PCZT and wallet files stay local.

An explicit later command must supply `--yes --prepared-revoke revoke.json
--approved-preview preview.json --approved-proposal proposal.private.json` with
the same network/board/index/target/identity/wallet settings. Bare `--yes` cannot
broadcast. Costs, target, descriptor, proposal hash and deadline are rechecked;
final transaction memo/output and actual fee must match before broadcast.
Target is checked again before signing and broadcast. There is no timer or
automatic broadcasting. A receipt records recovery txid before network submission;
uncertain results require manual chain review, never blind retry.

Web My Offer provides **Close offer**, not Delete. The protected loopback
companion accepts only operator-configured board/index pairs via `board_indexes`.
Preview is separate from execution; `revoke_execute_enabled` defaults false.
Execution also needs the existing private `publication_journal_dir`, exact saved
preview and human confirmation. Approval is consumed and journaled before the
wallet call. The hosted static UI has no wallet route or execution authority.

## API and commerce

`GET /offers` excludes revoked records; existing Offer JSON/verification semantics
are unchanged. Absent Offers cannot become `purchase_available`. Web rechecks
active hosted discovery before a new order and refuses revoked/expired states.

Seller config adds optional `canonical_index`:

```json
{"canonical_index":{"database":"/private/read-only/board.sqlite",
 "board_fingerprint":"EXACT_SIGNED_DESCRIPTOR_FINGERPRINT"}}
```

This source is read-only, must contain the exact signed configured Offer, and
must have the new schema. It is checked on every new order and capability query.
**Mainnet new orders fail closed when no canonical index is configured**, when
unreadable/mismatched, or when target is not accepted. Testnet standalone PoCs
may omit it. Maintain the index at chain tip; this is indexed chain state, not a
promise of instantaneous mempool revocation or service availability.

Revoked/expired new orders return HTTP **410** with `error: "offer_closed"`.
Unknown/unreadable lifecycle returns 503 `offer_lifecycle_unavailable`.
Capabilities report `orders_available: false`. Already-created orders and exact
idempotent retries continue their own payment/fulfillment state machine; revoke
does not cancel/refund existing invoices. Lifecycle source is excluded from the
invoice database's legacy config fingerprint, so enabling this safety check does
not invalidate old orders or alter their payment authority.

## Deployment status

Implementation/test work only. No production deployment, mainnet revocation or
transaction is authorized by this document. Before upgrading production, provide
a fingerprint-bound read-only canonical index to seller entries and validate
access/reorg behavior in staging. Keep spending/signing keys off the server.
`offer_update`, replacement, supersede and edit-in-place remain unimplemented.

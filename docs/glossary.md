# ZADP glossary

- **Offer:** seller-signed service announcement. Protocol 1.0 new publications
  use model/wire 3; immutable legacy records keep their original bytes.
- **Board:** a discovery stream defined by one signed descriptor and incoming scope.
- **BoardDescriptor:** signed version-1 network/address/viewing/birthday/policy binding;
  its fingerprint pins canonical index state.
- **Seller identity:** Ed25519 signing identity, not Zcash payer identity or KYC.
- **Seller endpoint:** optional seller-operated HTTPS service origin; signed presence
  alone does not establish control or order support.
- **Fixed price:** optional signed positive exact ZEC amount; requires endpoint.
  Distinct from listing value, hosted publication price and Zcash transaction fee.
- **Endpoint verification:** current proof of seller-identity ↔ HTTPS-origin control;
  not a guarantee of quality, legitimacy, availability or payment fulfillment.
- **Host visibility:** instance presentation decision independent of protocol validity
  and signed board admission.
- **orders_available:** seller-local Offer-specific new-order availability; must be
  checked alongside supported transport before preparation and enforced on POST.
- **purchase_available:** discovery host eligibility metadata; not live seller
  availability, wallet approval or signed authority.
- **Service area:** optional public signed geo center/radius, not verified residence.
- **Buyer location:** private transient client point used for local Nearby matching.
- **Discovery indexer:** incoming-only chain scanner that validates Offers and
  maintains isolated, reorg-aware canonical board state.

Old three-mode terminology and metadata remain only in historical design/read
compatibility discussion. See [normative protocol](protocol.md), [API](api.md)
and [privacy](privacy.md).

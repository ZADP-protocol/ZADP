# Public release review checklist

This is a review gate, not authorization to change repository visibility.

- [ ] Owner reviewed source boundaries and no hosted Web product remains in core HEAD.
- [ ] No production/server configuration or real host-policy data is included.
- [ ] Secret audit of HEAD and relevant full Git history reviewed.
- [ ] Any previously leaked credentials independently rotated; history treatment decided.
- [ ] Historical harmless UI/product files reviewed before public visibility.
- [ ] Personal author/email metadata and local artifacts intentionally reviewed.
- [ ] Neutral public examples and fixture-only identities checked.
- [ ] Licensing and third-party provenance reviewed; existing MIT OR Apache-2.0 choice confirmed.
- [ ] Protocol-focused README, SECURITY.md and CONTRIBUTING.md reviewed.
- [ ] Owner made a Code of Conduct decision.
- [ ] Protocol, wire, descriptor, optional commerce, expiry and privacy docs sufficient for implementers.
- [ ] Offer v1, Offer v2, indexer and reference client regression results reviewed.
- [ ] Independent self-hosting/reference client instructions tested.
- [ ] No production wallet seed, spending key or private authority included.
- [ ] Owner explicitly approves changing core repository visibility.

Web and operations remain separate private repositories. Do not mark the final approval automatically.

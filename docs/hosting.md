# Independent discovery and optional hosted policy

Core's `zadp-api` is a standalone read-only canonical discovery service. It
scans configured signed boards, enforces protocol/board admission and active
Offer lifecycle, and exposes public board metadata without private descriptors
or viewing keys. It has no dependency on ZADP-Hosted and no moderation/report
or administration endpoints.

Optional presentation policy, seller allowlists, content moderation, reports,
operator decisions and hosted publication-payment enforcement belong to
[ZADP-Hosted](https://github.com/ZADP-protocol/ZADP-Hosted). The former
`HOST_POLICY_MODE`, `HOST_SELLER_ALLOWLIST` and `HOST_POLICY_FILE` presentation
configuration is not a Core API contract. Consult Hosted for its current
operator configuration; do not apply obsolete pre-extraction instructions here.

A service promising Hosted moderation must use moderated Hosted routes for
list, search and exact lookup. Exposing an unfiltered standalone Core route as
a fallback would bypass that service's visibility policy. Canonical storage,
Offer signatures and protocol admission remain independent of hosted decisions.

Core protects discovery with bounded per-client and aggregate request limits;
see [API limits and trusted proxy handling](api.md#discovery-client-and-aggregate-budgets-not-protocol-rules).
Bind internal services to loopback, configure TLS at a trusted reverse proxy,
and accept forwarded client IPs only from that proxy. Authentication is an
operator choice, not a protocol requirement. Keep actual descriptors, incoming
viewing authority, runtime identity lists and deployment secrets outside Git.

Buyer geographic matching stays client-local; ordinary IP/network metadata is
still visible to servers. A valid Offer or verified endpoint does not establish
seller quality, availability or order acceptance. See [privacy](privacy.md),
[optional Hosted separation](hosted-moderation.md) and
[repository responsibilities](repository-boundaries.md).

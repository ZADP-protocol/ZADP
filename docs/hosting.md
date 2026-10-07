# Hosted discovery policy

The same application build supports local and hosted discovery. Without HOST_POLICY_MODE, the read-only scanner/API binary is unrestricted. Production operators explicitly set HOST_POLICY_MODE=allowlist and HOST_SELLER_ALLOWLIST to a JSON array of Ed25519 public seller IDs. Missing or empty allowlists return zero offers. Unknown modes, malformed JSON and invalid identities cause startup errors; configuration errors never fall back to unrestricted. No configuration contents are logged.

Filtering applies to /offers and /boards/{board_id}/offers, before response limits, including capability/search/board filters. Board metadata remains available. It does not modify canonical databases, signed board policy, offer signatures, ordering, geographical semantics or blockchain acceptance. Generic board_router_with_policy supports the same policy; the mock commerce demo is outside the production discovery path.

Buyer coordinates remain runtime-only in the browser; no location query, persistence or telemetry is added. Hosting does not make a seller trusted or a service verified. No signup or automatic approval is provided.

Host settings are presentation policy and separate from BoardDescriptor authorized_sellers. Production descriptors and viewing capability belong outside source control. Use root-readable environment files/systemd credentials, bind API to loopback, and terminate HTTPS plus authentication at a reverse proxy. Operational details for a specific domain belong in its separate private deployment repository.

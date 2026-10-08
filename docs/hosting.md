# Hosted discovery policy

The same application build supports local and hosted discovery. Without HOST_POLICY_MODE, the read-only scanner/API binary is unrestricted. Production operators explicitly set HOST_POLICY_MODE=allowlist and HOST_SELLER_ALLOWLIST to a JSON array of Ed25519 public seller IDs. Missing or empty allowlists return zero offers. Unknown modes, malformed JSON and invalid identities cause startup errors; configuration errors never fall back to unrestricted. No configuration contents are logged.

Filtering applies to /offers and /boards/{board_id}/offers, before response limits, including capability/search/board filters. Board metadata remains available. It does not modify canonical databases, signed board policy, offer signatures, ordering, geographical semantics or blockchain acceptance. Generic board_router_with_policy supports the same policy; the mock commerce demo is outside the production discovery path.

Buyer coordinates remain runtime-only in the browser; no location query, persistence or telemetry is added. Hosting does not make a seller trusted or a service verified. No signup or automatic approval is provided.

Host settings are presentation policy and separate from BoardDescriptor authorized_sellers. Production descriptors and viewing capability belong outside source control. Use root-readable environment files/systemd credentials, bind API to loopback, and terminate HTTPS plus authentication at a reverse proxy. Operational details for a specific domain belong in its separate private deployment repository.

## Generic manual presentation policy

`HOST_POLICY_MODE=manual` requires `HOST_POLICY_FILE`, a protected JSON version-1
file. Missing/malformed runtime snapshots fail closed. Its generic schema has
`sellers` (public identity + approved/suspended/blocked), `offers` (board ID,
seller, offer ID + approved/pending/blocked), `demo_auto_approved_sellers`,
`categories`, `capability_categories` and `minimum_publication_payment_zat`.
Defaults are not an operator's actual decisions; keep runtime identity lists
outside source. The index remains canonical even if one host hides an offer.

Each public request reads one policy snapshot. List/search, per-board lists and
`GET /offers/{id}` apply the same visibility rules. Hidden or missing IDs return
404; a visible ID repeated across boards requires a `board_id` selection.
Exact lookup occurs before the public list's 1,000-row response cap. No admin or
policy mutation endpoint is exposed by this generic read-only router.

The API health JSON may report `node_version` and `lightwalletd_version`, obtained
from the configured transport. These are public software version strings, not
wallet information, current network-tip evidence or compact-cache progress.
Public board summaries expose signed maximum TTL without the address/UIVK.
Private deployments own their manual CLI, economic rules and actual policy data.

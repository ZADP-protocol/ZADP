# Repository responsibilities

ZADP contains the open protocol, canonical indexer, generic API, Rust tools, reusable browser modules, reference clients and interoperability tests.

ZADP-Web contains the separate official hosted web product: branded UI, map presentation, seller and commerce UX, hosted duration/pricing presentation and product browser tests. It consumes an exact ZADP core commit; it does not maintain a second protocol implementation.

ZADP-Hosted owns optional moderation, reporting/admin storage and publication-payment visibility enforcement. It composes Core without changing signed protocol semantics.

ZADP-MCP is a read-only adapter over a configured moderated Hosted API; it has no wallet, order or publication authority.

ZADP-Server contains private instance deployment and operations. It records independent exact core and Web SHAs, runtime configuration and operational policy. Runtime secrets are outside Git.

ZADP-Seller contains the approved multi-Offer hosted seller, isolated private order DBs, incoming-only observation and exactly-once fulfillment. Core retains a small independently runnable reference seller.

Dependency direction: Web and Seller consume pinned core; operations deploys them. Core depends on neither private repository. Generic host visibility is distinct from canonical protocol validity, and a particular host's commercial pricing is not a protocol rule.

All repository visibility changes require a separate owner decision. Historical visual UI commits remain private during review; a HEAD-only move does not erase Git history.

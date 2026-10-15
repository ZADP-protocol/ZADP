# Security policy

This is an experimental reference implementation, not a production custody
product. No supported security maintenance window or independent audit is claimed.

Do not disclose a vulnerability with exploit details in a public issue. Use the
repository's private GitHub vulnerability reporting channel if enabled; otherwise
contact [security@zadp.io](mailto:security@zadp.io) before sharing details.
No guaranteed response time or independent security audit is advertised.

Never submit wallet seeds, spending keys, authentication/cookies, private seller
viewing keys, invoice inputs/tokens or funded wallet databases in reports, logs,
PRs or screenshots. Use disposable offline fixtures and sanitized reproductions.

Indexers use only board incoming viewing authority. An AI/LLM process must never
possess a Zcash spending key. Do not bypass sync, privacy, network or human
approval checks. A broadcast timeout is not authorization to retry a spend.

See [threat model](docs/threat-model.md) and [safe development](docs/development.md)
for current limits. Before any public deployment, review resource limits, TLS,
authentication, retention, refund/reorg policy and current dependency advisories.

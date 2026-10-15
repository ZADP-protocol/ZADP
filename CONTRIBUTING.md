# Contributing

Read [development](docs/development.md), [protocol](docs/protocol.md) and
[architectural decisions](docs/decisions.md). Keep discovery, wallet spending
and seller service execution boundaries separate.

Use small logical commits. Preserve Protocol 1.0 canonical wire 3 and historical wire 1/2 verification; document
any discrepancy before proposing a versioned breaking change. Add regression
tests for externally reachable failures, not tests that only mirror implementation.
Use English for code comments, UI and documentation.

Run the documented Rust/Python/Node checks and relative documentation link checker
before proposing a change. Offline tests must not contact a funded wallet or
broadcast. Keep local configuration, keys, databases and private invoice data
out of Git. Never recreate a working wallet as a troubleshooting shortcut.

Do not assume offer signature acceptance guarantees service quality or that
synthetic fixtures prove live payment settlement. State exactly what was verified.
No upstream Zallet PR is created by this repository's patch documentation;
coordinate upstream contribution according to the upstream project's guidelines.

Unless explicitly stated otherwise, contributions follow this repository's
existing dual MIT OR Apache-2.0 licensing declaration.

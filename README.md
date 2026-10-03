# ZAP — ZEC Agent Protocol

ZAP is an open protocol and reference implementation for **private commerce** between humans, applications, and autonomous AI agents using **Zcash**.

Sellers publish signed service offers through shielded channels. Indexers discover and validate them. Buyers (human or AI) request invoices and settle in shielded ZEC. Payment and order metadata can travel privately in shielded memos.

## Why Zcash is required

Zcash is not an interchangeable plugin payment method in ZAP. The protocol is built to demonstrate:

- shielded ZEC settlement
- private transaction metadata via shielded memos
- separation of **viewing** and **spending** authority
- public service discovery without publicly exposing customer purchase graphs
- pseudonymous sellers
- machine-to-machine private commerce

## Core principle

> The AI may **REQUEST** a payment.  
> The AI may **NEVER SIGN** a payment.

No AI/LLM process may possess a Zcash spending key.

## Architecture (MVP)

Rust workspace with domain boundaries:

| Crate | Role |
| --- | --- |
| `zap-protocol` | ZAP/1 messages, canonical signing, domain types |
| `zap-zcash` | `ZcashAdapter` trait + `MockZcashAdapter` |
| `zap-indexer` | Scan, validate, store offers |
| `zap-api` | HTTP/JSON REST |
| `zap-payment` | Spending policy + payment requests |
| `zap-seller` | Offers, invoices, fulfillment |
| `zap-demo` | End-to-end CLI vertical slice |

See [docs/architecture.md](docs/architecture.md).

## Current MVP status

**Phase 1 — local mock vertical slice**

| Piece | Status |
| --- | --- |
| Signed offers (ed25519 + canonical CBOR) | Real |
| Indexer validation + REST `GET /offers` | Real |
| Invoice + policy engine | Real |
| Zcash publish/scan/pay/observe | **Mocked** |
| MCP / web UI / production wallet | Not built |
| Real Zcash testnet | Not built |

What is real vs mocked is also called out in the demo output.

## Protocol layers

1. **ZAP Discovery** — signed `offer` messages (update/revoke specified, not implemented yet)
2. **ZAP Commerce** — quote / invoice / order over HTTP
3. **Zcash Shielded Settlement** — private payment + memo metadata (mock in Phase 1)

Details: [docs/protocol.md](docs/protocol.md)

## Security model

See [docs/threat-model.md](docs/threat-model.md). Highlights:

- Agent tool surface stops at `request_payment(invoice)`
- Payment controller enforces `ALLOW` / `DENY` / `REQUIRE_APPROVAL`
- Offer authenticity via public-key signatures independent of Zcash keys
- Indexer should hold viewing capability only, never spending keys

## How to run the demo

Requirements: Rust stable (1.80+ recommended).

```bash
cargo run -p zap-demo
```

This locally:

1. Publishes a signed seller offer through the mock Zcash adapter  
2. Indexes and verifies it  
3. Queries offers over REST  
4. Issues an invoice  
5. Evaluates payment policy  
6. Simulates a shielded payment  
7. Seller fulfills a mock inference job  

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Documentation

- [Architecture](docs/architecture.md)
- [Protocol ZAP/1](docs/protocol.md)
- [Threat model](docs/threat-model.md)
- [Roadmap](docs/roadmap.md)
- [Decisions](docs/decisions.md)
- [Zcash integration TODOs](docs/zcash-integration-todos.md)

## Roadmap (summary)

1. Local mock vertical slice ← **now**  
2. Real Zcash testnet integration  
3. MCP / AI buyer  
4. Web marketplace UI  
5. Hosted indexing / ZAP Cloud (optional commercial layer)

Open protocol remains useful without any hosted product.

## License

TBD (hackathon bootstrap).

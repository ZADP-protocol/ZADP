# ZADP

![ZADP neighborhood offerboard: private service discovery in a futuristic city](docs/images/zadp-neighborhood-offerboard.jpg)

**Zcash Agent Discovery Protocol**

[![ci](https://github.com/ZADP-protocol/ZADP/actions/workflows/ci.yml/badge.svg)](https://github.com/ZADP-protocol/ZADP/actions/workflows/ci.yml)

A permissionless service-discovery protocol where sellers publish signed offers through Zcash shielded memos and humans or autonomous agents discover them through independent, multi-board indexers.

**Publish services to a geographic area through Zcash and discover matching offers locally without publishing the buyer’s location.**

![ZADP concept: sellers publish shielded offers, independent indexers expose discovery, and humans or agents consume it](docs/images/zadp-ecosystem-concept.jpg)

*Concept illustration. Web/game interfaces, autonomous buyers and real purchase settlement are planned capabilities; live mainnet publication and independent discovery were verified on 2026-10-05. Paid service execution remains planned.*

## Why ZADP?

AI agents and applications increasingly need to discover paid services. Most discovery today depends on centralized registries. Public payment rails can also expose commercial relationships.

ZADP experiments with Zcash as a **permissionless publication** layer (and, later, a private settlement path). This is a design choice with specific properties—not a claim that blockchain discovery is universally better:

- permissionless publication to a dedicated shielded board
- cryptographically signed offers (seller Ed25519 identity, independent of Zcash keys)
- independently reproducible indexing from chain data + a portable board descriptor
- Zcash-native shielded memos for offer transport
- optional signed public service areas, with private buyer-side geographic matching
- pseudonymous sellers
- private buyer→seller settlement as a **related / future** capability (not required for discovery)

Open publication is subject to each board’s **admission policy** (e.g. open or operator-authorized sellers). Anyone can run an indexer; hosted indexing or a high-availability API may be offered commercially while the protocol remains self-hostable.

## How it works

```mermaid
flowchart LR
  Seller[Seller]
  Offer[Signed ZADP OFFER]
  Tx[Shielded Zcash tx + memo]
  Net[Zcash network]
  Idx[Indexer - read-only board viewing]
  API[REST API]
  Client[Buyer or AI agent]
  GPS[Private location - client memory only]
  Match[Local geographic match]

  Seller --> Offer --> Tx --> Net --> Idx --> API --> Client
  Client --> Match
  GPS --> Match
```

**Privacy boundary:** offer publication is intentionally discoverable by holders of the board’s public incoming viewing information. Purchases are **not** meant to go through that board viewing capability; settlement (when implemented for real) is direct buyer→seller.

**Hard invariant:** no AI/LLM process may possess a Zcash spending key. Agents may request payment through a separate controller; spending stays in a wallet boundary.

### Visual overview

![ZADP discovery and payment overview: signed offers, read-only board UIVK indexing, independent discovery and direct shielded payments](docs/images/zadp-discovery-and-settlement.png)

This conceptual overview separates board discovery from direct buyer-to-seller payments. Indexers use the board’s **Unified Incoming Viewing Key (UIVK)** to read offers and validate their signatures and admission policy; this capability cannot spend funds or read unrelated seller payments. Offers use compact CBOR within the 512-byte memo budget. The implemented discovery endpoints are `GET /offers` and `GET /healthz`.

Mainnet offer discovery and local self-payment fulfillment have been verified. An independent buyer purchase and public service deployment remain pending. The pictured service listings are illustrative. See [Protocol](docs/protocol.md), [Mainnet acceptance](docs/mainnet-acceptance.md) and [Paid service flow](docs/paid-service-flow.md) for implementation details.

## Current status

| Area | Status |
| --- | --- |
| Signed `offer` messages, Ed25519 seller identity | **Implemented** |
| Canonical signing + 512-byte memo framing (`0xf5` + `ZADP` + wire v1/v2 + CBOR) | **Implemented** |
| Board descriptor, board binding, admission policy | **Implemented** |
| CompactTxStreamer scanner (trial decrypt + full memo fetch) | **Implemented** (code + offline tests) |
| SQLite indexer, cursor, replay / reorg handling | **Implemented** |
| Mainnet board HTTP: `GET /offers`, `GET /healthz` | **Implemented** |
| Multi-board registry, external UIVK-only indexing and host moderation | **Implemented reference path** — isolated per-board indices, shared transport, filtered API; [operator guide](docs/multi-board.md) |
| Seller CLI: `identity-*`, `board-sign`, `publish` (Zallet PCZT) | **Implemented** (developer preview) |
| Mock demo: invoice / policy / mock pay / fulfill | **Implemented** (local only) |
| Live mainnet publish → mine → scan → API acceptance | **Verified 2026-10-05** — mined OFFER + two independent indices; [public evidence](docs/mainnet-acceptance.md) |
| Payment-gated text service | **Implemented experimental local path** — authenticated seller observer + persistent invoices; synthetic tests, live buyer payment still pending |
| Local web UI | **Implemented** — discovery, drafts, exact memo budget, optional read-only wallet/QR, unsigned publication costs and local invoice instructions |
| Geographic discovery | **Implemented reference path** — signed Offer v2 area, v1-compatible indexing/API, local Nearby/Anywhere matching; [guide](docs/geographic-discovery.md) |
| MCP / reputation / hosted marketplace | **Not implemented** |
| `offer_update` / `offer_revoke` | **Specified only** |

Do **not** treat CompactTxStreamer support or offline fixtures as proof of live-network acceptance.

## Protocol overview

1. **Discovery** — seller publishes a board-bound signed `offer` in a shielded memo.
2. **Indexing** — indexer scans with the board UIVK, validates, and stores current offer state.
3. **Local discovery** — human or agent downloads public offers and intersects public offer areas with a private local search circle.
4. **Commerce** — buyer contacts the seller `endpoint` directly (HTTP).
5. **Settlement** — future/related flow: shielded ZEC buyer→seller; not the board-discovery path.

### Local discovery

A seller can publish **“Cabin cleaning — available within 50 km”** with an
approximate public service-area center. The reference seller UI rounds
browser-acquired coordinates to reduce accidental disclosure of precise location.
A buyer can choose **“Show offers near
me within 25 km”**. The buyer’s location stays in client memory; it is not sent
to Zcash, the indexer, discovery API, seller or commerce service. Nearby matches
intersecting geographic areas; Anywhere also includes global offers. A zero
search radius means “the offer area includes my point”. Neither center nor
signature proves the seller’s physical position or service quality.

The browser and [small agent example](examples/geo/discover.js) use the same
local matcher and public `GET /offers` data. GPS is requested only on a click;
there is no server geo-search endpoint, map provider or automatic persistence.
[Geo protocol, privacy boundaries and offline demo](docs/geographic-discovery.md).

### Read offers without control of funds

An indexer uses the board's **Unified Incoming Viewing Key (UIVK)** to decrypt
incoming board outputs and read their offer memos and values. This key **cannot
sign transactions or spend ZEC**. It is intentionally included in the signed
public board descriptor, so independent operators can reconstruct discovery
without access to a wallet or its spending keys. The first mainnet offer was
verified this way by two independent indices.

The browser UI only reads the indexer's HTTP API; it does not need even the
UIVK. Publish only a dedicated board's incoming viewing authority: never a
personal seller viewing key, full viewing key, seed or spending key. Board
content is readable by descriptor holders; this is public discovery, not a
confidential mailbox. Details: [board descriptor](docs/board-descriptor.md).

## Zcash architecture

ZADP uses Zcash for:

- shielded outputs carrying encrypted memos
- separation of **incoming viewing** (board/indexer) from **spending** (seller wallet only)
- CompactTxStreamer / lightwalletd-style scanning of compact blocks

A public board UIVK makes board-bound values and memos visible to descriptor holders by design. That is narrower than a full viewing key and does **not** authorize spends. It is not stronger privacy than Zcash itself provides.

Publication today targets local **Zallet** PCZT RPC; scanning uses a lightwalletd-compatible service. See [docs/mainnet-publishing.md](docs/mainnet-publishing.md) and [docs/zcash-scanning.md](docs/zcash-scanning.md).

## Illustrative offer (human-readable)

JSON for humans/REST—not the canonical wire/memo encoding. Field names match the protocol (`zadp`, not a legacy `zap` key):

```json
{
  "zadp": 1,
  "type": "offer",
  "id": "01JEXAMPLE…",
  "seller": "ed25519:…",
  "capability": "llm.inference",
  "title": "Example inference",
  "price": { "amount": "0.01", "currency": "ZEC" },
  "endpoint": "https://seller.example/zadp",
  "expires_at": "2026-10-04T12:00:00Z",
  "nonce": "…",
  "board_id": "…",
  "issued_at": "2026-10-03T12:00:00Z",
  "signature": "…"
}
```

Canonical signing and memo layout: [docs/protocol.md](docs/protocol.md).

## Quick start

### Local mock demo

End-to-end mock discovery + invoice + policy + mock payment (no ZEC):

```sh
cargo run -p zadp-demo
```

### Real Zcash scanner (developer preview)

Prerequisites: synchronized CompactTxStreamer/lightwalletd endpoint, signed public `board.json` (Orchard-only UA + UIVK + birthday), Rust stable.

```sh
cargo run -p zadp-api -- --network mainnet --board board.json \
  --lightwalletd https://YOUR-SERVICE:PORT \
  --database board.sqlite --listen 127.0.0.1:8080
```

```sh
curl 'http://127.0.0.1:8080/offers?capability=llm.inference'
```

Funded publication (`zadp-seller publish`, optional `--dry-run`) is a separate privileged path: [Mainnet publishing](docs/mainnet-publishing.md). Automated tests never broadcast.

## API

### Mainnet board router (`zadp-api` binary)

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `/healthz` | Liveness |
| `GET` | `/offers?capability=` | Indexed offers + chain metadata |

There is **no** `GET /offers/{id}` in the current mainnet router.

### Mock commerce router (demo / `router()` helper)

Used by `zadp-demo` only—not the production board daemon:

| Method | Path |
| --- | --- |
| `GET` | `/healthz` |
| `GET` | `/offers` |
| `POST` | `/v1/invoices` |
| `POST` | `/v1/fulfill` |

## Crates

| Crate | Role |
| --- | --- |
| `zadp-protocol` | Messages, board descriptor, signing, memo encode/decode |
| `zadp-zcash` | `BoardSource` / scanner, mock adapter, lightwalletd client |
| `zadp-indexer` | Validation, SQLite persistence, reorg-aware sync |
| `zadp-api` | Board scanner binary + HTTP routers |
| `zadp-seller` | Identity, `board-sign`, Zallet `publish` CLI + mock seller service |
| `zadp-payment` | Spending policy + `request_payment` (mock path) |
| `zadp-demo` | One-command mock vertical slice |

## Security model

- AI/LLM processes never hold Zcash spending keys
- Offers are signed; memos and chain data are untrusted input
- Board policy, expiry, replay/duplicate handling in the indexer
- Seller publish path: local wallet only, explicit confirmation (default), dry-run available

See [docs/threat-model.md](docs/threat-model.md).

## Decentralization and hosting

ZADP is a **protocol**, not a mandatory marketplace. Operators can publish boards and run indexers independently. Optional hosted indexing or managed APIs can coexist; they must not be required to speak the protocol.

## Roadmap (summary)

1. **Done:** first live mainnet offer publication and independent discovery; [evidence](docs/mainnet-acceptance.md)
2. **Done:** locally tested English UI, Rust memo parity, read-only wallet and unsigned fee previews
3. **Done locally:** direct seller commerce reference, anonymous orders, ZIP-321 QR and mock fulfillment
4. **Next:** approve an independent real buyer-to-seller test after reviewing endpoint, invoice and transaction costs
5. Stable self-hosted API / ops docs
6. Optional hosted indexing
7. Agent/MCP buyer tools (still no spending keys in the model)
8. Production hardening and broader shielded purchase support

Details: [docs/roadmap.md](docs/roadmap.md).

## Direct seller commerce reference

The independently runnable Rust seller service implements anonymous, persistent
`text.uppercase` orders. Browsers call the seller directly; discovery stores no
buyer input, payment identifiers or results. Payment requests use ZIP-321 with
a unique order memo and complete QR. The local vertical slice uses simulated
evidence and sends no funds. See [Direct seller commerce](docs/commerce.md) for
API, isolated demo, boundaries and the separate real-payment approval gate.

## Local interface

Run `python3 ui/server.py` and open http://127.0.0.1:8090 with a local indexer on port 8080. The English-only UI shows real indexed offers, saves unsigned drafts in the browser, and includes a free local text-shortening test service (no AI model or payment). Optional protected operator integration shows filtered seller wallet readiness and a receive QR, prepares authoritative unsigned publication costs, and bridges invoices/payment checks to the local TLS text service. No UI route signs, broadcasts or transfers funds. Live buyer payment remains pending separate approval. See [UI instructions](ui/README.md).

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

CI (fmt, clippy, tests, demo smoke): `.github/workflows/ci.yml`.

## Agent coordination (optional, local)

Operators may use a local file inbox for handing tasks between agents (pending → result → archive). That path is **gitignored** and is not part of the public protocol surface.

## Documentation

- [Architecture](docs/architecture.md)
- [Direct seller commerce](docs/commerce.md)
- [Multi-board indexing and policy boundaries](docs/multi-board.md)
- [Protocol](docs/protocol.md)
- [Board descriptor](docs/board-descriptor.md)
- [Zcash scanning](docs/zcash-scanning.md)
- [Mainnet publishing](docs/mainnet-publishing.md)
- [Threat model](docs/threat-model.md)
- [Roadmap](docs/roadmap.md)
- [Zcash integration TODOs](docs/zcash-integration-todos.md)

## License

Workspace `Cargo.toml` declares **`MIT OR Apache-2.0`**. Standalone `LICENSE` / `LICENSE-*` files are **not** present in the repository yet; treat SPDX as intended dual-licensing pending finalized license text files.


## Developer and release documents

- [Development and troubleshooting](docs/development.md)
- [Paid-service milestone design](docs/paid-service-flow.md)
- [Dependency review](docs/dependencies.md)
- [Two-minute demo](docs/hackathon-demo.md) and [submission draft](docs/hackathon-submission-draft.md)
- [Security reporting](SECURITY.md), [contributing](CONTRIBUTING.md), [dual license](LICENSE)

These are preparation materials, not a public deployment, hackathon submission
or claim that real buyer settlement is complete.

## Principal Sponsors

_No principal sponsors yet._

**Principal Sponsor — 1.0 ZEC**

Public name/project listing with one HTTPS link, displayed above normal Sponsors.
README visibility only; no logos, badges or additional marketing text.

**Pay with ZEC:** scan the QR below or copy this complete ZIP-321 payment request into your Zcash
wallet. It specifies the project recipient and exact tier amount, with no memo:

```text
zcash:u1x599z4k4ff286gfkajzpmzj9g98xqk2cr9tkktnwr9fv7h6sjrzznttxpjuetfcu7ac722l2x8959s435vn20ll2s0zkgh67qhlhrl2mjg32q27t3lep20fenunkr5gr2umz32gh23hkp85yhqnf8s4lsdrqa8kr8emxv75mvqqlymau?amount=1.0
```

<img src="docs/images/zadp-principal-sponsor-qr.svg" alt="Principal Sponsor payment request QR: 1.0 ZEC to the project Unified Address, no memo" width="320">

[PNG QR fallback](docs/images/zadp-principal-sponsor-qr.png) · **1.0 ZEC**, excluding network fees.
Scan the QR to load the recipient and amount; **add your sponsorship memo manually** before paying.

Manually enter this memo in your wallet before approving payment:

```text
ZADP-SPONSOR|principal|public=YOUR_NAME|url=https://YOUR_URL
```

Example:

```text
ZADP-SPONSOR|principal|public=AliceLabs|url=https://alice.example
```

## Sponsors

_No sponsors yet._

**Sponsor — 0.1 ZEC**

Public name/project listing with one HTTPS link in this Sponsors section.
README visibility only; no logos, badges or additional marketing text.

**Pay with ZEC:** scan the QR below or copy this complete ZIP-321 payment request into your Zcash
wallet. It specifies the same project recipient and exact tier amount, with no memo:

```text
zcash:u1x599z4k4ff286gfkajzpmzj9g98xqk2cr9tkktnwr9fv7h6sjrzznttxpjuetfcu7ac722l2x8959s435vn20ll2s0zkgh67qhlhrl2mjg32q27t3lep20fenunkr5gr2umz32gh23hkp85yhqnf8s4lsdrqa8kr8emxv75mvqqlymau?amount=0.1
```

<img src="docs/images/zadp-sponsor-qr.svg" alt="Sponsor payment request QR: 0.1 ZEC to the project Unified Address, no memo" width="320">

[PNG QR fallback](docs/images/zadp-sponsor-qr.png) · **0.1 ZEC**, excluding network fees.
Scan the QR to load the recipient and amount; **add your sponsorship memo manually** before paying.

Manually enter this memo in your wallet before approving payment:

```text
ZADP-SPONSOR|sponsor|public=YOUR_NAME|url=https://YOUR_URL
```

Example:

```text
ZADP-SPONSOR|sponsor|public=AliceLabs|url=https://alice.example
```

**Sponsor ZADP** — Purchase project sponsorship visibility with ZEC.
Project sponsorship placements are available in ZEC.

Sponsor listings have no fixed term and remain visible while the ZADP Sponsors section is maintained.

Both tiers use the configured public project Unified Address:

```text
u1x599z4k4ff286gfkajzpmzj9g98xqk2cr9tkktnwr9fv7h6sjrzznttxpjuetfcu7ac722l2x8959s435vn20ll2s0zkgh67qhlhrl2mjg32q27t3lep20fenunkr5gr2umz32gh23hkp85yhqnf8s4lsdrqa8kr8emxv75mvqqlymau
```

`public=` is the name/project name you authorize us to publish. `url=` is the
single HTTPS link you authorize us to publish. Replace the placeholders yourself;
sponsor names and links are not automatically included in payment requests.
Network fees are additional: review the recipient, amount, memo and wallet fee
before sending. Never send a second payment merely because a listing is pending.

Both tier QRs encode exactly their complete ZIP-321 URI above: the project
Unified Address and tier amount only, with no sponsor name, URL or memo. Wallet
ZIP-321 support is required; verify the amount and recipient after scanning.
QRs use square dark modules (`#0D1117`) on an opaque white background with a
four-module quiet zone, no logo, and no transparent background. SVGs and PNG
fallbacks remain readable on GitHub light and dark backgrounds.

**Manual review and publication:** the maintainer checks the wallet and shielded
memo, confirms the tier/amount, reviews the name and HTTPS link, and manually
adds approved listings as simple Markdown text links. Publication is not
automatic. There is no sponsorship watcher, sponsor database, backend API or
automatic README/GitHub update. The project may refuse names or links containing
malicious links, impersonation, illegal content, deceptive affiliation claims,
malware or abusive content. Payment does not bypass manual review.

**Privacy:** the payment may be shielded, but the value provided in `public=`
and the `url=` link are intentionally published in the README after manual
approval. Do not include seeds, spending keys, wallet credentials or other
private information in the memo.

Project sponsorship is separate from the ZADP protocol. Neither tier grants
protocol privileges or affects offer ranking, board admission, protocol validity,
moderation, discovery, seller commerce, indexer behavior or reputation. A
Principal Sponsor receives README visibility only.

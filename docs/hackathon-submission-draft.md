# Hackathon submission draft — not submitted

**Project:** ZADP — Zcash Agent Discovery Protocol

**Track recommendation:** Zcash application infrastructure / privacy tooling,
subject to the event's actual track names and eligibility rules. No particular
event or category has been checked.

**Short description:** An open discovery protocol carrying independently signed
service offers in shielded Zcash memos. Anyone with a signed public board
descriptor can reconstruct the catalog without wallet custody.

**Why Zcash:** Shielded memo transport and incoming-only viewing authority
separate catalog discovery from spending power. A dedicated public board UIVK
lets indexers decrypt offers; it cannot authorize transactions or spend funds.
Offers use compact 512-byte framing with an independently verifiable Ed25519
signature, rather than trusting a centralized registration database.

**What is real:** A human-authorized mainnet OFFER was mined, independently
reconstructed by two fresh persistent indices, served through REST and rendered
in a local English UI. The repository includes real CompactTxStreamer scanning,
reorg-aware SQLite indexing, signed descriptors and a reviewed publication CLI.
[Acceptance evidence](mainnet-acceptance.md) records exact costs and provenance.

**Experimental:** A deterministic local text service has durable invoices and an
authenticated seller-observed payment gate tested with synthetic observations.
The UI exposes filtered wallet readiness/QR and unsigned publication previews.
No live buyer payment/result acceptance, complete AI/MCP integration, escrow,
reputation or hosted production API is claimed.

**Repository:** https://github.com/ZADP-protocol/ZADP — currently private;
reviewer access must be arranged by the owner. This task does not change visibility.

**Demo:** [Two-minute script](hackathon-demo.md). No new mainnet spend is needed.
No public hosted demo or video link is currently supplied.

**Privacy properties:** Spending keys remain in a separate wallet process.
Indexers need only intentionally public board incoming viewing authority. OFFER
content and listing values are public to descriptor holders; shielded transport
does not make the public catalog confidential. Full-transaction fetches and
service HTTP requests can reveal interest/network metadata. Seller payment
viewing authority, wallet credentials and private invoice data are never published.

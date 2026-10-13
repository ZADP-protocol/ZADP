# Optional hosted policy is outside Core

Moderation rules, browser validation, reports, operator decisions, admin UI and
hosted visibility adapters live in the separate [ZADP-Hosted repository](https://github.com/ZADP-protocol/ZADP-Hosted).

Core owns canonical board validation, append-only Offer/revoke history, reorgs,
active lifecycle queries and the standalone `zadp-api` discovery runtime. It has
no ZADP-Hosted dependency and creates no moderation/report tables. Its reads do
not apply existing operator moderation tables. Operators can run Core alone.

ZADP-Hosted explicitly composes the canonical runtime and its generic local
SQLite extension interface. Existing operator tables are preserved as application
metadata; their schemas, migration and filtering are owned by Hosted. Protocol
Offer wire 3, legacy bytes, signing domains and BoardDescriptor v1 are unchanged.

Do not expose standalone unfiltered Core routes behind a moderated public service.
Use `zadp-hosted` when operator visibility policy and report/admin endpoints are
required. Server owns deployment configuration, not content rules.

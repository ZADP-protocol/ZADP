# Dependency review — 2026-10-06

Scope: manifests, Cargo.lock, resolved Cargo metadata and duplicate dependency
tree. No large dependency upgrades were performed. Cargo.lock package versions
and checksums were preserved; only direct workspace dependency edges changed.

- Removed unused direct `async-trait` dependencies from `zadp-payment` and
  `zadp-seller`; source review found no usage in either crate. Other manifests
  retain their dependency surface pending an automated unused-dependency audit.
- Added `async-trait` as an API test dependency for a failing repository fixture.
- The full resolved metadata includes 394 packages. `cargo tree -d` can show the
  same package/version twice because host/target feature contexts differ; this
  does not necessarily mean duplicated versions or a source fork.
- Actual parallel versions include rand 0.8.8/0.9.5/0.10.3, thiserror 1.0.69/2.0.21,
  sha2 0.10.9/0.11.0-pre.4, digest 0.10.7/0.11.0-pre.9 and syn 2.0.119/3.0.6.
  Newer rand may be target-specific and absent from the default host tree.
  Pre-release cryptographic dependency families belong to the Zcash cohort;
  merging/upgrading them without upstream compatibility testing is risky.
- The scanner's explicit rustls ring provider handles the two transitive crypto
  providers. Keep TLS and consensus-branch checks; do not solve conflicts by
  disabling certificate verification.
- QR is the only Python dependency, pinned to qrcode 8.2; Node tests use standard
  libraries. No UI build framework or fragile browser CI dependency was added.
- Local checks ran with Rust/Cargo 1.99.0, Node 22.19.0 and Python 3.12.3. CI tracks
  stable Rust; no independently tested minimum Rust version is advertised.

`cargo-machete` and `cargo-audit` were not installed in the inspected environment.
No current vulnerability database, latest-version comparison, license compatibility
or full transitive feature audit was performed. This review must not be described
as a clean security advisory scan. Before a public release, run an up-to-date
RustSec audit and review advisories with the relevant upstream Zcash maintainers;
prefer cohesive pinned upgrades over forcing a single transitive version.

## Direct commerce additions

The reference commerce crate adds `qrcode` 0.14.1 and uses the existing locked
`zip321` 0.9.0 cohort to construct payment requests. The browser vendors
`qrcode-generator` 1.4.4 under MIT (source and license in `ui/vendor`); it encodes
validated ZIP-321 data locally without sending payment details to a QR service.
These additions postdate the dependency counts above. A current advisory audit
is still required before production release.

# Public-release source review — 2026-10-15

Review baseline: GitHub main `4a2847461b60eaa02fe416b168039baaf1813bc0`.
The repository was **private** when checked. This document neither changes
visibility nor authorizes a production release or transaction.

## Changes in this review

- Removed Principal Sponsors, their payment instructions and the two principal
  QR assets from the current tree. Kept the ordinary Sponsors section, its
  0.1 ZEC URI, SVG/PNG QR, public recipient and manual review instructions.
- Corrected obsolete Basic Auth claims, pre-extraction Core hosted-policy
  instructions, Protocol 1.0 contributor wording and repository boundaries.
- Added Hosted/MCP roles without introducing dependencies on private repos.
- Distinguished dated mainnet evidence from current operator state; removed
  unsupported blanket statements about current charging/availability.
- Updated the security contact and documented the current dependency audit.
- Unauthenticated public discovery health returned HTTP 200 during the review.
  Web and Hosted pin Core `1ac8011`; main includes newer seller-only protection
  commits. A consumer pin is an intentional reviewed dependency, not a reason
  to deploy or rewrite it automatically.

No protocol, wire bytes, runtime source, Cargo.lock, wallet authority, deployed
release, history, branch deletion or repository visibility changed.

## GitHub branches other than main

`codex/discovery-api-throttle` (`2126688`) is **9 commits behind**, with **0 unique
commits** relative to main. Its aggregate-throttle work is already incorporated;
it is a cleanup candidate, not missing functionality.

`codex/offer-commercial-modes` (`7c3e56f`) is **16 behind / 3 unique commits**.
The three unique commits are documentation-only:

- `952b250`: adds `docs/images/zadp-architecture.svg`.
- `5b74ca0`: switches README to that diagram.
- `7c3e56f`: removes the earlier overview PNG.

Do not merge the old branch wholesale: its base predates revocation, moderation
extraction, IP throttling and seller DoS work. Review/reuse the SVG separately if
wanted. Main already contains the actual Protocol 1.0 commercial-mode runtime.
Neither remote branch was deleted, merged or pushed by this review.

## Local branch state

The local main pointer was 9 commits behind origin/main, with no unique work;
it was fast-forwarded to the reviewed upstream baseline. The checkout was clean
before creating `codex/public-release-doc-review`.

The following existing local branches have no unique commits relative to that
baseline: `codex/agent-transfer-proposal-size`, `codex/client-ip-throttle`,
`codex/commerce-endpoint-proof`, `codex/discovery-api-throttle`,
`codex/extract-hosted-moderation`, `codex/hosted-offer-lifetime`,
`codex/launch-moderation`, `codex/offer-commercial-modes`, `codex/offer-revoke`,
`codex/seller-dos-protection`, `codex/seller-read-only-observer`,
`codex/service-endpoint-semantics`, `codex/usd-publication-pricing`.
Their historical feature work is integrated, even where the local branch's name
matches a remote branch whose tip differs.

Two local branches need preservation/review rather than an automatic merge:

- `feat/hosted-seller-policy`: 34 behind / 1 unique old allowlist commit; attached
  to another worktree. Hosted policy has since moved out of Core.
- `privacy-backup-before-geo-rewrite`: 41 behind / 6 unique historical geo/UI
  commits. Backup of pre-wire-3 work, not a current implementation candidate.

Pruned remote-tracking refs for three already-deleted GitHub branches were
removed by fetch; local branch/worktree contents were preserved.

## Checks and remaining release decisions

- Baseline main CI passed before this documentation-only review.
- Relative documentation link checks and Rust formatting checks are recorded in
  the task results. No new behavioral tests are needed for unchanged runtime.
- Recognizable-pattern screening inspected **785 unique blobs** reachable from
  main and both GitHub feature branches. No private-key header, GitHub token,
  bcrypt hash or AWS access-ID pattern was found; no blobs exceeded the 4MB
  screening limit. No currently tracked `.key`, `.pem`, seed, wallet identity,
  database or actual credential-env file was found.
- This is **not** a full mnemonic/custom-secret or image-content audit. Historical
  Web/product and moderation commits still exist; removing files from HEAD does
  not erase them from Git history. Review the history deliberately before public
  visibility. No history rewrite or credential rotation was attempted.
- Two author email identities occur in reachable local history; whether to retain
  them publicly is the owner's decision. No author metadata was rewritten.
- Main was not protected on GitHub when inspected. Consider required PR/check
  protection before accepting public contributions; no repo settings changed.
- Dependency audit **does not pass**: four rustls advisories plus one unmaintained
  transitive dependency warning. See [dependency review](dependencies.md).
  Update/retest dependencies before calling this a clean public-release audit.

The separate release checklist was removed at the owner's request on 2026-10-17.
This dated review still records its findings; removal does not resolve them or
change repository visibility. Detailed operator configuration and private
acceptance records remain in their own repositories.

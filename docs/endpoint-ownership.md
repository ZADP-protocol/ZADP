# Commerce endpoint ownership

A commerce Offer does not merely advertise an endpoint. The endpoint cryptographically proves that it is controlled by the same seller identity that signed the Offer.

This is a separate, off-chain proof, not an Offer field or a board admission rule. Offer v1/v2 wire formats, canonical signing, IDs, geo, commerce and BoardDescriptor v1 are unchanged. Discovery-only Offers do not require a proof. Ownership verification does not replace seller approval, confirm service quality, guarantee fulfillment, or authorize payment.

## Generate offline

Use an existing protected seller **identity** file on the seller's private signing computer. This is the Ed25519 Offer identity, not a Zcash spending key. No wallet or blockchain connection is used:

```sh
zadp-seller endpoint-proof \
  --identity /private/seller.identity \
  --origin https://seller.example \
  --expires-at 2027-01-01T00:00:00Z \
  --output /private/public-endpoint-proof.json
```

Choose a future expiry yourself. The output file must not already exist. The command uses the current whole-second UTC issue time and validates its own result. The JSON contains only `zadp`, `type`, seller public identity, exact normalized HTTPS origin, issue/expiry timestamps and public signature. It never contains the private identity or spending key.

Deploy **only the public proof JSON** to `GET https://seller.example/.well-known/zadp`, with HTTP 200. It can be a static file. Do not copy the signing identity to the public seller server or to zadp.io. Rotate the proof by generating and deploying another offline-signed document before expiry; the server does not renew proofs itself.

The reference seller accepts the optional public file:

```sh
zadp-commerce --config /private/seller-config.json \
  --endpoint-proof /public/public-endpoint-proof.json \
  --bind 127.0.0.1:8094
```

At startup it validates the proof against its configured Offer's seller and origin. It serves the public JSON at the well-known path without a claim token. After expiry it returns 404. Restart with the replacement proof to rotate it. The existing default without a proof still supports isolated local simulations, but does not qualify for verified hosted commerce. A reverse proxy may serve the same static document instead; proof generation never requires a public server to hold a signing key.

## Canonical proof format

Version 1 uses JSON for interchange, rejects unknown fields and duplicate fields, and uses the existing canonical CBOR encoder for signing:

```text
ASCII "ZADP-ENDPOINT-PROOF/1" + NUL
+ canonical CBOR array:
[1, "commerce-endpoint", seller, origin, issued_unix_seconds, expires_unix_seconds]
```

All seven JSON fields are required. The signature is the existing Base64 Ed25519 encoding. Timestamps are UTC RFC3339 at whole-second precision; fractional seconds are rejected. The proof origin must already be the exact normalized origin, with no trailing slash, path, query, fragment or credentials. URL normalization follows the URL parser: lowercase/IDNA hostname and omitted default HTTPS port; non-default ports remain part of the origin. An Offer endpoint may have a service path, but ownership covers its origin. The reference order client still supports only root-origin commerce endpoints.

Accept only version/type 1/`commerce-endpoint`, `issued_at <= now < expires_at`, a valid signature, exact Offer seller equality, and exact normalized origin equality. Copying a proof to another origin or advertising another seller's proven origin cannot pass. There is no key delegation in this format.

## Hosted availability, separately from signed discovery

The read-only API adds unsigned presentation metadata alongside the unchanged signed Offer:

```json
{"commerce_verification":{"status":"verified","origin":"https://seller.example","expires_at":"2027-01-01T00:00:00Z","valid_until":"2026-12-01T12:00:30Z"}}
```

Statuses are `discovery_only`, `unverified`, `unavailable`, or `verified`. Missing proof, invalid proof, expired proof, unsafe endpoint or a pending check is unverified. DNS/HTTP/TLS failure is unavailable. Neither status deletes or changes the canonical Offer. Expired Offers remain excluded by existing active-discovery rules.

The hosted UI enables Buy only with current verified metadata matching the endpoint origin; it also checks proof expiry and the short host attestation lease (`valid_until`) locally. Saved verification metadata cannot enable a new purchase indefinitely. That metadata is an attestation from the connected host, not an independent browser signature verification. Browser orders, claim tokens, payment correlation, input and results still travel directly to the seller, never to the discovery host. Buyer location stays in browser memory and is unrelated to proof fetching.

Host visibility/approval runs first. A verified endpoint does not admit a blocked/unapproved seller. Conversely, an otherwise approved discovery Offer remains visible if its commerce proof cannot be verified.

## Restricted fetching

There is no URL-fetch HTTP route. Only origins extracted from visible, cryptographically valid commerce Offers are checked, and only `/.well-known/zadp` is fetched. Credentialed/HTTP/local/private/link-local/reserved endpoints are rejected. Every DNS address must be ordinary public unicast; the validated address snapshot is pinned for TLS connection, preventing a second DNS lookup from redirecting to a private target. Normal TLS hostname/certificate checks remain enabled. Redirects and environment proxies are disabled. No browser credentials, buyer coordinates, seller secrets or order data enter these requests.

Checks run asynchronously with four concurrent requests, at most 512 in-memory cached origins, a five-second total deadline including DNS, and a 16 KiB response cap. Cached results are reused for less than 60 seconds, with proof expiry and seller/origin/signature checked again for each Offer. The first discovery response may be unverified while checking; normal feed refresh obtains the result. Stale checks disable buying until revalidated. Downtime can take up to this cache interval to be noticed; an expiry is never extended by caching. The cache is not canonical database state and is empty after restart.

No hosted or seller deployment, payment, publication, wallet route or custody change is required to implement this feature. Deployment must preserve Basic Auth and the existing read-only hosted API.

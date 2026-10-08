# Reference browser modules

Protocol-neutral JavaScript, with no dependency on a hosted web product:

- `offers.js`: public Offer JSON validation, optional-commerce parsing, geographic validation, Haversine area matching, and a volatile buyer-location controller.
- `geo.js`: CommonJS entry for the same matcher; no duplicated math.
- `memo-budget.js`: byte-budget estimator checked against the actual Rust encoder. Supports a generic `ttl_seconds`; the signed board still decides permitted lifetimes.

Load offers.js as a classic browser script, or require these modules from Node. Coordinates are accepted by local matching/controller methods only; no network, storage or logging operation is part of those modules. Seller browser acquisition rounds to three decimals for review; manual e5 precision remains explicit.

The parser checks shape and network metadata; it does **not** independently verify Ed25519 signatures. Use the Rust protocol verifier for cryptographic verification. Do not mistake API acceptance for browser verification.

Run the `test_*.js` files here from the repository root. The memo parity test invokes the local Rust size oracle with ephemeral fixture identities; it never uses a real wallet or broadcasts.

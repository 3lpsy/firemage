# Request signing

Outbound header injection and signing with owner-scoped secret resolution.

- DTOs and validation work on WASM with default features disabled.
- AWS SigV4 uses the official AWS signer, including S3 path and payload settings.
- HMAC-SHA256 signs `METHOD\nPATH?QUERY\nUNIX_TIMESTAMP\nSHA256(body)` and sends `x-firemage-date`.
- Signing keys must reference named secrets. Replaced headers discard guest duplicates.
- Tests use the published AWS S3 GET signature and RFC 4231 HMAC vector.

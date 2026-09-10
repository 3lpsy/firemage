Shared JSON/TOML contracts for VM management, authentication and guest setup.

- VM specifications default to jailed host isolation; trusted and external modes require explicit selection.
- Validates resource limits, boot inputs, guest paths, environment references and network attachments.
- Carries secret references without secret values; egress/signing contracts remain WASM-compatible.
- Host allowlists and operational prerequisites are enforced by the runtime rather than these transport types.

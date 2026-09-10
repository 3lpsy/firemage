Shared JSON/TOML contracts for VM management, authentication and guest setup.

- VM specifications default to jailed host isolation; trusted and external modes require explicit selection.
- Validates resource limits, boot inputs, guest paths, environment references and network attachments.
- Carries secret references without secret values; egress/signing contracts remain WASM-compatible.
- OCI assets require a SHA-256 reference and bounded disk size; registry basic/bearer credentials and custom CAs use vault references, with an explicit HTTPS token realm when needed.
- Host allowlists and operational prerequisites are enforced by the runtime rather than these transport types.
- Kernel catalog entries expose filenames, aliases, byte sizes and VM reference counts; kernel assets select a catalog filename.

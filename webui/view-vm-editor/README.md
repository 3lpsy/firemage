# view-vm-editor

Guided VM definition and a complete TOML configuration editor.

- `view` crate in the desktop web UI.
- Guided creation defaults to jailed host isolation; external sockets use explicit external mode.
- OCI registry access selects owner-vault credentials, a trusted token endpoint, and custom CA.
- Optional JSON fields are omitted when converting to TOML.
- The server validates paths, assets, permissions, and runtime constraints.

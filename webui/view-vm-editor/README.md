# view-vm-editor

Guided VM definition and a complete TOML configuration editor.

- `view` crate in the desktop web UI.
- Guided creation and editing preserve settings outside the form. Creation defaults to jailed host isolation; external sockets use explicit external mode.
- Kernel selection searches catalog filenames and aliases; no free-form kernel paths.
- OCI registry access selects owner-vault credentials, a trusted token endpoint, and custom CA.
- Inline attachments select reusable assets and set guest destinations and permissions.
- Optional JSON fields are omitted when converting to TOML.
- The server validates paths, assets, permissions, and runtime constraints.

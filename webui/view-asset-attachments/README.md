# view-asset-attachments

Shared library asset picker and per-VM attachment fields.

- `view` crate used by guided VM configuration and boot input editing.
- Searches aliases and filenames; saves stable asset IDs with absolute guest destinations.
- UID/GID and octal permissions are validated before saving, with no fallback for invalid input.
- Library fetches use the current owner's authenticated API session.

# view-asset-attachments

Shared asset and secret file picker with per-VM destinations.

- `view` crate used by guided VM configuration and boot input editing.
- Searches aliases and filenames; saves stable asset IDs with absolute guest destinations.
- Secret references contain names only and default to mode `0600`.
- UID/GID and octal permissions are validated before saving, with no fallback for invalid input.
- Library fetches use the current owner's authenticated API session.

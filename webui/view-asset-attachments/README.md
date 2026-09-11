# view-asset-attachments

Shared asset and secret file picker with per-VM destinations.

- `view` crate used by guided VM configuration and boot input editing.
- Searches aliases and filenames; saves stable asset IDs with absolute guest destinations.
- Secret references contain names only and default to mode `0600`.
- UID/GID and octal permissions share one row and are validated before saving.
- Remove sits beside each destination label; asset refresh uses an accessible icon button.
- Library fetches use the current owner's authenticated API session.

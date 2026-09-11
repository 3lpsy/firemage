# Catalog files

Confined filesystem operations shared by kernel and uploaded-file catalogs.

- Opens every directory component without following symlinks and anchors operations to an open descriptor.
- Rejects filename traversal, symbolic links, hard links and non-regular files.
- Applies caller-provided size bounds; temporary files publish atomically without overwriting existing files.
- Streams public HTTPS downloads with per-hop DNS pinning, size limits, optional SHA-256 verification and a 300-second deadline.
- Contains no authorization or database logic. Catalog callers enforce ownership and reference locks.

# Catalog files

Confined filesystem operations shared by kernel and uploaded-file catalogs.

- Opens every directory component without following symlinks and anchors operations to an open descriptor.
- Rejects filename traversal, symbolic links, hard links and non-regular files.
- Applies caller-provided size bounds; temporary files publish atomically without overwriting existing files.
- Contains no authorization or database logic. Catalog callers enforce ownership and reference locks.

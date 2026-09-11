# Kernels

Manage kernel files inside one configured directory.

- Directory scans discover manually installed files; SQLite aliases belong to the queries crate.
- Descriptor-relative operations reject traversal, symbolic links and hard links.
- Uploads and verified HTTPS downloads publish atomically without overwriting existing files. Maximum size is 128 MiB.
- Uses the shared catalog-files downloader with optional SHA-256 verification, public-address checks and at most five HTTPS redirects.
- Runtime locks and VM reference checks coordinate catalog mutations with VM definitions and preparation.

# Kernels

Manage kernel files inside one configured directory.

- Directory scans discover manually installed files; SQLite aliases belong to the queries crate.
- Descriptor-relative operations reject traversal, symbolic links and hard links.
- Uploads and verified HTTPS downloads publish atomically without overwriting existing files. Maximum size is 128 MiB.
- Downloads follow at most five HTTPS redirects, check public addresses and pin DNS at every hop, within one 300-second deadline.
- Runtime locks and VM reference checks coordinate catalog mutations with VM definitions and preparation.

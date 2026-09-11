Native OCI image pulling and root filesystem preparation.

- Uses `oci-spec` for OCI references and image documents, plus `tar`, `flate2`, and `zstd` for archive formats.
- HTTPS pulls accept tags or SHA-256 pins and select the host's Linux architecture. Tags resolve once per import; manifest headers, explicit pins, blobs, and uncompressed layer digests are verified.
- Basic and bearer credentials stay on the registry origin. An additional token endpoint must be explicitly trusted; Docker Hub's standard endpoint is recognized.
- Downloads and extraction have byte, entry, path, and time limits. Whiteouts apply before layer entries; filesystem operations remain confined to a private staging directory.
- Imports preserve UID/GID, mode, mtime, user xattrs and file capabilities. Special nodes, sparse files, ACLs and unsupported xattrs fail explicitly.
- `Image` owns cleanup. Call `has_file` for guest path inspection and `finalize` before constructing the disk image.

Images without an entrypoint or command require an explicit workload argv override.
The importer retains image defaults; the current override is supplied through the boot seed.

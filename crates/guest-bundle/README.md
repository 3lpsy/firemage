Embeds the static Linux x86_64 `firemage-guest` executable in the host binary.

- `bytes()` exposes the validated embedded payload without writing it to disk.
- `validate()` shares ELF checks with runtime overrides; the build rejects ELF files with an interpreter or shared-library dependencies.
- `just build` and the Cargo wrapper compile the guest in a separate target directory first.
- For direct Cargo use, run `just guest-build`, then export its printed path as `FIREMAGE_GUEST_BIN_PATH`.
- The server's `firemage_guest_bin_path` configuration overrides injection at runtime; it does not change this bundle.

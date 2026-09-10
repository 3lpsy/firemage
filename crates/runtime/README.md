VM lifecycle, assets, seed inputs, and managed guest networking.

- Persists VM definitions and observed state in SQLite; serializes operations per VM.
- Owns Firecracker processes and verifies persisted process identity during recovery.
- Opens egress listeners before boot, restores registrations after daemon restart, and closes connections on stop.
- Resolves owner-scoped secrets for boot inputs, outbound proxy requests and OCI pulls; registry dependencies are checked on define/edit/prepare and stored VM documents contain references only.

- Defaults to jailed Firecracker with per-VM identities, cgroup v2 limits and confined assets; trusted and external modes require host policy.
- Verifies jail identity during recovery and uses an isolated helper to extract guest output. Typed snapshots remain in the VM's managed directory in every mode.
- Kernel catalog references use stable filenames; aliases are display labels. Catalog changes and VM references share a lifecycle lock.

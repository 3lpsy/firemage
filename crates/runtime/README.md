VM lifecycle, assets, seed inputs, and managed guest networking.

- Persists VM definitions and observed state in SQLite; serializes operations per VM.
- Owns Firecracker processes and verifies persisted process identity during recovery.
- Opens egress listeners before boot, restores registrations after daemon restart, and closes connections on stop.
- Resolves owner-scoped secrets only for boot inputs or outbound proxy requests.

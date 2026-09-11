Owner-scoped SeaORM queries and database transactions.

- Stores users, credentials, VMs, networks, secrets, kernels, files, snapshots and reusable egress catalogs.
- Policy and proxy edits use optimistic revisions; owner checks reject cross-owner dependencies.
- VM creation, edits and live policy assignment update catalog bindings in the same transaction as the VM specification.
- Foreign keys protect referenced catalogs in every VM state; VM deletion removes its binding.

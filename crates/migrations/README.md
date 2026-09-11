Ordered SeaORM schema migrations for existing and new databases.

- Tables: users, credentials, VMs, networks, activity, secrets, kernel aliases, files, snapshots, upstream proxies, egress policies and VM policy bindings.
- Each new table has its own migration; shipped migrations remain unchanged.
- Egress aliases are unique per owner; composite foreign keys enforce matching owners and protect referenced resources.
- Legacy inline egress specifications migrate through the runtime before egress recovery.
- Network VM references migrate from owned names to stable UUIDs; network names stay globally unique.

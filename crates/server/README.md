Axum HTTP and Unix-socket API with embedded browser UI serving.

- Authenticates bearer credentials and browser sessions; browser mutations require same-origin CSRF protection.
- Routes VM lifecycle, catalogs, snapshots, guest files, serial output and terminal input through the runtime.
- `/v1/egress/policies` and `/v1/egress/proxies` provide catalog CRUD, revisions and associated VMs; mutations require an administrator.
- `/v1/vms/{id}/egress-policy` changes policy assignments, including running VMs; references must belong to the VM owner.
- Migrates legacy egress catalogs before recovering active networking at startup.

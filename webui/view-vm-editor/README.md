# view-vm-editor

Shared full-page Create and Edit VM form with collapsible sections and a section index.

- Hosted at `#vms/new` and `#vms/{id}/edit`; one submit saves the complete definition.
- Guided setup covers machine, storage, OCI workload, network, egress, environment, attachments, boot inputs, limits, metadata and terminal.
- Existing egress, environment, boot and security editors can apply changes to the draft without an API write.
- Kernel search uses the catalog; file and secret attachments retain identities, destinations and permissions.
- Full TOML round trips preserve every section. Runtime socket and isolation are fixed when a VM is created.
- The server validates ownership, policy and prepared disk constraints before saving.

- Egress selects a shared policy. Creating one opens a full page while the VM draft remains mounted.

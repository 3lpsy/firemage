# view-vm-detail

VM sidebar and full page with configuration transfer, attachments, lifecycle controls, host security, egress, environment, boot inputs, guest serial, terminal access, Firecracker diagnostics, output files, metadata, and raw Firecracker access.

- `view` crate in the desktop web UI.
- Full pages use `#vms/{id}` and share tabs with the inventory sidebar.
- Start prepares automatically; manual Launch appears only for eligible VM definitions.

# view-vm-detail

VM sidebar and full page with configuration transfer, attachments, lifecycle controls, host security, egress, environment, boot inputs, guest serial, terminal access, Firecracker diagnostics, output files, metadata, and raw Firecracker access.

- `view` crate in the desktop web UI.
- Full pages use `#vms/{id}/{tab}` with browser history; inventory sidebar tabs stay local.
- Start prepares automatically; manual Launch appears only for eligible VM definitions.
- Web Shell opens a separate guest PTY; Serial keeps log output and raw TTY Stream independent.

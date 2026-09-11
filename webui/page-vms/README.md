# page-vms

VM inventory, selected-machine sidebar, and full detail, Create and Edit pages.

- Hosted by the authenticated shell at `#vms`, `#vms/{id}/{tab}`, `#vms/new` and `#vms/{id}/edit`.
- Create and Edit share one form; saving opens the VM detail page.
- Only administrators edit definitions, and only while defined, stopped or failed.
- Import resolves portable config references before creating a definition.
- Mutations use the server session and CSRF token.

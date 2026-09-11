# routes

Navigation names and administrator-only page classification.

- `routes` crate in the desktop web UI.
- Shared by the authenticated app shell and its resource pages.

- Full VM tabs use `#vms/{id}/{tab}`; bare VM URLs still open Overview. Drawer tabs remain local.
- VM creation/edit, snapshot inspectors, and egress catalog/policy editor paths use URL fragments.

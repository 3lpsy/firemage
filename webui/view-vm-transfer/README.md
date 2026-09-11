Portable VM configuration export and import.

- `view` crate shared by the VM inventory and detail views.
- Exports catalog aliases and secret names; stored secret values are never fetched.
- Validates resource references before creation or updating a stopped VM.
- Imports allocate a new guest address; edits retain the existing address.

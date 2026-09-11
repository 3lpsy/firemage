Snapshot catalog page at `#snapshots`, with shared VM capture and restore controls.

- Alias links open `#snapshots/{id}`; row and chevron controls toggle the detail drawer, downloads and deletion.
- Uploads browser File/Blob bodies without copying whole bundles into WASM memory.
- Capture selects paused jailed VMs; restore selects stopped compatible VMs and requires trusted snapshots. The backend checks complete compatibility.
- Restore uses a searchable VM picker with names first, state beneath, and IDs only to distinguish duplicate names.
- `VmSnapshots` lists snapshots by source VM ID beneath inline capture/restore actions in the sidebar and full page. Uploaded bundles remain unbound; bundle contents may contain guest credentials.

Snapshot catalog page at `#snapshots`, with shared VM capture and restore controls.

- Lists aliases and informational source names with a detail drawer, downloads and deletion.
- Uploads browser File/Blob bodies without copying whole bundles into WASM memory.
- Capture selects paused jailed VMs; restore selects stopped compatible VMs and requires trusted snapshots. The backend checks complete compatibility.
- Exports `VmSnapshots` for the VM sidebar and full page. Bundle contents may contain guest credentials.

# FEAT-001 (Web terminal / Web shell)

Implemented 2026-09-11. Optional Web Terminal is configured in create/edit VM
and opens a separate Web Shell tab in the inspector and full page. TTY Stream
remains the raw ttyS0 console.

The embedded static firemage-guest runs only for enabled VMs, through the existing
OCI seed setup hook. Custom images provide compatible initialization. Browser
sessions use CSRF protection, short-lived single-use tickets, bounded vsock/PTY
messages, resize, reconnect, and process cleanup. One-shot workload semantics
remain independent. Builds also publish the helper as a separate release asset.

Tests cover configuration transfer, authorization, PTY interaction and cleanup,
browser lifecycle, and real OCI startup, recovery, snapshots, and one-shot exit.
See [VM documentation](../../vms.md) for configuration and proxy requirements.

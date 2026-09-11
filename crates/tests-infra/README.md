Embedded acceptance tests for disposable Linux infrastructure hosts.

- Linked into `firemage` only with the opt-in `tests-infra` Cargo feature.
- `firemage tests-infra --confirm` runs HTTPS CLI, raw Firecracker boot, and managed VM suites; `--suite` selects one.
- Confirmation happens before filesystem or host changes. Without a terminal, `--confirm` is required.
- Tests execute the current binary without a checkout. Python 3 and runtime tools are required; VM suites use passwordless sudo and `--fixtures-dir`.
- `--results-dir` retains suite stdout/stderr, daemon/guest logs, and `infra-results.json`, including failures. Temporary state and test-owned host resources are cleaned up.
- Managed guests exercise secret files, OCI one-shot/keep-alive commands, userdata ordering, ttyS0 input, separate logs and stopped-disk browsing. Web Shell checks browser authentication, a real vsock PTY, input/resize, session cleanup and main workload exit.
- Network cases cover HTTP/TLS policies, upstream credentials, binary tunnels and daemon recovery. Snapshot coverage includes bundle upload/download, trust checks, source deletion and cross-VM restore with retained egress policy.

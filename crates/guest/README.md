# firemage-guest

Optional Linux guest service for Firemage Web Shell sessions.

- Listens on vsock port 1024 and accepts only host CID 2, with at most eight sessions.
- Runs each shell in a separate pseudoterminal with resize support and disconnect cleanup.
- Defaults to `/bin/sh -i`; the host supplies optional command arguments.
- Runs under guest init, which must mount `/dev/pts`; it does not replace PID 1.
- Supports `--port`, `--version`, and `--help`; release builds are statically linked.

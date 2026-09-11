#!/usr/bin/env bash
set -euo pipefail

if ! command -v sccache >/dev/null; then
    echo 'sccache is missing; rebuild the shared toolchain image before running release workflows.' >&2
    exit 1
fi

# Host-network containers must never fall back to the shared default TCP port.
case "${SCCACHE_SERVER_UDS:-}" in
    /*) ;;
    *) echo 'Set SCCACHE_SERVER_UDS to an absolute, private socket path.' >&2; exit 2 ;;
esac

case "${1:-}" in
    start)
        : "${SCCACHE_DIR:?missing compiler cache directory}"
        mkdir -p "$SCCACHE_DIR"
        sccache --version
        sccache --start-server
        sccache --zero-stats
        ;;
    finish)
        # Preserve reporting errors while still flushing and stopping the daemon.
        trap 'sccache --stop-server >/dev/null' EXIT
        sccache --show-stats
        ;;
    *) echo 'Usage: sccache.sh start|finish' >&2; exit 2 ;;
esac

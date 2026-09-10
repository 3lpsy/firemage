#!/usr/bin/env bash
set -euo pipefail
proxy_index="${FIREMAGE_DEPENDENCY_INDEX:-${CARGO_REGISTRIES_CHILLED_PROXY_INDEX:-}}"
if [[ "$proxy_index" != sparse+https://*/ ]]; then
    echo 'Set FIREMAGE_DEPENDENCY_INDEX to the HTTPS sparse index of your dependency proxy.' >&2
    exit 1
fi
exec cargo --config 'source.crates-io.replace-with="firemage-proxy"' \
    --config "source.firemage-proxy.registry=\"$proxy_index\"" "$@"

#!/usr/bin/env bash
set -euo pipefail
proxy_index="${FIREMAGE_DEPENDENCY_INDEX:-${CARGO_REGISTRIES_CHILLED_PROXY_INDEX:-}}"
if [[ "$proxy_index" != sparse+https://*/ ]]; then
    echo 'Set FIREMAGE_DEPENDENCY_INDEX to the HTTPS sparse index of your dependency proxy.' >&2
    exit 1
fi
# Tool installation and WASM builds do not consume the native guest bundle.
if [[ "${FIREMAGE_BUILDING_GUEST:-}" != 1 && -f crates/guest/Cargo.toml ]]; then
    guest_needed=false
    case "${1:-}" in build|check|clippy|test|run|nextest|rustc|doc) guest_needed=true ;; esac
    packages=0
    guest_packages=0
    previous=''
    workspace=false
    for argument in "$@"; do
        case "$argument" in wasm32-*|--target=wasm32-*) guest_needed=false ;; --workspace) workspace=true ;; esac
        if [[ "$previous" == -p || "$previous" == --package ]]; then
            packages=$((packages + 1))
            case "$argument" in firemage-guest|firemage-guest-protocol) guest_packages=$((guest_packages + 1)) ;; esac
        fi
        previous="$argument"
    done
    if [[ "$workspace" == false ]] && (( packages > 0 && packages == guest_packages )); then guest_needed=false; fi
    if [[ "$guest_needed" == true && -z "${FIREMAGE_GUEST_BIN_PATH:-}" ]]; then
        FIREMAGE_GUEST_BIN_PATH="$(bash ci/build/guest.sh)"
        export FIREMAGE_GUEST_BIN_PATH
    fi
fi
exec cargo --config 'source.crates-io.replace-with="firemage-proxy"' \
    --config "source.firemage-proxy.registry=\"$proxy_index\"" "$@"

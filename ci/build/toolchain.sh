#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
operation="${1:-build}"
case "$operation" in build|publish) ;; *) echo 'Expected build or publish.' >&2; exit 2 ;; esac
engine="${CONTAINER_ENGINE:-}"
if [[ -z "$engine" ]]; then
    if command -v docker >/dev/null 2>&1; then engine=docker; else engine=podman; fi
fi
base="${FIREMAGE_TOOLCHAIN_BASE_IMAGE:-${CI_BASE_IMAGE:-docker.io/library/fedora:44}}"
proxy_index="${FIREMAGE_DEPENDENCY_INDEX:-${CARGO_REGISTRIES_CHILLED_PROXY_INDEX:-}}"
[[ "$proxy_index" == sparse+https://*/ ]] || { echo 'Set FIREMAGE_DEPENDENCY_INDEX to your Cargo proxy.' >&2; exit 1; }
image="${FIREMAGE_TOOLCHAIN_IMAGE:-firemage-toolchain:local}"
if [[ "$operation" == publish ]]; then
    [[ "$image" == */*:* && "$image" != *@* && "$image" != *[[:space:]]* ]] \
        || { echo 'Publishing requires FIREMAGE_TOOLCHAIN_IMAGE with a registry, image and tag.' >&2; exit 1; }
fi
args=(build --file ci/docker/toolchain/Dockerfile
    --build-arg "BASE_IMAGE=$base" --build-arg "DEPENDENCY_INDEX=$proxy_index" --tag "$image")
if [[ "$(basename "$engine")" == podman ]]; then
    args+=(--layers)
    if [[ -n "${FIREMAGE_TOOLCHAIN_CACHE:-}" ]]; then
        args+=(--cache-from "$FIREMAGE_TOOLCHAIN_CACHE" --cache-to "$FIREMAGE_TOOLCHAIN_CACHE")
    fi
elif [[ -n "${FIREMAGE_TOOLCHAIN_CACHE:-}" ]]; then
    echo 'FIREMAGE_TOOLCHAIN_CACHE requires Podman.' >&2
    exit 1
fi
if [[ "$operation" == publish ]]; then args+=(--pull); fi
"$engine" "${args[@]}" .
[[ "$operation" == publish ]] || exit 0
push_args=(push)
if [[ "$(basename "$engine")" == podman ]]; then push_args+=(--compression-format gzip --force-compression); fi
"$engine" "${push_args[@]}" "$image"

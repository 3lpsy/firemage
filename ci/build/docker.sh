#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
engine="${CONTAINER_ENGINE:-}"
if [[ -z "$engine" ]]; then
    if command -v docker >/dev/null 2>&1; then engine=docker; else engine=podman; fi
fi
command -v "$engine" >/dev/null || { echo "Container engine not found: $engine" >&2; exit 1; }
proxy_index="${FIREMAGE_DEPENDENCY_INDEX:-${CARGO_REGISTRIES_CHILLED_PROXY_INDEX:-}}"
[[ "$proxy_index" == sparse+https://*/ ]] || { echo 'Set FIREMAGE_DEPENDENCY_INDEX to your dependency proxy sparse index.' >&2; exit 1; }
toolchain="${FIREMAGE_TOOLCHAIN_IMAGE:-firemage-toolchain:local}"

ensure_toolchain() {
    case "${FIREMAGE_TOOLCHAIN_SOURCE:-build}" in
        build) bash ci/build/toolchain.sh build ;;
        registry)
            [[ -n "${FIREMAGE_TOOLCHAIN_IMAGE:-}" ]] || { echo 'Set FIREMAGE_TOOLCHAIN_IMAGE.' >&2; exit 1; }
            "$engine" pull "$toolchain"
            ;;
        *) echo 'FIREMAGE_TOOLCHAIN_SOURCE must be build or registry.' >&2; exit 1 ;;
    esac
}

operation="${1:?missing operation}"
shift
case "$operation" in
    toolchain) bash ci/build/toolchain.sh build; exit ;;
    recipe) ;;
    *) echo "Unknown container operation: $operation" >&2; exit 1 ;;
esac
profile="${1:?missing profile}"
recipe="${2:?missing recipe}"
shift 2
[[ "$profile" == dev || "$profile" == release ]] || { echo 'Profile must be dev or release.' >&2; exit 1; }
case "$recipe" in cargo|build|release|build-dev|build-dev-release|ui-build|ui-check|test-webui|test-e2e|test-infra-command|check|test|test-crates|test-doc|test-cli|fmt|fmt-check|lint|verify|run) ;; *) echo "Unknown recipe: $recipe" >&2; exit 1 ;; esac
[[ -f .env ]] || { echo 'Missing .env; copy .env.example first.' >&2; exit 1; }
ensure_toolchain
image="$toolchain"
if [[ "$recipe" != fmt && "$recipe" != fmt-check ]]; then
    image="${FIREMAGE_DEPENDENCIES_IMAGE:-firemage-dependencies}:$profile"
    "$engine" build --file ci/docker/build/Dockerfile --build-arg "TOOLCHAIN=$toolchain" \
        --build-arg "PROFILE=$profile" --tag "$image" .
fi

mkdir -p target/podman dist
args=(run --rm --interactive --workdir /workspace
    --volume "$root:/workspace:z" --volume "$root/target/podman:/build-target:z"
    --env FIREMAGE_CARGO_TARGET_DIR=/build-target --env CARGO_TARGET_DIR=/build-target
    --env XDG_CONFIG_HOME=/workspace/target/podman/config)
if [[ -t 0 && -t 1 ]]; then args+=(--tty); fi
if [[ "$(basename "$engine")" == podman ]]; then args+=(--userns=keep-id); fi
args+=(--user "${FIREMAGE_CONTAINER_USER:-$(id -u):$(id -g)}")
# Forward environment overrides by name; .env itself is read by just inside.
while IFS= read -r name; do
    case "$name" in FIREMAGE_*|CARGO_REGISTRIES_CHILLED_PROXY_INDEX|RUST_LOG) args+=(--env "$name") ;; esac
done < <(compgen -e)
# Reapply the container paths after forwarding host settings.
args+=(--env FIREMAGE_CARGO_TARGET_DIR=/build-target --env CARGO_TARGET_DIR=/build-target)
if [[ -n "${FIREMAGE_DOCKER_NETWORK:-}" ]]; then args+=(--network "$FIREMAGE_DOCKER_NETWORK"); fi
if [[ "$profile" == dev ]]; then args+=(--env CARGO_INCREMENTAL=1); fi
exec "$engine" "${args[@]}" --entrypoint bash "$image" /workspace/ci/build/container-entry.sh "$recipe" "$@"

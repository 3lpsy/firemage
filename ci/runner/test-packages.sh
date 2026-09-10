#!/usr/bin/env bash
set -euo pipefail
packages="$(bash ci/runner/cargo-test-packages.sh "${1:?missing tree}")"
[[ -n "$packages" ]] || { echo 'No packages in requested tier.' >&2; exit 1; }
args=()
while IFS= read -r package; do args+=(-p "$package"); done <<< "$packages"
exec bash ci/build/cargo.sh nextest run "${args[@]}" --profile "${NEXTEST_PROFILE:-default}"

#!/usr/bin/env bash
# Browser journeys use the built UI and an isolated server, with no KVM dependency.
set -euo pipefail
export FIREMAGE_WEBUI_EMBED_DIR="$PWD/dist/webui"
bash ci/build/cargo.sh build -p firemage
export FIREMAGE_E2E_BINARY="${CARGO_TARGET_DIR:-$PWD/target}/debug/firemage"
results="${FIREMAGE_CI_RESULTS_DIR:-}"
if [[ -z "$results" ]]; then
    mkdir -p "$PWD/target/e2e-results"
    results="$(mktemp -d "$PWD/target/e2e-results/run.XXXXXX")"
fi
export FIREMAGE_E2E_SCREENSHOT_DIR="$results/screenshots"
mkdir -p "$FIREMAGE_E2E_SCREENSHOT_DIR"
exec bash ci/build/cargo.sh nextest run -p firemage-webui-e2e \
    --profile "${NEXTEST_PROFILE:-default}" --run-ignored ignored-only --test-threads 1 --no-fail-fast

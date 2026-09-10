#!/usr/bin/env bash
set -euo pipefail
requested="${1:-check-all}"
case "$requested" in
    check-all)
        gate="$(just --dump --dump-format json | jq -er '
            .recipes["check-all"] as $recipe
            | if ($recipe.body | length) != 0 or ($recipe.dependencies | length) == 0
                or any($recipe.dependencies[]; (.arguments | length) != 0 or .star != null)
              then error("check-all must contain only unparameterized dependencies")
              else $recipe.dependencies[].recipe end')"
        mapfile -t recipes <<< "$gate"
        ;;
    check) recipes=(fmt-check check test-ci test-docker test-fj) ;;
    test-crates|test-webui|test-e2e|test-doc|clippy|fmt-check|build|build-release|build-dev-release) recipes=("$requested") ;;
    *) echo "Unknown tier: $requested" >&2; exit 2 ;;
esac
export NEXTEST_PROFILE=ci
# Nextest stores reports under the workspace, independently of Cargo build caches.
junit="$PWD/target/nextest/ci/junit.xml"
collect_results() {
    status=$?
    if [[ -n "${FIREMAGE_CI_RESULTS_DIR:-}" && -f "$junit" ]]; then
        cp "$junit" "$FIREMAGE_CI_RESULTS_DIR/tests.xml" || true
        if [[ -n "${recipe:-}" ]]; then
            cp "$junit" "$FIREMAGE_CI_RESULTS_DIR/tests-$recipe.xml" || true
        fi
    fi
    return "$status"
}
trap collect_results EXIT
rm -f -- "$junit"
for recipe in "${recipes[@]}"; do
    rm -f -- "$junit"
    if [[ -n "${FIREMAGE_CI_RESULTS_DIR:-}" ]]; then
        node ci/report/ci-record.mjs "$recipe" just "$recipe"
    else
        just "$recipe"
    fi
    collect_results
done

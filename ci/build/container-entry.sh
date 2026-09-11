#!/usr/bin/env bash
set -euo pipefail
recipe="${1:?missing just recipe}"
shift
case "$recipe" in
    cargo|build|release|build-dev|build-dev-release|ui-build|ui-check|test-webui|test-e2e|test-infra-command|check|test|test-crates|test-doc|test-cli|fmt|fmt-check|lint|verify|run) ;;
    *) echo "Unsupported container recipe: $recipe" >&2; exit 1 ;;
esac
export FIREMAGE_CARGO_TARGET_DIR=/build-target
export CARGO_TARGET_DIR=/build-target

# Formatting only needs the source mount, not a copy of compiled dependencies.
if [[ "$recipe" != fmt && "$recipe" != fmt-check ]]; then
    mkdir -p "$CARGO_TARGET_DIR"
    (
        flock 9
        profile="$(cat /opt/firemage/profile)"
        marker="$CARGO_TARGET_DIR/.chef-$profile"
        if ! cmp -s /opt/firemage/cache-key "$marker"; then
            cp -a --no-preserve=ownership --no-clobber /opt/firemage/target/. "$CARGO_TARGET_DIR/"
            cp /opt/firemage/cache-key "$marker"
        fi
    ) 9> "$CARGO_TARGET_DIR/.chef.lock"
fi

if [[ "$recipe" == build || "$recipe" == release || "$recipe" == build-dev || "$recipe" == build-dev-release ]]; then
    just "$recipe" "$@"
    profile=debug
    [[ "$recipe" != release && "$recipe" != build-dev-release ]] || profile=release
    mkdir -p /workspace/dist
    temporary="$(mktemp /workspace/dist/.firemage.XXXXXX)"
    trap 'rm -f "$temporary"' EXIT
    install -m 0755 "$CARGO_TARGET_DIR/$profile/firemage" "$temporary"
    mv -f "$temporary" /workspace/dist/firemage
    install -m 0755 "$CARGO_TARGET_DIR/guest-build/${FIREMAGE_GUEST_TARGET:-x86_64-unknown-linux-gnu}/release/firemage-guest" \
        /workspace/dist/firemage-guest
else
    exec just "$recipe" "$@"
fi

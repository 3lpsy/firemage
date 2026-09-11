#!/usr/bin/env bash
# Build the guest before the host Cargo process acquires its workspace lock.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${FIREMAGE_GUEST_TARGET:-x86_64-unknown-linux-gnu}"
case "$target" in
    x86_64-unknown-linux-gnu|x86_64-unknown-linux-musl) ;;
    *) echo 'Guest target must be Linux x86_64 GNU or musl.' >&2; exit 2 ;;
esac
build_dir="${FIREMAGE_CARGO_TARGET_DIR:-${CARGO_TARGET_DIR:-$root/target}}/guest-build"
cd "$root"
if [[ "$target" == x86_64-unknown-linux-gnu ]] && [[ ! -f "$(cc -print-file-name=libc.a)" ]]; then
    echo 'Static guest builds require libc.a; install glibc-static or your platform static libc package.' >&2
    exit 1
fi
# An explicit target keeps crt-static out of host proc macros and build scripts.
CARGO_TARGET_DIR="$build_dir" FIREMAGE_BUILDING_GUEST=1 \
    CARGO_ENCODED_RUSTFLAGS=$'-C\x1ftarget-feature=+crt-static' RUSTFLAGS='-C target-feature=+crt-static' \
    bash ci/build/cargo.sh build --locked --release --target "$target" -p firemage-guest --bin firemage-guest >&2
python3 ci/build/validate-guest.py "$build_dir/$target/release/firemage-guest"
realpath "$build_dir/$target/release/firemage-guest"

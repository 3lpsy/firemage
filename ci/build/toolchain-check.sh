#!/usr/bin/env bash
set -euo pipefail
test -f "$(cc -print-file-name=libc.a)"
cargo --version
cargo fmt --version
cargo clippy --version
cargo chef --version
cargo nextest --version
sccache --version
just --version
python3 --version
node --version
jq --version
wasm-bindgen --version
chromedriver --version
command -v chromium-browser || command -v chromium
test -d "$(rustc --print target-libdir --target wasm32-unknown-unknown)"
cache_probe="$(mktemp "${CARGO_HOME:?missing Cargo home}/.write-check.XXXXXX")"
rm -- "$cache_probe"

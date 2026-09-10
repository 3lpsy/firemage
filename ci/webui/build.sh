#!/usr/bin/env bash
set -euo pipefail
want="$(awk '/^name = "wasm-bindgen"$/{found=1} found && /^version = /{gsub(/[",]/,"",$3);print $3;exit}' Cargo.lock)"
have="$(wasm-bindgen --version | awk '{print $2}')"
[[ -n "$want" && "$have" == "$want" ]] || { echo "wasm-bindgen CLI must match Cargo.lock ($want); found $have" >&2; exit 1; }
bash ci/build/cargo.sh build --locked -p firemage-webui-app --profile wasm-release --target wasm32-unknown-unknown
mkdir -p dist/webui/assets
wasm-bindgen --target web --out-dir dist/webui --out-name firemage \
    "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/wasm-release/firemage-webui-app.wasm"
cp webui/app/index.html dist/webui/index.html
cp -R webui/app/assets/. dist/webui/assets/
cp webui/app/bootstrap.js dist/webui/bootstrap.js

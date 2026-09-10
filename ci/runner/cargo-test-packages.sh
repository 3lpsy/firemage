#!/usr/bin/env bash
set -euo pipefail
tier="${1:?missing tree}"
case "$tier" in crates|webui) ;; *) echo 'Select crates or webui.' >&2; exit 2 ;; esac
bash ci/build/cargo.sh metadata --no-deps --format-version 1 | jq -r --arg tier "$tier" '
    .workspace_root as $root | .workspace_members as $members | .packages[]
    | select(.id as $id | $members | index($id))
    | select(.manifest_path | startswith($root + "/" + $tier + "/"))
    | select(.name != "firemage-webui-e2e") | .name'

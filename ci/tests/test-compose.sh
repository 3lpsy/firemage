#!/usr/bin/env bash
# Exercise rootless Compose startup, HTTP port forwarding, bind mounts, and teardown.
set -euo pipefail
export PODMAN_COMPOSE_PROVIDER=podman-compose
export FIREMAGE_CI_CONTAINER_IMAGE
FIREMAGE_CI_CONTAINER_IMAGE="$(cat /opt/firemage/fixtures/container-image)"
mkdir -p target
scratch="$(mktemp -d "$PWD/target/compose.XXXXXX")"
export FIREMAGE_CI_COMPOSE_DIR="$scratch"
export FIREMAGE_CI_COMPOSE_PORT
FIREMAGE_CI_COMPOSE_PORT="$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    print(sock.getsockname()[1])
PY
)"
project="firemage-ci-${FIREMAGE_CI_COMPOSE_PORT}"
compose=(podman compose -p "$project" -f ci/docker/testing/compose/compose.yml)
cleanup() {
    status=$?
    "${compose[@]}" logs || true
    if ! "${compose[@]}" down --volumes --remove-orphans; then status=1; fi
    rm -rf -- "$scratch"
    return "$status"
}
trap cleanup EXIT
printf 'compose-mounted-data\n' > "$scratch/input"
"${compose[@]}" version
"${compose[@]}" up --detach
python3 - <<'PY'
import os
import time
import urllib.request
url = f'http://127.0.0.1:{os.environ["FIREMAGE_CI_COMPOSE_PORT"]}'
for attempt in range(60):
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            assert response.read() == b'compose-ready\n'
        break
    except OSError:
        time.sleep(0.5)
else:
    raise RuntimeError('Compose HTTP service did not become ready')
PY
cmp "$scratch/input" "$scratch/output"
printf 'Compose service, HTTP forwarding, and bind mount passed\n'

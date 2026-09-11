#!/usr/bin/env bash
# Public snapshots omit deployment and remote-publishing checks.
set -euo pipefail
if [[ ! -d ci/internal/tests ]]; then
    echo 'Private CI checks are not included in this source snapshot.'
    exit 0
fi
python3 ci/internal/tests/test-ci.py
python3 ci/internal/tests/test-ci-report.py
python3 ci/internal/tests/test-release.py
just test-mirror
node --test ci/internal/tests/test-promotion.mjs

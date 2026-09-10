#!/usr/bin/env python3
"""Write Cargo source replacement before any crate is downloaded."""
import json
import os
from pathlib import Path
from urllib.parse import urlsplit

index = os.environ.get('DEPENDENCY_INDEX', '')
url = urlsplit(index.removeprefix('sparse+'))
if (not index.startswith('sparse+https://') or not index.endswith('/')
        or not url.hostname or url.username or url.password or url.query or url.fragment
        or any(char.isspace() or ord(char) < 32 for char in index)):
    raise SystemExit('DEPENDENCY_INDEX must be a credential-free HTTPS sparse index ending in /')
root = Path(os.environ['CARGO_HOME'])
root.mkdir(parents=True, exist_ok=True)
(root / 'config.toml').write_text(
    '[source.crates-io]\nreplace-with = "firemage-proxy"\n\n'
    '[source.firemage-proxy]\nregistry = ' + json.dumps(index) + '\n'
)

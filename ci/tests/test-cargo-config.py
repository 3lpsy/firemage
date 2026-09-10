#!/usr/bin/env python3
"""Validate the Cargo configuration written into the shared toolchain."""
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class CargoConfigTests(unittest.TestCase):
    def test_proxy_is_the_only_crates_io_source(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'target') as directory:
            index = 'sparse+https://deps.example.com/crates/index/'
            result = subprocess.run(['python3', str(ROOT / 'ci/build/configure-cargo.py')],
                                    env=dict(os.environ, CARGO_HOME=directory, DEPENDENCY_INDEX=index),
                                    capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            config = tomllib.loads((Path(directory) / 'config.toml').read_text())
            self.assertEqual(config['source']['crates-io']['replace-with'], 'firemage-proxy')
            self.assertEqual(config['source']['firemage-proxy']['registry'], index)

    def test_invalid_or_credential_bearing_proxy_never_writes_config(self):
        for index in ('', 'https://deps.example.com/', 'sparse+http://deps.example.com/',
                      'sparse+https://secret@deps.example.com/', 'sparse+https://deps.example.com',
                      'sparse+https://deps.example.com/\n'):
            with self.subTest(index=index), tempfile.TemporaryDirectory(dir=ROOT / 'target') as directory:
                result = subprocess.run(['python3', str(ROOT / 'ci/build/configure-cargo.py')],
                                        env=dict(os.environ, CARGO_HOME=directory, DEPENDENCY_INDEX=index),
                                        capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((Path(directory) / 'config.toml').exists())


if __name__ == '__main__':
    unittest.main()

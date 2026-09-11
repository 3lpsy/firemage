#!/usr/bin/env python3
"""Check compiler cache setup and reporting without a daemon or compilation."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'ci/build/sccache.sh'


class CompilerCacheTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.log = self.work / 'calls.jsonl'
        self.cache = self.work / 'persistent cache' / 'sccache'
        self.socket = self.work / 'private.sock'
        fake = self.work / 'sccache'
        fake.write_text('#!/usr/bin/env python3\n'
                        'import json, os, sys\n'
                        'with open(os.environ["CACHE_TEST_LOG"], "a") as log:\n'
                        ' log.write(json.dumps({"args": sys.argv[1:], '
                        '"socket": os.getenv("SCCACHE_SERVER_UDS")}) + "\\n")\n'
                        'if sys.argv[1] == os.getenv("CACHE_TEST_FAIL"):\n'
                        ' sys.exit(42)\n'
                        'print("cache statistics")\n')
        fake.chmod(0o755)
        self.env = dict(os.environ, PATH=f'{self.work}:{os.environ["PATH"]}',
                        SCCACHE_DIR=str(self.cache), SCCACHE_SERVER_UDS=str(self.socket),
                        CACHE_TEST_LOG=str(self.log))

    def invoke(self, command):
        return subprocess.run(['bash', str(SCRIPT), command], cwd=self.work,
                              env=self.env, capture_output=True, text=True)

    def calls(self):
        return [json.loads(line)['args'] for line in self.log.read_text().splitlines()]

    def test_start_and_finish_use_the_same_private_socket(self):
        for action in ('start', 'finish'):
            result = self.invoke(action)
            self.assertEqual(result.returncode, 0, result.stderr)
        for line in self.log.read_text().splitlines():
            self.assertEqual(json.loads(line)['socket'], str(self.socket))

    def test_missing_or_non_path_socket_never_uses_default_tcp_endpoint(self):
        for socket in ('', 'relative.sock', '@shared-abstract-socket'):
            for action in ('start', 'finish'):
                with self.subTest(socket=socket, action=action):
                    self.env['SCCACHE_SERVER_UDS'] = socket
                    result = self.invoke(action)
                    self.assertEqual(result.returncode, 2)
                    self.assertIn('private socket path', result.stderr)
                    self.assertFalse(self.log.exists())

    def test_start_creates_cache_without_removing_previous_entries(self):
        self.cache.mkdir(parents=True)
        entry = self.cache / 'previous-entry'
        entry.write_bytes(b'cached object')
        result = self.invoke('start')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(entry.read_bytes(), b'cached object')
        self.assertEqual(self.calls(), [['--version'], ['--start-server'], ['--zero-stats']])

    def test_start_creates_new_directory(self):
        result = self.invoke('start')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.cache.is_dir())

    def test_start_failure_is_not_silenced(self):
        self.env['CACHE_TEST_FAIL'] = '--start-server'
        result = self.invoke('start')
        self.assertEqual(result.returncode, 42)
        self.assertEqual(self.calls(), [['--version'], ['--start-server']])

    def test_reporting_stops_daemon_even_when_statistics_fail(self):
        for failure in ('', '--show-stats'):
            with self.subTest(failure=failure):
                self.log.unlink(missing_ok=True)
                self.env['CACHE_TEST_FAIL'] = failure
                result = self.invoke('finish')
                self.assertEqual(result.returncode, 42 if failure else 0, result.stderr)
                self.assertEqual(self.calls(), [['--show-stats'], ['--stop-server']])

    def test_invalid_action_does_not_start_daemon(self):
        result = self.invoke('invalid')
        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())

    def test_missing_cache_directory_fails_before_start(self):
        self.env.pop('SCCACHE_DIR')
        result = self.invoke('start')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('missing compiler cache directory', result.stderr)
        self.assertFalse(self.log.exists())


if __name__ == '__main__':
    unittest.main()

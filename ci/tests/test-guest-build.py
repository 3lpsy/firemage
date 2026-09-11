#!/usr/bin/env python3
"""Check static guest validation and Cargo prebuild dispatch without compiling."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('validate_guest', ROOT / 'ci/build/validate-guest.py')
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


def elf(kind=1):
    data = bytearray(160)
    data[:7] = b'\x7fELF\x02\x01\x01'
    struct.pack_into('<HH', data, 16, 2, 62)
    struct.pack_into('<Q', data, 32, 64)
    struct.pack_into('<HH', data, 54, 56, 1)
    struct.pack_into('<I', data, 64, kind)
    return data


class GuestTests(unittest.TestCase):
    def test_static_executable_validation(self):
        validator.validate(elf())
        for data in (b'', b'#!/bin/sh\n', elf(3), elf()[:100]):
            with self.subTest(data=data[:8]), self.assertRaises(ValueError):
                validator.validate(data)
        data = elf(2)
        struct.pack_into('<Q', data, 72, 128)
        struct.pack_into('<Q', data, 96, 16)
        struct.pack_into('<Q', data, 128, 1)
        with self.assertRaisesRegex(ValueError, 'shared libraries'):
            validator.validate(data)

    def test_staged_host_must_embed_the_exact_guest(self):
        with tempfile.TemporaryDirectory() as directory:
            guest = Path(directory) / 'guest'
            host = Path(directory) / 'host'
            guest.write_bytes(elf())
            for content, success in [(b'prefix' + elf() + b'suffix', True), (b'unrelated host', False)]:
                host.write_bytes(content)
                result = subprocess.run(['python3', str(ROOT / 'ci/build/validate-guest.py'), str(guest), str(host)],
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode == 0, success, result.stderr)

    @unittest.skipUnless(shutil.which('cargo-chef'), 'cargo-chef is not installed')
    def test_chef_stubs_embedding_sources_without_building(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            source = work / 'source'
            cooked = work / 'cooked'
            source.mkdir()
            cooked.mkdir()
            shutil.copytree(ROOT / 'crates/guest-bundle/src', source / 'src')
            shutil.copy(ROOT / 'crates/guest-bundle/build.rs', source / 'build.rs')
            (source / 'Cargo.toml').write_text('[package]\nname="guest-bundle-fixture"\nversion="0.1.0"\nedition="2024"\n')
            recipe = work / 'recipe.json'
            for cwd, args in [(source, ['prepare']), (cooked, ['cook', '--no-build'])]:
                result = subprocess.run(['cargo', 'chef', *args, '--recipe-path', str(recipe)],
                                        cwd=cwd, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((cooked / 'build.rs').read_text().strip(), 'fn main() {}')
            self.assertEqual((cooked / 'src/lib.rs').read_text().strip(), '')
            self.assertFalse((cooked / 'src/validate.rs').exists())

    def test_native_build_precedes_host_without_affecting_wasm_or_guest_tests(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / 'ci/build').mkdir(parents=True)
            (work / 'crates/guest').mkdir(parents=True)
            (work / 'crates/guest/Cargo.toml').touch()
            shutil.copy(ROOT / 'ci/build/cargo.sh', work / 'ci/build/cargo.sh')
            (work / 'ci/build/guest.sh').write_text('echo "guest" >> "$CALL_LOG"\necho /static/guest\n')
            cargo = work / 'cargo'
            cargo.write_text('#!/usr/bin/env python3\nimport json, os, sys\n'
                             'with open(os.environ["CALL_LOG"], "a") as f:\n'
                             ' f.write(json.dumps({"args": sys.argv[1:], "guest": os.getenv("FIREMAGE_GUEST_BIN_PATH")}) + "\\n")\n')
            cargo.chmod(0o755)
            env = dict(os.environ, PATH=f'{work}:{os.environ["PATH"]}',
                       FIREMAGE_DEPENDENCY_INDEX='sparse+https://deps.example.com/index/',
                       CALL_LOG=str(work / 'calls'))
            env.pop('FIREMAGE_GUEST_BIN_PATH', None)
            env.pop('FIREMAGE_BUILDING_GUEST', None)
            cases = [(['check', '--workspace'], True),
                     (['nextest', 'run', '-p', 'firemage-guest', '-p', 'firemage-runtime'], True),
                     (['test', '-p', 'firemage-guest'], False),
                     (['check', '--workspace', '-p', 'firemage-guest'], True),
                     (['check', '--target', 'wasm32-unknown-unknown'], False),
                     (['install', 'cargo-nextest'], False)]
            for args, expected in cases:
                with self.subTest(args=args):
                    (work / 'calls').write_text('')
                    result = subprocess.run(['bash', 'ci/build/cargo.sh', *args], cwd=work,
                                            env=env, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    calls = (work / 'calls').read_text().splitlines()
                    self.assertEqual(calls[0] == 'guest', expected)
                    self.assertEqual(json.loads(calls[-1])['guest'], '/static/guest' if expected else None)


if __name__ == '__main__':
    unittest.main()

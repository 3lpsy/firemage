#!/usr/bin/env python3
"""Check container command construction without a container daemon or network."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FAKE_ENGINE = r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

args = sys.argv[1:]
with open(os.environ["ENGINE_LOG"], "a") as output:
    output.write(json.dumps({"args": args, "token": os.environ.get("FIREMAGE_AUTHTOKEN")}) + "\n")
failure = os.environ.get("ENGINE_FAIL", "")
if failure and (failure == args[0] or failure in args):
    sys.exit(42)
'''


def values(args, flag):
    return [args[index + 1] for index, value in enumerate(args[:-1]) if value == flag]


class DockerCommands(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="docker-tests-", dir=ROOT / "target")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "project with spaces"
        scripts = self.root / "ci/build"
        scripts.mkdir(parents=True)
        shutil.copy2(ROOT / "justfile", self.root)
        shutil.copy2(ROOT / "ci/build/docker.sh", scripts)
        shutil.copy2(ROOT / "ci/build/toolchain.sh", scripts)
        (self.root / ".env").write_text(
            "FIREMAGE_TOOLCHAIN_BASE_IMAGE=example.invalid/rust:stable\n"
            "FIREMAGE_DEPENDENCY_INDEX=sparse+https://example.invalid/crates/index/\n"
        )
        self.engine = Path(self.temporary.name) / "docker"
        self.engine.write_text(FAKE_ENGINE)
        self.engine.chmod(0o755)
        self.log = Path(self.temporary.name) / "engine.jsonl"
        self.env = {
            key: value for key, value in os.environ.items()
            if not key.startswith("FIREMAGE_") and key not in {
                "CONTAINER_ENGINE", "CI_BASE_IMAGE", "CARGO_REGISTRIES_CHILLED_PROXY_INDEX",
            }
        }
        self.env.update(CONTAINER_ENGINE=str(self.engine), ENGINE_LOG=str(self.log))

    def invoke(self, recipe, *args, success=True):
        self.log.unlink(missing_ok=True)
        result = subprocess.run(
            ["just", "--justfile", str(self.root / "justfile"), recipe, *args],
            cwd=self.temporary.name, env=self.env, capture_output=True, text=True,
        )
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stderr)
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_recipes_select_profile_and_dispatch(self):
        for recipe in ("build", "release", "check", "test", "test-cli", "test-crates", "test-doc", "fmt", "fmt-check", "lint", "verify", "run"):
            with self.subTest(recipe=recipe):
                calls = self.invoke("docker-" + recipe)
                self.assertEqual([call["args"][0] for call in calls],
                                 ["build", "run"] if recipe in ("fmt", "fmt-check") else ["build", "build", "run"])
                run = calls[-1]["args"]
                self.assertEqual(run[-2:], ["/workspace/ci/build/container-entry.sh", recipe])
                if recipe not in ("fmt", "fmt-check"):
                    profile = "release" if recipe == "release" else "dev"
                    self.assertIn("PROFILE=" + profile, values(calls[1]["args"], "--build-arg"))
                    self.assertTrue(run[-3].endswith(":" + profile))

    def test_toolchain_gets_proxy_from_dotenv(self):
        calls = self.invoke("docker-toolchain")
        self.assertEqual(len(calls), 1)
        args = calls[0]["args"]
        self.assertIn("BASE_IMAGE=example.invalid/rust:stable", values(args, "--build-arg"))
        self.assertIn("DEPENDENCY_INDEX=sparse+https://example.invalid/crates/index/", values(args, "--build-arg"))

    def test_published_toolchain_is_pulled_without_rebuilding(self):
        self.env.update(FIREMAGE_TOOLCHAIN_SOURCE="registry",
                        FIREMAGE_TOOLCHAIN_IMAGE="example.invalid/firemage-toolchain:stable")
        calls = self.invoke("docker-build")
        self.assertEqual([call["args"][0] for call in calls], ["pull", "build", "run"])
        self.assertEqual(calls[0]["args"][1], "example.invalid/firemage-toolchain:stable")
        self.assertIn("TOOLCHAIN=example.invalid/firemage-toolchain:stable",
                      values(calls[1]["args"], "--build-arg"))

    def test_publish_uses_registry_cache(self):
        podman = self.engine.with_name("podman")
        self.engine.rename(podman)
        self.env.update(CONTAINER_ENGINE=str(podman),
                        FIREMAGE_TOOLCHAIN_IMAGE="example.invalid/firemage-toolchain:latest",
                        FIREMAGE_TOOLCHAIN_CACHE="example.invalid/cache/toolchain")
        calls = self.invoke("ci-toolchain")
        self.assertEqual([call["args"][0] for call in calls], ["build", "push"])
        build = calls[0]["args"]
        self.assertIn("--pull", build)
        self.assertIn("--layers", build)
        self.assertEqual(values(build, "--cache-from"), ["example.invalid/cache/toolchain"])
        self.assertEqual(values(build, "--cache-to"), ["example.invalid/cache/toolchain"])
        self.assertIn("gzip", calls[1]["args"])

    def test_failed_build_never_pushes(self):
        self.env["FIREMAGE_TOOLCHAIN_IMAGE"] = "example.invalid/firemage-toolchain:latest"
        self.env["ENGINE_FAIL"] = "build"
        calls = self.invoke("ci-toolchain", success=False)
        self.assertEqual([call["args"][0] for call in calls], ["build"])

    def test_incremental_mounts_user_and_environment(self):
        self.env.update(FIREMAGE_CONTAINER_USER="1001:1002", FIREMAGE_AUTHTOKEN="secret value",
                        FIREMAGE_CARGO_TARGET_DIR="/host-only", FIREMAGE_DOCKER_NETWORK="host")
        call = self.invoke("docker-build")[-1]
        args = call["args"]
        self.assertEqual(values(args, "--volume"),
                         [f"{self.root}:/workspace:z", f"{self.root}/target/podman:/build-target:z"])
        self.assertEqual(values(args, "--user"), ["1001:1002"])
        self.assertEqual(values(args, "--network"), ["host"])
        forwarded = values(args, "--env")
        self.assertIn("FIREMAGE_AUTHTOKEN", forwarded)
        self.assertNotIn("secret value", " ".join(args))
        self.assertEqual(call["token"], "secret value")
        self.assertIn("CARGO_INCREMENTAL=1", forwarded)
        paths = [value for value in forwarded if value.startswith("FIREMAGE_CARGO_TARGET_DIR")]
        self.assertEqual(paths[-1], "FIREMAGE_CARGO_TARGET_DIR=/build-target")

    def test_podman_preserves_user_namespace(self):
        podman = self.engine.with_name("podman")
        self.engine.rename(podman)
        self.env["CONTAINER_ENGINE"] = str(podman)
        args = self.invoke("docker-build")[-1]["args"]
        self.assertIn("--userns=keep-id", args)
        self.assertEqual(values(args, "--user"), [f"{os.getuid()}:{os.getgid()}"])

    def test_run_preserves_literal_arguments(self):
        marker = Path(self.temporary.name) / "should-not-exist"
        arguments = ["--config", "file with spaces.toml", f"$(touch '{marker}')",
                     f"`touch '{marker}'`", "; echo unwanted", 'a"b\'c', "", "*.toml"]
        args = self.invoke("docker-run", *arguments)[-1]["args"]
        self.assertEqual(args[-len(arguments):], arguments)
        self.assertFalse(marker.exists())

    def test_engine_failure_stops_before_run_or_export(self):
        output = self.root / "dist/firemage"
        output.parent.mkdir()
        output.write_text("previous binary")
        for failure, count in (("ci/docker/toolchain/Dockerfile", 1),
                               ("ci/docker/build/Dockerfile", 2), ("run", 3)):
            with self.subTest(failure=failure):
                self.env["ENGINE_FAIL"] = failure
                calls = self.invoke("docker-build", success=False)
                self.assertEqual(len(calls), count)
                self.assertEqual(output.read_text(), "previous binary")
                self.assertEqual(list(output.parent.iterdir()), [output])


if __name__ == "__main__":
    unittest.main()

"""HTTPS daemon and VM lifecycle helpers for privileged CI acceptance tests."""
import base64
import contextlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import ssl
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

from .fixture import prepare

ROOT = Path(__file__).resolve().parents[4]


def wait_for(description, predicate, timeout=90):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.2)
    raise AssertionError(f"Timed out waiting for {description}")


class Harness:
    def __init__(self):
        self.binary = Path(os.environ.get("FIREMAGE_TEST_BINARY", ROOT / "dist/firemage")).resolve()
        self.results = Path(os.environ.get("FIREMAGE_CI_RESULTS_DIR", ROOT / "ci-results")).resolve()
        self.fixtures = Path(os.environ.get("FIREMAGE_TEST_FIXTURES", "/opt/firemage/fixtures"))
        self.temporary = tempfile.TemporaryDirectory(prefix="fm-vm-", dir="/tmp")
        self.directory = Path(self.temporary.name)
        self.data = self.directory / "data"
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("FIREMAGE_")}
        self.env["XDG_CONFIG_HOME"] = str(self.directory / "config")
        self.server = None
        self.token = None
        self.vms = []
        self.networks = []
        self.log = None
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            self.port = reservation.getsockname()[1]
        self.base = f"https://127.0.0.1:{self.port}"

    def __enter__(self):
        try:
            assert os.geteuid() == 0, "VM acceptance tests require root on an ephemeral CI VM"
            assert self.binary.is_file() and os.access(self.binary, os.X_OK), self.binary
            for fixture in ("kernel", "rootfs.ext4"):
                assert (self.fixtures / fixture).is_file(), f"Missing VM fixture {fixture}"
            self.rootfs = self.directory / "rootfs.ext4"
            prepare(self.fixtures / "rootfs.ext4", self.rootfs)
            self.results.mkdir(parents=True, exist_ok=True)
            cert, key = self.directory / "cert.pem", self.directory / "key.pem"
            subprocess.run([
                "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                "-subj", "/CN=localhost", "-addext", "subjectAltName=IP:127.0.0.1",
                "-addext", "basicConstraints=critical,CA:FALSE", "-addext",
                "keyUsage=critical,digitalSignature,keyEncipherment", "-addext",
                "extendedKeyUsage=serverAuth", "-keyout", str(key), "-out", str(cert),
            ], check=True, capture_output=True, timeout=30)
            self.context = ssl.create_default_context(cafile=cert)
            self.opener = urllib.request.build_opener(
                urllib.request.ProxyHandler({}), urllib.request.HTTPSHandler(context=self.context))
            spec = importlib.util.spec_from_file_location("cli_acceptance", ROOT / "crates/tests-infra/scripts/test-cli.py")
            cli = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(cli)
            cli.BINARY = self.binary
            cli.prompted(["user", "bootstrap", "operator", "--data-dir", str(self.data)], self.env, 2)
            self.log = (self.results / "vm-server.log").open("wb")
            os.fchmod(self.log.fileno(), 0o644)
            self.start_server()
            self.token = self.request("POST", "/v1/auth/login", {
                "username": "operator", "password": cli.PASSWORD.decode().strip(),
            })["token"]
            return self
        except BaseException:
            self.__exit__(None, None, None)
            raise

    def start_server(self):
        self.server = subprocess.Popen([
            self.binary, "serve", "--listen", f"127.0.0.1:{self.port}", "--data-dir", self.data,
            "--tls-cert", self.directory / "cert.pem", "--tls-key", self.directory / "key.pem",
        ], env=self.env, stdout=self.log, stderr=self.log)

        def ready():
            assert self.server.poll() is None, "Firemage daemon exited; inspect vm-server.log"
            try:
                return self.request("GET", "/health")["status"] == "ok"
            except urllib.error.URLError:
                return False
        wait_for("HTTPS readiness", ready, 30)

    def stop_server(self):
        if self.server is not None:
            self.server.terminate()
            try:
                self.server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.server.kill()
                self.server.wait(timeout=10)
            self.server = None

    def request(self, method, path, body=None):
        headers = {"Content-Type": "application/json"}
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        request = urllib.request.Request(self.base + path, method=method, headers=headers,
                                         data=None if body is None else json.dumps(body).encode())
        try:
            with self.opener.open(request, timeout=30) as response:
                data = response.read()
                return json.loads(data) if data else None
        except urllib.error.HTTPError as error:
            raise AssertionError(f"{method} {path}: HTTP {error.code}: {error.read().decode()}") from error

    def define(self, name, script, **extra):
        spec = {
            "name": name, "kernel": {"kind": "local", "path": str(self.fixtures / "kernel")},
            "rootfs": {"kind": "local", "path": str(self.rootfs)},
            "boot_args": "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw init=/init",
            "memory_mib": 128, "files": [{"path": "run.sh", "content": script}],
        }
        spec.update(extra)
        vm = self.request("POST", "/v1/vms", spec)
        self.vms.append(vm["id"])
        return vm["id"]

    def action(self, vm, action):
        return self.request("POST", f"/v1/vms/{vm}/actions", {"action": action})

    def state(self, vm, state, timeout=90):
        def reached():
            current = self.request("GET", f"/v1/vms/{vm}")
            assert current["state"] != "failed", current
            return current if current["state"] == state else None
        return wait_for(f"{vm} state {state}", reached, timeout)

    def output(self, vm, name):
        value = self.request("GET", f"/v1/vms/{vm}/files?path=firemage/output/{name}")
        return base64.b64decode(value["base64"], validate=True)

    def console(self, vm):
        path = self.data / "vms" / vm / "console.log"
        return path.read_text(errors="replace") if path.exists() else ""

    def __exit__(self, *_):
        # Preserve evidence before API deletion removes each guest directory.
        if self.results.exists():
            for vm in self.vms:
                source = self.data / "vms" / vm / "console.log"
                if source.exists():
                    destination = self.results / f"vm-{vm}.log"
                    shutil.copyfile(source, destination)
                    destination.chmod(0o644)
        if self.server is not None:
            for vm in self.vms:
                with contextlib.suppress(Exception):
                    self.action(vm, "stop")
                    self.request("DELETE", f"/v1/vms/{vm}")
            for network in self.networks:
                with contextlib.suppress(Exception):
                    self.request("DELETE", f"/v1/networks/{network}")
        self.stop_server()
        # A crashed daemon may leave managed Firecracker children behind.
        for process in Path("/proc").glob("[0-9]*/cmdline"):
            with contextlib.suppress(OSError, ValueError):
                args = process.read_bytes().split(b"\0")
                if b"--api-sock" in args:
                    path = args[args.index(b"--api-sock") + 1].decode()
                    if path.startswith(str(self.data / "sockets") + "/"):
                        os.kill(int(process.parent.name), signal.SIGKILL)
        for vm in self.vms:
            interface = "fm" + vm.replace("-", "")[:10]
            for command in (["nft", "delete", "table", "netdev", interface],
                            ["ip", "link", "delete", interface]):
                subprocess.run(command, capture_output=True, timeout=10, check=False)
        if self.log:
            self.log.close()
        self.temporary.cleanup()

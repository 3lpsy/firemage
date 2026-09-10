#!/usr/bin/env python3
"""Exercise the real binary over TLS, including terminal-only login prompts."""
import contextlib
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import shutil
import socket
import ssl
import subprocess
import tempfile
import termios
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
BUILD_TARGET = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
BINARY = Path(os.environ.get("FIREMAGE_TEST_BINARY", BUILD_TARGET / "debug/firemage")).resolve()
PASSWORD = b"integration-password-123\n"


def prompted(args, env, prompts):
    master, slave = pty.openpty()
    reader, writer = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.dup2(slave, 0)
        os.dup2(writer, 1)
        os.dup2(slave, 2)
        for fd in (master, slave, reader, writer):
            if fd > 2:
                os.close(fd)
        os.execve(BINARY, [str(BINARY), *args], env)
    os.close(slave)
    os.close(writer)
    terminal = b""
    stdout = b""
    reaped = False
    sent = 0
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master, reader], [], [], 0.1)
            for fd in ready:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    data = b""
                if fd == reader:
                    stdout += data
                else:
                    terminal += data
                    if sent < prompts and b"assword: " in terminal:
                        time.sleep(0.05)
                        os.write(master, PASSWORD)
                        sent += 1
                        terminal = b""
            finished, status = os.waitpid(pid, os.WNOHANG)
            if finished:
                reaped = True
                stdout += os.read(reader, 65536)
                assert os.waitstatus_to_exitcode(status) == 0, terminal.decode(errors="replace")
                assert sent == prompts
                return stdout
        raise AssertionError("interactive command timed out")
    finally:
        if not reaped:
            with contextlib.suppress(ProcessLookupError):
                os.kill(pid, signal.SIGKILL)
            with contextlib.suppress(ChildProcessError):
                os.waitpid(pid, 0)
        os.close(master)
        os.close(reader)


def interrupted(number, _frame):
    raise SystemExit(128 + number)


def main():
    signal.signal(signal.SIGTERM, interrupted)
    if not BINARY.is_file() or not os.access(BINARY, os.X_OK):
        raise RuntimeError(f"Missing executable test binary: {BINARY}")
    # Unix socket paths must stay below 108 bytes even in deeply nested CI checkouts.
    with tempfile.TemporaryDirectory(prefix="fm-cli-", dir="/tmp") as temporary:
        directory = Path(temporary)
        env = {k: v for k, v in os.environ.items() if not k.startswith("FIREMAGE_")}
        env["XDG_CONFIG_HOME"] = str(directory / "config")
        cert, key = directory / "cert.pem", directory / "key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1", "-addext", "basicConstraints=critical,CA:FALSE", "-addext", "keyUsage=critical,digitalSignature,keyEncipherment", "-addext", "extendedKeyUsage=serverAuth", "-keyout", str(key), "-out", str(cert)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        prompted(["user", "bootstrap", "operator", "--data-dir", str(directory / "server")], env, 2)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        base = f"https://127.0.0.1:{port}"
        with (directory / "server.log").open("wb") as log:
            server = subprocess.Popen([BINARY, "serve", "--listen", f"127.0.0.1:{port}", "--data-dir", directory / "server", "--tls-cert", cert, "--tls-key", key], env=env, stdout=log, stderr=log)
            try:
                context = ssl.create_default_context(cafile=cert)
                for _ in range(100):
                    try:
                        with urllib.request.urlopen(base + "/health", context=context, timeout=1) as response:
                            assert json.load(response)["status"] == "ok"
                            break
                    except OSError:
                        if server.poll() is not None:
                            raise AssertionError((directory / "server.log").read_text())
                        time.sleep(0.05)
                else:
                    raise AssertionError("TLS server did not become ready")
                flags = ["--url", base, "--ca-cert", str(cert), "--username", "operator"]
                token = prompted(["authtoken", *flags], env, 1)
                assert re.fullmatch(rb"fm_session_[0-9a-f]{64}\n", token), "authtoken stdout must contain only token"
                assert not (directory / "config").exists(), "authtoken wrote client files"
                unattended = subprocess.run([BINARY, "authtoken", *flags, "--password-stdin"], env=env, input=PASSWORD, capture_output=True, check=True)
                assert re.fullmatch(rb"fm_session_[0-9a-f]{64}\n", unattended.stdout)
                assert not (directory / "config").exists(), "stdin authtoken wrote client files"
                prompted(["login", *flags], env, 1)

                def command(*args, extra_env=None):
                    result = subprocess.run([BINARY, *args], env=env | (extra_env or {}), capture_output=True, text=True)
                    assert result.returncode == 0, f"{args}: {result.stderr}"
                    return json.loads(result.stdout)

                assert command("whoami")["username"] == "operator"
                api = command("apitoken", "create", "integration", "--expires-at", str(int(time.time()) + 3600))
                assert command("whoami", extra_env={"FIREMAGE_APITOKEN": api["token"]})["admin"]
                tokens = command("apitoken", "list")
                command("apitoken", "delete", tokens[0]["id"])
                invalid = subprocess.run([BINARY, "whoami"], env=env | {"FIREMAGE_APITOKEN": api["token"]}, capture_output=True)
                assert invalid.returncode != 0, "deleted API token still worked"
                spec = directory / "vm.toml"
                spec.write_text('name = "offline"\n')
                vm = command("vm", "create", str(spec))
                assert vm["spec"]["network"] is None
                command("vm", "delete", vm["id"])
                assert command("vm", "list") == []
                assert len(list((directory / "config/firemage").glob("*.toml"))) == 2
                print("CLI HTTPS integration passed: bootstrap, disk-free authtoken, login persistence, API-token revocation, offline VM definition")
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
                results = os.environ.get("FIREMAGE_CI_RESULTS_DIR")
                if results:
                    shutil.copyfile(directory / "server.log", Path(results) / "cli-server.log")


if __name__ == "__main__":
    main()

"""Guest acceptance cases with real disk I/O, process recovery and TAP policy."""
import base64
import contextlib
import http.client
import http.server
import json
import os
import secrets
import socket
import subprocess
import threading
import urllib.request

from .harness import wait_for
from .guest_files import verify as verify_guest_files


def offline(harness):
    payload = bytes(range(256)) + b"\x00runtime-input\xff"
    secret = "guest-" + secrets.token_hex(16)
    harness.request("PUT", "/v1/secrets/guest-test", {"value": secret})
    userdata = """set -eu
[ "$PLAIN_VALUE" = "plain-value" ]
[ -n "$PRIVATE_VALUE" ]
[ "$(stat -c '%u:%g:%a' /home/guest/payload.bin)" = "1000:1001:640" ]
[ "$(ls /sys/class/net)" = lo ]
printf 'userdata-ready\\n' > /firemage/output/userdata-result
"""
    script = """set -eu
[ "$(ls /sys/class/net)" = lo ]
[ ! -e /sys/class/net/eth0 ]
[ ! -e /host ]
[ ! -e /workspace ]
[ "$(cat /firemage/output/userdata-result)" = userdata-ready ]
cp /home/guest/payload.bin /firemage/output/payload.bin
printf '%s' "$PRIVATE_VALUE" > /firemage/output/environment-value
printf 'offline-ok\\n' > /firemage/output/result
"""
    vm = harness.define("offline-roundtrip", script, userdata=userdata, environment={
        "PLAIN_VALUE": "plain-value", "PRIVATE_VALUE": {"secret": "guest-test"},
    }, files=[
        {"path": "run.sh", "content": script},
        {"path": "payload.bin", "content": base64.b64encode(payload).decode(), "encoding": "base64",
         "destination": "/home/guest/payload.bin", "uid": 1000, "gid": 1001, "mode": 0o640},
    ])
    returned = harness.request("GET", f"/v1/vms/{vm}")
    assert secret not in json.dumps(returned), "VM API exposed a resolved environment secret"
    assert harness.action(vm, "start")["state"] == "running"
    harness.state(vm, "stopped")
    assert harness.output(vm, "exit-code") == b"0\n"
    assert harness.output(vm, "result") == b"offline-ok\n"
    assert harness.output(vm, "payload.bin") == payload
    assert harness.output(vm, "environment-value") == secret.encode()
    assert harness.output(vm, "userdata-result") == b"userdata-ready\n"
    verify_guest_files(harness, vm, payload)
    print("PASS offline guest: no NIC, isolated disk, file ownership/mode, secret environment, userdata ordering", flush=True)


def serial_terminal(harness):
    script = """set -eu
printf 'FIREMAGE_TERMINAL_READY\\n'
IFS= read -r line < /dev/ttyS0
[ "$line" = browser-input ]
printf '%s\\n' "$line" > /firemage/output/result
"""
    vm = harness.define("serial-terminal", script, terminal=True)
    harness.action(vm, "start")
    wait_for("serial terminal readiness", lambda: "FIREMAGE_TERMINAL_READY" in harness.console(vm))
    assert harness.request("GET", f"/v1/vms/{vm}/terminal")["state"] == "available"
    harness.request("POST", f"/v1/vms/{vm}/terminal", {"input": "browser-input\n"})
    harness.state(vm, "stopped")
    assert harness.output(vm, "result") == b"browser-input\n"
    serial = harness.request("GET", f"/v1/vms/{vm}/logs?stream=serial")
    api = harness.request("GET", f"/v1/vms/{vm}/logs?stream=firecracker")
    assert b"FIREMAGE_TERMINAL_READY" in base64.b64decode(serial["base64"])
    assert b"FIREMAGE_TERMINAL_READY" not in base64.b64decode(api["base64"])
    print("PASS ttyS0 terminal: guest input and serial/API stream separation", flush=True)


class UnixConnection(http.client.HTTPConnection):
    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self.host)


def lifecycle(harness):
    vm = harness.define("lifecycle-recovery", "echo FIREMAGE_GUEST_READY\nsleep 300\n")
    assert harness.action(vm, "prepare")["state"] == "ready"
    assert harness.action(vm, "start")["state"] == "running"
    wait_for("guest lifecycle readiness", lambda: "FIREMAGE_GUEST_READY" in harness.console(vm))
    assert harness.action(vm, "pause")["state"] == "paused"
    status = harness.request("POST", f"/v1/vms/{vm}/firecracker", {"method": "GET", "path": "/"})
    assert status["status"] == 200 and status["body"]["state"] == "Paused", status
    assert harness.action(vm, "resume")["state"] == "running"
    harness.stop_server()
    sockets = list((harness.data / "sockets").glob("*.sock"))
    owned = []
    for path in sockets:
        connection = UnixConnection(str(path), timeout=5)
        try:
            connection.request("GET", "/")
            response = connection.getresponse()
            state = json.loads(response.read())
            if response.status == 200 and state.get("state") == "Running":
                owned.append(path)
        except OSError:
            pass
        finally:
            connection.close()
    assert len(owned) == 1, f"Expected one live test VMM, got {owned}"
    connection = UnixConnection(str(owned[0]), timeout=5)
    try:
        connection.request("PATCH", "/vm", json.dumps({"state": "Paused"}), {"Content-Type": "application/json"})
        response = connection.getresponse()
        assert response.status == 204, response.read()
    finally:
        connection.close()
    harness.start_server()
    harness.state(vm, "paused", timeout=20)
    assert harness.action(vm, "resume")["state"] == "running"
    assert harness.action(vm, "stop")["state"] == "stopped"
    harness.state(vm, "stopped")
    print("PASS lifecycle: prepare, start, pause, resume, daemon recovery, stop", flush=True)


class Endpoint(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"host-endpoint-ok\n")

    def log_message(self, *_):
        pass


def host_only(harness):
    suffix = secrets.randbelow(100) + 20
    allowed, denied = f"198.18.{suffix}.1", f"198.18.{suffix}.2"
    gateway, guest = f"198.19.{suffix}.1", f"198.19.{suffix}.2"
    interface = f"fmt{os.getpid():x}"
    servers, threads = [], []
    subprocess.run(["ip", "link", "add", interface, "type", "dummy"], check=True, timeout=10)
    try:
        for address in (allowed, denied):
            subprocess.run(["ip", "addr", "add", f"{address}/32", "dev", interface], check=True, timeout=10)
        subprocess.run(["ip", "link", "set", interface, "up"], check=True, timeout=10)
        for address in (allowed, denied):
            server = http.server.ThreadingHTTPServer((address, 0), Endpoint)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            servers.append(server)
            threads.append(thread)
        ports = [server.server_address[1] for server in servers]
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        for address, port in zip((allowed, denied), ports):
            with opener.open(f"http://{address}:{port}/", timeout=5) as response:
                assert response.read() == b"host-endpoint-ok\n"
        name = "host-only-acceptance"
        harness.request("POST", "/v1/networks", {
            "name": name, "subnet": f"198.19.{suffix}.0/24", "gateway": gateway,
            "policy": {"mode": "host-only", "address": allowed},
        })
        harness.networks.append(name)
        script = f"""set -eu
command -v timeout >/dev/null
ip link set lo up
ip link set eth0 up
ip addr add {guest}/24 dev eth0 2>/dev/null || true
ip route add default via {gateway} dev eth0 2>/dev/null || true
wget -q -T 5 -O /firemage/output/allowed http://{allowed}:{ports[0]}/
if timeout 5 wget -q -T 3 -O /firemage/output/denied http://{denied}:{ports[1]}/; then
    echo 'host-only policy allowed the denied host address' >&2
    exit 1
fi
printf 'host-only-ok\\n' > /firemage/output/result
"""
        vm = harness.define("host-only-policy", script, network={
            "network": name, "address": guest, "mac": "02:fc:00:00:00:02",
        })
        harness.action(vm, "start")
        harness.state(vm, "stopped")
        assert harness.output(vm, "exit-code") == b"0\n"
        assert harness.output(vm, "allowed") == b"host-endpoint-ok\n"
        assert harness.output(vm, "result") == b"host-only-ok\n"
        print("PASS host-only: allowed host endpoint reachable, second live host endpoint blocked", flush=True)
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
        for thread in threads:
            thread.join(timeout=5)
        with contextlib.suppress(subprocess.SubprocessError):
            subprocess.run(["ip", "link", "delete", interface], check=True, timeout=10)

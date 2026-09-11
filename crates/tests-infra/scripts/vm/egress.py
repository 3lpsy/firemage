"""Real guest egress through TLS inspection and authenticated upstream CONNECT."""
import base64
import contextlib
import http.server
import os
import secrets
import select
import socket
import socketserver
import ssl
import subprocess
import threading

from .egress_recovery import recovery


class Origin(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        authorized = self.headers.get("Authorization") == "Bearer guest-proxy-test-secret"
        self.server.authorized.append(authorized)
        if hasattr(self.server, "paths"):
            self.server.paths.append(self.path)
        body = b"egress-origin-ok\n" if authorized else b"missing-credential\n"
        if authorized and self.path == "/allowed/stage":
            body = (self.server.stage + "\n").encode()
        self.send_response(200 if authorized else 401)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


class Echo(socketserver.BaseRequestHandler):
    def handle(self):
        while data := self.request.recv(4096):
            self.request.sendall(data)


class Upstream(http.server.BaseHTTPRequestHandler):
    def do_CONNECT(self):
        expected = "Basic " + base64.b64encode(b"proxy-user:proxy-test-password").decode()
        if self.headers.get("Proxy-Authorization") != expected or self.path not in self.server.targets:
            self.send_error(403)
            return
        host, port = self.path.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as target:
            self.server.seen.append(self.path)
            self.send_response(200)
            self.end_headers()
            readers = [self.connection, target]
            while readers:
                ready, _, _ = select.select(readers, [], [], 20)
                if not ready:
                    return
                for source in ready:
                    data = source.recv(65536)
                    destination = target if source is self.connection else self.connection
                    if data:
                        destination.sendall(data)
                    else:
                        readers.remove(source)
                        with contextlib.suppress(OSError):
                            destination.shutdown(socket.SHUT_WR)

    def log_message(self, *_):
        pass


def isolated_egress(harness):
    suffix = secrets.randbelow(100) + 120
    target, gateway, guest = f"198.18.{suffix}.1", f"198.19.{suffix}.1", f"198.19.{suffix}.2"
    interface = f"fme{os.getpid():x}"
    servers, threads = [], []
    subprocess.run(["ip", "link", "add", interface, "type", "dummy"], check=True, timeout=10)
    try:
        subprocess.run(["ip", "addr", "add", f"{target}/32", "dev", interface], check=True, timeout=10)
        subprocess.run(["ip", "link", "set", interface, "up"], check=True, timeout=10)
        plain = http.server.ThreadingHTTPServer((target, 0), Origin)
        tls = http.server.ThreadingHTTPServer((target, 0), Origin)
        echo = socketserver.ThreadingTCPServer((target, 0), Echo)
        echo.daemon_threads = True
        upstream = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Upstream)
        host_service = http.server.ThreadingHTTPServer(("0.0.0.0", 0), Origin)
        host_service.authorized = []
        plain.authorized, tls.authorized, upstream.seen = [], [], []
        plain.paths, tls.paths = [], []
        plain_port, tls_port, echo_port = [server.server_address[1] for server in (plain, tls, echo)]
        upstream.targets = {f"{target}:{port}" for port in (plain_port, tls_port, echo_port)}
        cert, key = harness.directory / "egress-origin.pem", harness.directory / "egress-origin.key"
        subprocess.run([
            "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
            "-subj", f"/CN={target}", "-addext", f"subjectAltName=IP:{target}",
            "-addext", "basicConstraints=critical,CA:FALSE", "-addext",
            "keyUsage=critical,digitalSignature,keyEncipherment", "-addext",
            "extendedKeyUsage=serverAuth", "-keyout", str(key), "-out", str(cert),
        ], check=True, capture_output=True, timeout=30)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        tls.socket = context.wrap_socket(tls.socket, server_side=True)
        servers.extend((plain, tls, echo, upstream, host_service))
        for server in servers:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            threads.append(thread)
        for name, value in (("egress-api-key", "guest-proxy-test-secret"),
                            ("egress-proxy-password", "proxy-test-password")):
            harness.request("PUT", f"/v1/secrets/{name}", {"value": value})
        network = "firemage-only-acceptance"
        harness.request("POST", "/v1/networks", {
            "name": network, "subnet": f"198.19.{suffix}.0/24", "gateway": gateway,
            "policy": {"mode": "firemage-only"},
        })
        harness.networks.append(network)
        rules = [{
            "scheme": scheme, "host": target, "port": port, "methods": ["GET"],
            "path_prefix": "/allowed", "allowed_ips": [f"{target}/32"],
            "headers": {"Authorization": {"secret": "egress-api-key", "prefix": "Bearer "}},
        } for scheme, port in (("http", plain_port), ("https", tls_port))]
        policy = {
            "http": {"port": 3128, "rules": rules, "upstream_ca_pem": cert.read_text()},
            "tunnels": [{"name": "echo", "listen_port": 19090, "target_host": target,
                         "target_port": echo_port}],
            "upstream": {"url": f"http://127.0.0.1:{upstream.server_address[1]}",
                         "username": "proxy-user", "password": {"secret": "egress-proxy-password"}},
        }
        script = guest_script(target, gateway, plain_port, tls_port, harness.port, host_service.server_address[1])
        vm = harness.define("firemage-only-egress", script, network={
            "network": network, "address": guest, "mac": "02:fc:00:00:00:04",
        }, egress=policy)
        harness.action(vm, "start")
        harness.state(vm, "stopped", timeout=120)
        assert harness.output(vm, "exit-code") == b"0\n", harness.console(vm)
        assert harness.output(vm, "result") == b"egress-policy-ok\n"
        assert plain.authorized and all(plain.authorized), "plain requests did not receive injected credentials"
        assert tls.authorized and all(tls.authorized), "TLS requests did not receive injected credentials"
        assert set(upstream.seen) == upstream.targets, "HTTP, TLS and TCP did not all traverse the upstream proxy"
        print("PASS isolated egress: HTTP/TLS rules, credential injection, upstream auth, binary tunnel, host bypass blocked", flush=True)
        recovery(harness, target, gateway, network, policy, plain, tls, host_service.server_address[1])
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
        for thread in threads:
            thread.join(timeout=5)
        with contextlib.suppress(subprocess.SubprocessError):
            subprocess.run(["ip", "link", "delete", interface], check=True, timeout=10)


def guest_script(target, gateway, plain_port, tls_port, management_port, host_port):
    return f"""set -eu
command -v curl >/dev/null
[ -n "$http_proxy" ]
[ -f "$CURL_CA_BUNDLE" ]
curl -fsS --max-time 10 -H 'Authorization: attacker-value' http://{target}:{plain_port}/allowed > /firemage/output/http
curl -fsS --max-time 10 https://{target}:{tls_port}/allowed > /firemage/output/https
grep -qx 'egress-origin-ok' /firemage/output/http
grep -qx 'egress-origin-ok' /firemage/output/https
[ "$(curl -sS --max-time 10 -o /dev/null -w '%{{http_code}}' https://{target}:{tls_port}/denied)" = 403 ]
[ "$(curl -sS --max-time 10 -X POST -o /dev/null -w '%{{http_code}}' https://{target}:{tls_port}/allowed)" = 403 ]
if curl -fsS --max-time 3 --noproxy '*' http://{target}:{plain_port}/allowed; then exit 1; fi
if curl -sS --max-time 3 --noproxy '*' http://{gateway}:{host_port}/; then exit 1; fi
if curl -kfsS --max-time 3 --noproxy '*' https://{gateway}:{management_port}/health; then exit 1; fi
if curl -fsS --max-time 3 --noproxy '*' http://{gateway}:19091/; then exit 1; fi
printf '\\000\\377binary-tunnel\\001' > /firemage/output/tunnel-input
(timeout 3 nc {gateway} 19090 < /firemage/output/tunnel-input || true) > /firemage/output/tunnel-output
cmp /firemage/output/tunnel-input /firemage/output/tunnel-output
printf 'egress-policy-ok\\n' > /firemage/output/result
"""

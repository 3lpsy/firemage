"""Boot a native OCI import from a private TLS registry on the disposable host."""
import base64
import gzip
import hashlib
import http.server
import json
import platform
import secrets
import shutil
import ssl
import subprocess
import tarfile
import threading


def digest(path):
    with path.open("rb") as stream:
        return "sha256:" + hashlib.file_digest(stream, "sha256").hexdigest()


def descriptor(path, media_type):
    return {"mediaType": media_type, "size": path.stat().st_size, "digest": digest(path)}


def image_fixture(harness, directory):
    root = directory / "rootfs"
    root.mkdir()
    # The source is the trusted warm fixture, not a guest-written disk.
    subprocess.run(["debugfs", "-R", f"rdump / {root}", str(harness.fixtures / "rootfs.ext4")],
                   check=True, capture_output=True, timeout=60)
    assert (root / "bin/busybox").is_file(), "OCI fixture extraction lacks BusyBox"
    archive = directory / "layer.tar"

    def regular_entries(entry):
        return entry if entry.isfile() or entry.isdir() or entry.issym() or entry.islnk() else None

    with tarfile.open(archive, "w", format=tarfile.PAX_FORMAT) as layer:
        for entry in sorted(root.iterdir()):
            layer.add(entry, arcname=entry.name, filter=regular_entries)
    compressed = directory / "layer.tar.gz"
    with archive.open("rb") as source, gzip.open(compressed, "wb") as target:
        shutil.copyfileobj(source, target)
    config = directory / "config.json"
    config.write_text(json.dumps({
        "architecture": {"x86_64": "amd64", "aarch64": "arm64"}[platform.machine()],
        "os": "linux", "rootfs": {"type": "layers", "diff_ids": [digest(archive)]},
        "config": {"User": "root", "Env": ["IMAGE_VALUE=from-oci"], "WorkingDir": "/",
                   "Entrypoint": ["/bin/sh"], "Cmd": ["/firemage/input/run.sh"]},
    }, separators=(",", ":")))
    manifest = directory / "manifest.json"
    manifest.write_text(json.dumps({
        "schemaVersion": 2, "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": descriptor(config, "application/vnd.oci.image.config.v1+json"),
        "layers": [descriptor(compressed, "application/vnd.oci.image.layer.v1.tar+gzip")],
    }, separators=(",", ":")))
    return manifest, config, compressed


class Registry(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.headers.get("Authorization") != self.server.expected_auth:
            self.server.challenges += 1
            self.send_response(401)
            self.send_header("WWW-Authenticate", 'Basic realm="native-oci-test"')
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        item = self.server.files.get(self.path)
        if item is None:
            self.send_error(404)
            return
        path, content_type = item
        self.server.authorized.add(self.path)
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(path.stat().st_size))
        self.send_header("Docker-Content-Digest", digest(path))
        self.end_headers()
        with path.open("rb") as source:
            shutil.copyfileobj(source, self.wfile)

    def log_message(self, *_):
        pass


def private_oci(harness):
    directory = harness.directory / "private-registry"
    directory.mkdir()
    manifest, config, layer = image_fixture(harness, directory)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Registry)
    server.daemon_threads = True
    password = secrets.token_hex(24)
    server.expected_auth = "Basic " + base64.b64encode(f"pull-user:{password}".encode()).decode()
    server.authorized, server.challenges = set(), 0
    server.files = {
        f"/v2/test/image/manifests/{digest(manifest)}": (manifest, "application/vnd.oci.image.manifest.v1+json"),
        f"/v2/test/image/blobs/{digest(config)}": (config, "application/vnd.oci.image.config.v1+json"),
        f"/v2/test/image/blobs/{digest(layer)}": (layer, "application/vnd.oci.image.layer.v1.tar+gzip"),
    }
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(harness.directory / "cert.pem", harness.directory / "key.pem")
    server.socket = context.wrap_socket(server.socket, server_side=True)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        harness.request("PUT", "/v1/secrets/oci-password", {"value": password})
        harness.request("PUT", "/v1/secrets/oci-ca", {"value": (harness.directory / "cert.pem").read_text()})
        userdata = "set -eu\n[ \"$IMAGE_VALUE\" = from-oci ]\nprintf 'ready\\n' > /firemage/output/userdata-result\n"
        script = """set -eu
[ "$(ls /sys/class/net)" = lo ]
[ "$IMAGE_VALUE" = from-oci ]
[ "$(cat /firemage/output/userdata-result)" = ready ]
printf 'native-private-oci-ok\\n' > /firemage/output/result
"""
        vm = harness.define("native-private-oci", script, userdata=userdata,
            boot_args="console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw", rootfs={
                "kind": "oci", "image": f"127.0.0.1:{server.server_port}/test/image@{digest(manifest)}",
                "size_mib": 128, "registry": {
                    "auth": {"kind": "basic", "username": "pull-user", "password_secret": "oci-password"},
                    "ca_secret": "oci-ca",
                },
            })
        returned = harness.request("GET", f"/v1/vms/{vm}")
        assert password not in json.dumps(returned), "VM response contains registry credentials"
        assert harness.action(vm, "start")["state"] == "running"
        harness.state(vm, "stopped")
        assert harness.output(vm, "exit-code") == b"0\n", harness.console(vm)
        assert harness.output(vm, "result") == b"native-private-oci-ok\n"
        assert server.challenges > 0 and server.authorized == set(server.files), "native pull did not authenticate every registry asset"
        assert not list((harness.data / "vms" / vm).glob(".firemage-oci-*")), "OCI extraction staging leaked"
        assert password not in harness.console(vm), "registry credentials leaked into guest logs"
        print("PASS native OCI: private TLS registry, vault authentication, image command/environment, userdata, offline guest", flush=True)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)

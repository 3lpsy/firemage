"""Minimal authenticated browser WebSocket client using only the standard library."""
import base64
import hashlib
import json
import os
import socket
import struct
import urllib.request


class WebSocket:
    def __init__(self, harness, vm):
        headers = {"Cookie": f"firemage_session={harness.token}", "Origin": harness.base}
        request = urllib.request.Request(harness.base + "/v1/browser/session", headers=headers)
        with harness.opener.open(request, timeout=10) as response:
            headers["X-CSRF-Token"] = json.load(response)["csrf_token"]
        request = urllib.request.Request(harness.base + f"/v1/vms/{vm}/shell/sessions",
                                         headers=headers, data=b"", method="POST")
        with harness.opener.open(request, timeout=10) as response:
            session = json.load(response)
            path = session["url"]
            ticket = session["ticket"]
        raw = socket.create_connection(("127.0.0.1", harness.port), timeout=10)
        self.socket = harness.context.wrap_socket(raw, server_hostname="127.0.0.1")
        key = base64.b64encode(os.urandom(16)).decode()
        try:
            self.socket.sendall((
                f"GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{harness.port}\r\n"
                f"Origin: {harness.base}\r\nCookie: firemage_session={harness.token}\r\n"
                "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n"
                f"Sec-WebSocket-Protocol: firemage-shell, {ticket}\r\n"
                f"Sec-WebSocket-Key: {key}\r\n\r\n").encode())
            response = bytearray()
            while not response.endswith(b"\r\n\r\n"):
                response.extend(self.exact(1))
                assert len(response) < 16_384, "oversized WebSocket upgrade headers"
            lines = response.decode().split("\r\n")
            assert lines[0].split()[1] == "101", f"WebSocket upgrade failed: {lines[0]}"
            received = {name.lower(): value.strip() for line in lines[1:] if ":" in line
                        for name, value in [line.split(":", 1)]}
            accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            assert received["sec-websocket-accept"] == accept
            assert received["sec-websocket-protocol"] == "firemage-shell"
            ready = self.receive()
            assert ready == {"type": "ready"}, f"guest helper did not open a PTY: {ready}"
        except BaseException:
            self.close()
            raise

    def exact(self, size):
        result = bytearray()
        while len(result) < size:
            chunk = self.socket.recv(size - len(result))
            assert chunk, "WebSocket closed before expected terminal output"
            result.extend(chunk)
        return bytes(result)

    def frame(self, opcode, data):
        assert len(data) <= 65_536
        size = len(data)
        if size < 126:
            header = bytes([0x80 | opcode, 0x80 | size])
        elif size <= 65_535:
            header = bytes([0x80 | opcode, 0xFE]) + struct.pack("!H", size)
        else:
            header = bytes([0x80 | opcode, 0xFF]) + struct.pack("!Q", size)
        mask = os.urandom(4)
        self.socket.sendall(header + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(data)))

    def send(self, message):
        self.frame(1, json.dumps(message).encode())

    def input(self, text):
        self.send({"type": "input", "data": base64.b64encode(text.encode()).decode()})

    def receive(self):
        while True:
            first, second = self.exact(2)
            assert first & 0x80 and not second & 0x80, "unexpected fragmented or masked server frame"
            size = second & 127
            if size == 126:
                size = struct.unpack("!H", self.exact(2))[0]
            elif size == 127:
                size = struct.unpack("!Q", self.exact(8))[0]
            assert size <= 65_536, "oversized shell frame"
            data = self.exact(size)
            opcode = first & 15
            if opcode == 9:
                self.frame(10, data)
                continue
            if opcode == 10:
                continue
            assert opcode == 1, f"unexpected WebSocket opcode {opcode}"
            return json.loads(data)

    def until(self, marker):
        output = ""
        while marker not in output:
            message = self.receive()
            assert message["type"] == "output", message
            output += base64.b64decode(message["data"], validate=True).decode(errors="replace")
            assert len(output) < 262_144, "expected marker missing from terminal output"
        return output

    def close(self):
        self.socket.close()

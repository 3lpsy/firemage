import json
import os
import socketserver
import sys


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        line = self.rfile.readline()
        if not line:
            return
        method, path, _ = line.decode().split()
        headers = {}
        while line := self.rfile.readline().strip():
            key, value = line.decode().split(":", 1)
            headers[key.lower()] = value.strip()
        body = self.rfile.read(int(headers.get("content-length", "0")))
        result = {"state": "Not started"} if path == "/" else None
        data = json.dumps(result).encode()
        self.wfile.write(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: " + str(len(data)).encode() + b"\r\n\r\n" + data)
        self.wfile.flush()
        if method == "PUT" and path == "/actions" and json.loads(body)["action_type"] == "InstanceStart":
            os._exit(0)


socket = sys.argv[sys.argv.index("--api-sock") + 1]
with socketserver.UnixStreamServer(socket, Handler) as server:
    server.serve_forever()

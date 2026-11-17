#!/usr/bin/env python3
"""Serve dist/ at http://127.0.0.1:4173/drive-study/ for the smoke test."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "dist"
PREFIX = "/drive-study"


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path.startswith(PREFIX):
            path = path[len(PREFIX) :] or "/"
        if path.endswith("/"):
            path += "index.html"
        file = (ROOT / path.lstrip("/")).resolve()
        if not str(file).startswith(str(ROOT.resolve())) or not file.is_file():
            self.send_error(404)
            return
        data = file.read_bytes()
        kind = "application/octet-stream"
        if file.suffix == ".html":
            kind = "text/html; charset=utf-8"
        elif file.suffix == ".js":
            kind = "text/javascript; charset=utf-8"
        elif file.suffix == ".css":
            kind = "text/css; charset=utf-8"
        elif file.suffix == ".json":
            kind = "application/json; charset=utf-8"
        elif file.suffix == ".wasm":
            kind = "application/wasm"
        elif file.suffix == ".svg":
            kind = "image/svg+xml"
        elif file.suffix == ".png":
            kind = "image/png"
        elif file.suffix == ".webmanifest":
            kind = "application/manifest+json"
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, fmt, *args):
        return


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", 4173), Handler).serve_forever()

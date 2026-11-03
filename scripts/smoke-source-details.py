#!/usr/bin/env python3
"""Validate source capabilities and live selection explanations with local upstreams."""
import http.cookiejar
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request


def upstream(status):
    class Handler(BaseHTTPRequestHandler):
        calls = 0

        def log_message(self, *_args):
            pass

        def do_GET(self):
            type(self).calls += 1
            self.send_response(status)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"module example.com/test\n")

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, Handler, f"http://127.0.0.1:{server.server_port}"


def main():
    binary = str(Path(sys.argv[1]).resolve())
    bad, bad_handler, bad_url = upstream(503)
    good, good_handler, good_url = upstream(200)
    try:
        with tempfile.TemporaryDirectory(prefix="mirrorproxy-source-details-") as directory:
            root = Path(directory)
            with socket.socket() as reservation:
                reservation.bind(("127.0.0.1", 0))
                port = reservation.getsockname()[1]
            base = f"http://127.0.0.1:{port}"
            config = root / "config.toml"
            config.write_text(f'''listen_addr = "127.0.0.1:{port}"
database_path = "{(root / 'data.sqlite3').as_posix()}"
enabled_proxies = ["go"]
forward_client_authorization = true
[geoip]
enabled = false
[upstreams]
go_proxy = "{bad_url}, {good_url}"
[upstream_selection]
strategy = "adaptive"
failure_threshold = 1
cooldown_secs = 60
''')
            password = secrets.token_urlsafe(32)
            environment = {key: value for key, value in os.environ.items() if not key.startswith("MIRRORPROXY_")}
            environment["MIRRORPROXY_ADMIN_PASSWORD"] = password
            cookies = http.cookiejar.CookieJar()
            browser = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPCookieProcessor(cookies))
            public = urllib.request.build_opener(urllib.request.ProxyHandler({}))

            def request(path, opener=public, body=None, status=200):
                headers = {"Content-Type": "application/json", "Origin": base}
                data = json.dumps(body).encode() if body is not None else None
                try:
                    response = opener.open(urllib.request.Request(base + path, data=data, headers=headers), timeout=5)
                except urllib.error.HTTPError as error:
                    response = error
                with response:
                    payload = response.read()
                    assert response.status == status, (path, response.status)
                    return json.loads(payload) if payload.startswith(b"{") else payload

            with (root / "server.log").open("w+") as log:
                process = subprocess.Popen([binary, "--config", str(config), "serve"], env=environment, stdout=log, stderr=log)
                try:
                    for _ in range(80):
                        try:
                            request("/healthz")
                            break
                        except (OSError, urllib.error.URLError):
                            assert process.poll() is None, "server exited before becoming healthy"
                            time.sleep(0.1)
                    else:
                        raise AssertionError("server did not become healthy")
                    build = request("/version")
                    assert build["version"] in subprocess.check_output([binary, "--version"], text=True)
                    expected_commit = os.environ.get("MIRRORPROXY_SMOKE_EXPECTED_COMMIT")
                    if expected_commit:
                        assert build["commit"] == expected_commit, "running binary commit differs from the expected build"
                    assert b'id="root"' in request("/admin"), "embedded console missing"
                    targets = {item["code"]: item["capabilities"] for item in request("/api/sources")["targets"]}
                    assert targets["go"]["cache"] == "conditional" and not targets["go"]["cache_enabled"]
                    assert targets["go"]["client_authorization_forwarding"]
                    assert targets["huggingface"]["cache"] == "bypass"
                    assert targets["huggingface"]["authentication"] == "public_only"
                    assert not targets["huggingface"]["client_authorization_forwarding"]
                    assert targets["docker"]["cache"] == targets["homebrew"]["cache"] == "bypass"
                    request("/admin/api/source-health", status=401)
                    request("/admin/api/auth/login", browser, {"username": "admin", "password": password})
                    assert request("/admin/api/source-health", browser)["selection"]["samples"] == []
                    download_path = "/goproxy/example.com/test/@v/v1.0.0.mod?token=" + secrets.token_urlsafe(12)
                    for _ in range(2):
                        downloaded = request(download_path)
                        assert downloaded == b"module example.com/test\n", (type(downloaded).__name__, repr(downloaded)[:120])
                        selection = request("/admin/api/source-health", browser)["selection"]
                        endpoints = selection["samples"][0]["endpoints"]
                        assert selection["strategy"] == "adaptive"
                        assert endpoints[0]["origin"] == good_url and endpoints[0]["configured_position"] == 2
                        assert endpoints[1]["origin"] == bad_url and endpoints[1]["reason"] == "cooldown"
                        assert endpoints[1]["consecutive_failures"] == 1 and endpoints[1]["cooldown_remaining_secs"] > 0
                        assert "token" not in json.dumps(selection)
                    assert bad_handler.calls == 1 and good_handler.calls == 2
                    assert "selection" not in request("/api/source-health")
                    print("source capabilities, HF credential isolation, live fallback/ranking, cooldown, redaction, and administrator boundary passed")
                finally:
                    process.terminate()
                    process.wait(timeout=10)
    finally:
        bad.shutdown()
        good.shutdown()
        bad.server_close()
        good.server_close()


if __name__ == "__main__":
    main()

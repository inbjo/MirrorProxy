#!/usr/bin/env python3
"""Real process SIGTERM/restart acceptance with an active proxied response."""
import http.client
import http.server
import json
import os
from pathlib import Path
import secrets
import socket
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

binary = str(Path(sys.argv[1]).resolve())
release = threading.Event()
class Upstream(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *_): pass
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/octet-stream')
        self.send_header('Transfer-Encoding', 'chunked')
        self.end_headers()
        self.wfile.write(b'5\r\nhello\r\n'); self.wfile.flush()
        if not release.wait(10): return
        self.wfile.write(b'5\r\nworld\r\n0\r\n\r\n'); self.wfile.flush()

upstream = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
threading.Thread(target=upstream.serve_forever, daemon=True).start()
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
with tempfile.TemporaryDirectory(prefix='mirrorproxy-shutdown-') as directory:
    directory = Path(directory)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    database = directory / 'state.sqlite3'
    config = directory / 'config.toml'
    config.write_text(f'''listen_addr = "127.0.0.1:{port}"
database_path = "{database}"
enabled_proxies = ["npm"]
[upstreams]
npm = "http://127.0.0.1:{upstream.server_port}"
[quota]
enabled = true
monthly_gb = 1
bidirectional_accounting = false
[geoip]
enabled = false
''')
    env = dict(os.environ, MIRRORPROXY_ADMIN_PASSWORD='Smoke-' + secrets.token_urlsafe(24), MIRRORPROXY_MASTER_KEY=secrets.token_hex(32))
    def start(log):
        process = subprocess.Popen([binary, '--config', str(config), 'serve'], stdout=log, stderr=log, env=env)
        for _ in range(100):
            if process.poll() is not None: raise AssertionError('server exited during startup')
            try:
                with opener.open(f'http://127.0.0.1:{port}/readyz', timeout=1) as response:
                    assert json.load(response)['status'] == 'ready'
                    return process
            except OSError: time.sleep(.1)
        process.kill(); process.wait(); raise AssertionError('server never became ready')
    def totals():
        with sqlite3.connect(database) as db:
            return db.execute('SELECT SUM(request_count), SUM(response_bytes), SUM(reserved_bytes) FROM traffic_monthly').fetchone()
    with (directory / 'server.log').open('w') as log:
        process = start(log)
        try:
            connection = http.client.HTTPConnection('127.0.0.1', port, timeout=10)
            connection.request('GET', '/npm/payload.tgz')
            response = connection.getresponse(); assert response.status == 200
            assert response.read(5) == b'hello'
            assert totals()[2] > 0, 'active response must hold a quota reservation'
            process.terminate()
            release.set()
            assert response.read() == b'world'
            response.close(); connection.close()
            assert process.wait(timeout=15) == 0
            assert totals() == (1, 10, 0), f'shutdown settlement mismatch: {totals()}'
            process = start(log)
            assert totals() == (1, 10, 0), 'restart must preserve settled bytes without duplicate billing'
            process.terminate(); assert process.wait(timeout=15) == 0
            assert totals() == (1, 10, 0)
            print('Active response completed across SIGTERM; exact billing, zero reservation and restart persistence passed')
        finally:
            release.set()
            if process.poll() is None:
                process.kill(); process.wait()
            upstream.shutdown(); upstream.server_close()

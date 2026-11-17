#!/usr/bin/env python3
"""Optional public DoH checks, separate from deterministic protocol acceptance."""
import argparse
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('binary', type=Path)
parser.add_argument('--upstream', action='append', required=True)
parser.add_argument('--report', type=Path)
args = parser.parse_args()
results = []
for upstream in args.upstream:
    with tempfile.TemporaryDirectory(prefix='mirrorproxy-doh-public-') as directory:
        directory = Path(directory)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
        config = directory / 'config.toml'
        config.write_text(f'''listen_addr = "127.0.0.1:{port}"
database_path = "{directory}/state.sqlite3"
[doh]
enabled = true
upstreams = [{json.dumps(upstream)}]
ecs_mode = "passthrough"
[geoip]
enabled = false
''')
        env = dict(os.environ, MIRRORPROXY_ADMIN_PASSWORD='Smoke-' + secrets.token_urlsafe(24), MIRRORPROXY_MASTER_KEY=secrets.token_hex(32))
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with (directory / 'server.log').open('w') as log:
            process = subprocess.Popen([str(args.binary.resolve()), '--config', str(config), 'serve'], stdout=log, stderr=log, env=env)
            try:
                for _ in range(100):
                    if process.poll() is not None: raise RuntimeError('local server startup failed')
                    try:
                        opener.open(f'http://127.0.0.1:{port}/healthz', timeout=1).close(); break
                    except OSError: time.sleep(.1)
                result = subprocess.run(['python3', str(Path(__file__).with_name('smoke-doh.py')), f'http://127.0.0.1:{port}'], capture_output=True, text=True, timeout=100)
                results.append({'upstream': upstream, 'passed': result.returncode == 0, 'output': result.stdout, 'error': result.stderr})
                print(f'{upstream}: {"passed" if result.returncode == 0 else "failed"}', flush=True)
                print(result.stdout if result.returncode == 0 else result.stderr, flush=True)
            finally:
                process.terminate()
                try: process.wait(timeout=15)
                except subprocess.TimeoutExpired: process.kill(); process.wait()
if args.report:
    args.report.write_text(json.dumps({'scope': 'public upstream connectivity and protocol; environment dependent', 'results': results}, indent=2))
raise SystemExit(0 if all(result['passed'] for result in results) else 1)

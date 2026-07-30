#!/usr/bin/env bash
set -euo pipefail

server_binary="${1:-target/debug/mirrorproxy-server}"
if [[ ! -x "$server_binary" ]]; then
  echo "server binary is not executable: $server_binary" >&2
  exit 1
fi

smoke_dir="$(mktemp -d)"
server_pid=""
cleanup() {
  exit_code=$?
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  if [[ $exit_code -ne 0 && -f "$smoke_dir/server.log" ]]; then
    echo "server log from failed smoke:" >&2
    sed -n '1,160p' "$smoke_dir/server.log" >&2
  fi
  if [[ -n "$smoke_dir" && "$smoke_dir" == /tmp/* ]]; then
    rm -rf -- "$smoke_dir"
  fi
  return "$exit_code"
}
trap cleanup EXIT

listen_addr="127.0.0.1:31873"
management_addr="127.0.0.1:31874"
config_path="$smoke_dir/config.toml"
database_path="$smoke_dir/mirrorproxy.sqlite3"
cookie_jar="$smoke_dir/cookies.txt"
cat >"$config_path" <<EOF
listen_addr = "$listen_addr"
database_path = "$database_path"
public_base_url = "http://$listen_addr"
enabled_proxies = ["npm"]

[management]
enabled = true
listen_addr = "$management_addr"

[geoip]
enabled = false
ipv4_path = "missing.xdb"
ipv6_path = "missing.xdb"
EOF

MIRRORPROXY_ADMIN_PASSWORD='MirrorProxy-Smoke-Password-1!' \
MIRRORPROXY_MASTER_KEY='000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f' \
  "$server_binary" --config "$config_path" serve >"$smoke_dir/server.log" 2>&1 &
server_pid=$!

for _ in $(seq 1 80); do
  if curl --fail --silent "http://$listen_addr/healthz" >/dev/null; then break; fi
  sleep 0.25
done
curl --fail --silent "http://$listen_addr/healthz" >/dev/null
curl --fail --silent "http://$listen_addr/admin" >/dev/null
curl --fail --silent "http://$listen_addr/metrics" >/dev/null
test "$(curl --silent --output /dev/null --write-out '%{http_code}' \
  -H 'X-Forwarded-For: 203.0.113.7' "http://$listen_addr/metrics")" = "403"
curl --fail --silent --cookie-jar "$cookie_jar" \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"MirrorProxy-Smoke-Password-1!"}' \
  "http://$listen_addr/admin/api/auth/login" >/dev/null
curl --fail --silent --cookie "$cookie_jar" "http://$listen_addr/admin/api/config" >/dev/null
curl --fail --silent --cookie "$cookie_jar" "http://$management_addr/admin/api/config" >/dev/null
curl --fail --silent --cookie "$cookie_jar" "http://$management_addr/admin/api/cache" >/dev/null
curl --fail --silent --cookie "$cookie_jar" -X DELETE "http://$management_addr/admin/api/cache" >/dev/null

kill "$server_pid"
wait "$server_pid"
server_pid=""
MIRRORPROXY_MASTER_KEY='000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f' \
  "$server_binary" --config "$config_path" doctor --json >/dev/null
MIRRORPROXY_MASTER_KEY='000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f' \
  "$server_binary" --config "$config_path" backup "$smoke_dir/backup.sqlite3" >/dev/null
test -s "$smoke_dir/backup.sqlite3"

echo "real public/private admin API, local-only metrics, encrypted database, doctor, and backup smoke passed"

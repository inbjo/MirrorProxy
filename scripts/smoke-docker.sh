#!/usr/bin/env bash
set -euo pipefail

image="${1:?usage: smoke-docker.sh IMAGE GIT_COMMIT}"
expected_commit="${2:?Git commit required}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/crates/server/Cargo.toml" | head -n1)"
version="$(docker run --rm "$image" /usr/local/bin/mirrorproxy-server --version)"
test "$version" = "mirrorproxy-server $expected_version"
revision="$(docker image inspect "$image" --format '{{index .Config.Labels "org.opencontainers.image.revision"}}')"
test "$revision" = "$expected_commit"

work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
export DOCKER_SMOKE_IMAGE="$image" DOCKER_SMOKE_ROOT="$work"
cat >"$work/server" <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
exec docker run --rm --network host \
  --user "$(id -u):$(id -g)" \
  --mount "type=bind,source=$DOCKER_SMOKE_ROOT,target=$DOCKER_SMOKE_ROOT" \
  --env MIRRORPROXY_ADMIN_PASSWORD \
  --env MIRRORPROXY_LISTEN_ADDR \
  --env MIRRORPROXY_DB \
  --env MIRRORPROXY_CACHE_DIRECTORY \
  --env MIRRORPROXY_GEOIP_IPV4_PATH \
  --env MIRRORPROXY_GEOIP_IPV6_PATH \
  --entrypoint /usr/local/bin/mirrorproxy-server \
  "$DOCKER_SMOKE_IMAGE" "$@"
WRAPPER
chmod +x "$work/server"
MIRRORPROXY_SMOKE_EXPECTED_COMMIT="$expected_commit" TMPDIR="$work" python3 "$root/scripts/smoke-source-details.py" "$work/server"
printf 'Native Docker entrypoint, version, revision, embedded console and proxy protocol passed: %s\n' "$image"

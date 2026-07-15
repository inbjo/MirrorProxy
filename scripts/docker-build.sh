#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="${MIRRORPROXY_DOCKER_IMAGE:-kudang/mirrorproxy}"
version="${MIRRORPROXY_DOCKER_VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${root}/crates/server/Cargo.toml" | head -n 1)}"
platforms=""
push=0
latest=1

usage() {
  cat <<'EOF'
Build the MirrorProxy server container image.

Usage: scripts/docker-build.sh [options]
  --image NAME          Image repository (default: kudang/mirrorproxy)
  --version VERSION     Image version tag (default: server crate version)
  --platforms LIST      Buildx platforms (push default: linux/amd64,linux/arm64)
  --push                Push a multi-platform image instead of loading locally
  --no-latest           Do not also tag the image as latest
  -h, --help            Show this help

Environment equivalents:
  MIRRORPROXY_DOCKER_IMAGE, MIRRORPROXY_DOCKER_VERSION,
  MIRRORPROXY_DOCKER_PLATFORMS, MIRRORPROXY_DOCKER_BASE_REGISTRY
EOF
}

while (($#)); do
  case "$1" in
    --image) image="${2:?--image requires a value}"; shift 2 ;;
    --version) version="${2:?--version requires a value}"; shift 2 ;;
    --platforms) platforms="${2:?--platforms requires a value}"; shift 2 ;;
    --push) push=1; shift ;;
    --no-latest) latest=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

command -v docker >/dev/null 2>&1 || { echo "docker is required" >&2; exit 1; }
docker buildx version >/dev/null 2>&1 || { echo "docker buildx is required" >&2; exit 1; }

version="${version#v}"
git_commit="$(git -C "${root}" rev-parse HEAD)"
build_time="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
tags=(--tag "${image}:${version}")
((latest)) && tags+=(--tag "${image}:latest")

if ((push)); then
  platforms="${platforms:-${MIRRORPROXY_DOCKER_PLATFORMS:-linux/amd64,linux/arm64}}"
  output=(--push)
else
  platforms="${platforms:-${MIRRORPROXY_DOCKER_PLATFORMS:-linux/amd64}}"
  if [[ "${platforms}" == *,* ]]; then
    echo "local --load builds support one platform; use --push for multiple platforms" >&2
    exit 2
  fi
  output=(--load)
fi

docker buildx build \
  --file "${root}/Dockerfile" \
  --platform "${platforms}" \
  --build-arg "VERSION=${version}" \
  --build-arg "GIT_COMMIT=${git_commit}" \
  --build-arg "BUILD_TIME=${build_time}" \
  --build-arg "BASE_IMAGE_REGISTRY=${MIRRORPROXY_DOCKER_BASE_REGISTRY:-docker.io/library}" \
  "${tags[@]}" \
  "${output[@]}" \
  "${root}"

suffix=""
((push)) && suffix=" and pushed"
printf 'Built %s:%s for %s%s\n' \
  "${image}" "${version}" "${platforms}" "${suffix}"

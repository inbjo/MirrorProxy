#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESTINATION="${MIRRORPROXY_GEOIP_DESTINATION:-$ROOT_DIR/geoip}"
V4_URL="https://raw.githubusercontent.com/lionsoul2014/ip2region/master/data/ip2region_v4.xdb"
V6_URL="https://raw.githubusercontent.com/lionsoul2014/ip2region/master/data/ip2region_v6.xdb"
V4_SHA256="6307a9696f5711f84bcb8b25f07894de68a64a0ed4a1cc7e990562dd3084f210"
V6_SHA256="5b93da35ac28bc316dccc54a758381f7a874ae0461dd51ff5df5e34815586f11"

mkdir -p "$DESTINATION"

fetch() {
  local url="$1" expected="$2" target="$3"
  local temporary="${target}.tmp"
  if [[ -f "$target" ]] && printf '%s  %s\n' "$expected" "$target" | sha256sum --check --status; then
    return
  fi
  curl --fail --location --retry 3 --output "$temporary" "$url"
  printf '%s  %s\n' "$expected" "$temporary" | sha256sum --check --status
  mv "$temporary" "$target"
}

fetch "$V4_URL" "$V4_SHA256" "$DESTINATION/ip2region_v4.xdb"
fetch "$V6_URL" "$V6_SHA256" "$DESTINATION/ip2region_v6.xdb"
printf 'GeoIP databases ready in %s\n' "$DESTINATION"

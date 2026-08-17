#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESTINATION="${MIRRORPROXY_GEOIP_DESTINATION:-$ROOT_DIR/geoip}"
IP2REGION_COMMIT="800d19424237f4be5f2081e6cd9547d98f3871c3"
V4_URL="https://raw.githubusercontent.com/lionsoul2014/ip2region/$IP2REGION_COMMIT/data/ip2region_v4.xdb"
V6_URL="https://raw.githubusercontent.com/lionsoul2014/ip2region/$IP2REGION_COMMIT/data/ip2region_v6.xdb"
V4_SHA256="c6edaf379fe524d7283a9c11c7eac27d5641a0976baa48c22c319ccd59aa3f36"
V6_SHA256="939f6b46bd2b8bec3cf7c5ceb8ba782266ae9b1f35b5ba7916700dec0b7506ed"

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

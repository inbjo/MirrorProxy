#!/bin/sh
set -eu

mkdir -p /data/geoip
for database in ip2region_v4.xdb ip2region_v6.xdb; do
  if [ ! -f "/data/geoip/$database" ]; then
    cp "/usr/share/mirrorproxy/geoip/$database" "/data/geoip/$database"
  fi
done

exec "$@"

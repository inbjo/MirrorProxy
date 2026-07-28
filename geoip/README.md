# GeoIP runtime data

MirrorProxy release archives and container images include the IPv4 and IPv6
ip2region XDB v3 databases in this directory. Source checkouts fetch the pinned
files with `bash scripts/fetch-geoip.sh`; the binary data is intentionally not
stored in the main Git history.

ip2region is Copyright Lionsoul and contributors, distributed under
Apache-2.0 OR MIT: <https://github.com/lionsoul2014/ip2region>.

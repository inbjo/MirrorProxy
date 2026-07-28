# GeoIP and IP Access Control

MirrorProxy uses the offline ip2region XDB v3 databases for IPv4 and IPv6.
Release archives and container images include pinned database snapshots. The
admin console shows each database's path, size, timestamp and SHA-256, supports
non-persistent single-IP lookups, and lets a super administrator perform an
atomic manual update. Missing or corrupt databases degrade locations to
`Unknown` without stopping proxy traffic or access rules.

IP access rules accept exact IPv4/IPv6 addresses and CIDR ranges. Exact values
are normalized to `/32` or `/128`; CIDRs are normalized to their network
address. Deny rules always win. When one or more allow rules are enabled, any
unmatched client is denied. Rules apply only to recognized proxy paths, so the
admin console and health endpoints cannot be locked out.

Regional reports aggregate completed proxy responses by day, target, country,
region and city. They show delivered and billed bytes separately. Raw client
IPs and ad-hoc lookup results are never stored. Historic aggregates are not
reclassified when a database is updated.

[中文](GeoIP-and-IP-Access-Control-zh-CN)

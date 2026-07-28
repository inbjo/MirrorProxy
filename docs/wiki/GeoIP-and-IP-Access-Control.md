# GeoIP and IP Access Control

MirrorProxy uses offline ip2region XDB v3 files for IPv4 and IPv6. Releases and
images include pinned data; the console shows path, size, timestamp, and SHA-256
and a super administrator can update it atomically. One-off lookups are not
stored. Missing or invalid data becomes `Unknown` without stopping the proxy or
IP rules.

## Trusted client address

GeoIP, access rules, and rate limits use the client source address. For direct
connections this is the TCP peer. Behind a reverse proxy, `X-Forwarded-For` is
read only when that peer matches `trusted_proxies`, and is resolved right to left.
Only list real proxies, never `0.0.0.0/0`. A single Nginx hop should overwrite
client-provided XFF; every hop in a multi-proxy chain must be trusted.

## Rules

- Exact IPv4/IPv6 and CIDR are accepted and normalized to `/32`, `/128`, or the network.
- Deny rules always win.
- Once an enabled allow rule exists, every non-match is denied.
- Rules apply only to recognized package-proxy paths, never `/admin`, `/healthz`,
  or control APIs.

Add and verify your own egress IP from a second network before enabling an
allowlist-only policy.

## Regional reports

Reports aggregate completed responses by date range, target, country,
region/province, and city. They show delivered bytes, billable bytes, and request
counts. Billable bytes can exceed delivered bytes when bidirectional accounting
is enabled. Historical aggregates do not store raw client IPs and are not
rewritten after a GeoIP update. `Unknown` normally means private/reserved space,
no XDB result, or an incorrect trusted-proxy configuration.

[简体中文](GeoIP-and-IP-Access-Control-zh-CN)

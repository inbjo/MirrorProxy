# Deployment

The server archive contains `mirrorproxy-server`, `config.example.toml`, and
IPv4/IPv6 ip2region data. Keep SQLite, cache, GeoIP, and ACME storage on durable
local storage for either Docker or binary deployments; retain that data directory
across image and binary upgrades.

## Deployment modes

### TLS at a reverse proxy

The default mode binds an internal address such as `127.0.0.1:3000`. Nginx,
Caddy, Traefik, Apache, or HAProxy provides public TLS and forwards the original
`Host`, path, and method. This fits servers where the edge already owns
certificates. Only put actual reverse-proxy IPs/networks in `trusted_proxies`:
MirrorProxy reads `X-Forwarded-For` only from a trusted TCP peer and resolves the
chain right to left.

A single Nginx hop must overwrite client-supplied XFF with `$remote_addr`; a
multi-hop chain is safe only when every intermediate hop is trusted. Expose the
console through HTTPS as well.

### Native MirrorProxy HTTPS

Without Caddy/Nginx, enable ACME `direct_https` in the console or TOML.
MirrorProxy binds 80 and 443, keeps HTTP-01 reachable, and sends other HTTP
requests to HTTPS with 308 after a certificate is ready. Renewals hot-reload
without restart. See [ACME Certificates](ACME-Certificates).

For a binary service, prefer a non-root account with systemd
`CAP_NET_BIND_SERVICE`. The container is non-root by default; use internal ports
3000/3443 and map host ports 80/443 instead of adding unnecessary capability.

## Docker operating notes

- Persist `/data`; the image keeps its database, cache, and writable GeoIP there.
- Environment variables override TOML. `MIRRORPROXY_ACME_*` also makes the console
  ACME fields read-only.
- Pin an image version before an upgrade, then verify `/healthz`, `/admin`, one
  public proxy target, and logs.
- Keep SQLite on local disk, not a network filesystem, for a single-instance deployment.

## User subdomains

`subdomain_required` is an optional per-user routing/accounting mode, not a
requirement for ordinary proxying. Enable it only after wildcard DNS, wildcard
TLS, and original Host forwarding are ready and the console's readiness check is
confirmed; otherwise proxy routes on the base domain are deliberately rejected.

## Enterprise CA for upstream mirrors

Mirror upstream requests trust public WebPKI and OS roots. Add private PEM
bundles when an enterprise TLS proxy signs upstream traffic:

```toml
[upstream_tls]
ca_certificates = ["/etc/mirrorproxy/ca/company-root.pem"]
insecure_skip_verify = false
```

Mount this file into containers. `insecure_skip_verify = true` disables TLS
verification for every mirror upstream and is only for short-lived diagnostics;
it does not affect ACME, DNS provider APIs, or OAuth control-plane requests.

[简体中文](Deployment-zh-CN)

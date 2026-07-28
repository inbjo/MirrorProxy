# Deployment

Release archives contain `mirrorproxy-server`, `config.example.toml`, and the
IPv4/IPv6 ip2region databases. Run the service from a persistent working
directory or override `MIRRORPROXY_DB` and the two `MIRRORPROXY_GEOIP_*_PATH`
variables.

Production installations should keep the listener private and terminate TLS at
Nginx, Caddy, Traefik, Apache, or HAProxy. Configure `trusted_proxies` with only
the proxy peers. MirrorProxy walks `X-Forwarded-For` from right to left and
accepts it only when the immediate peer is trusted. A single Nginx proxy should
overwrite the header with `$remote_addr`; multi-proxy installations may append
a chain only when every intermediate proxy is listed as trusted.

Wildcard user subdomains require wildcard DNS, a wildcard TLS certificate, and
preservation of the original Host header. Enable `subdomain_required` only
after those checks pass.

## Enterprise CAs for mirror upstreams

The mirror-upstream client trusts both WebPKI public roots and the operating
system root store by default. Mount one or more PEM bundles when a private CA
signs an internal HTTPS upstream:

```toml
[upstream_tls]
ca_certificates = ["/etc/mirrorproxy/ca/company-root.pem"]
insecure_skip_verify = false
```

Docker deployments must mount each certificate into the container first:

```yaml
volumes:
  - ./company-root.pem:/etc/mirrorproxy/ca/company-root.pem:ro
```

Environment equivalents are also available:

```text
MIRRORPROXY_UPSTREAM_TLS_CA_CERTIFICATES=/etc/mirrorproxy/ca/company-root.pem
MIRRORPROXY_UPSTREAM_TLS_INSECURE_SKIP_VERIFY=false
```

Setting `insecure_skip_verify = true` disables certificate verification for all
mirror-upstream HTTPS requests and permits man-in-the-middle attacks. Use it
only temporarily for debugging. Additional CAs and this unsafe switch never
apply to ACME, DNS-provider APIs, OAuth, or other control-plane requests.

[中文](Deployment-zh-CN)

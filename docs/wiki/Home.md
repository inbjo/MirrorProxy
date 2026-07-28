# MirrorProxy Wiki

MirrorProxy is a self-hosted Rust mirror-proxy platform with three independently
evolving parts:

- `mirrorproxy-server`: proxy service, SQLite database, and embedded React console.
- `mirrorproxy`: standalone Windows, macOS, and Linux source-management CLI.
- `mirrorproxy-catalog`: shared target, route, and local-configuration catalog.

It provides one controlled endpoint for GitHub, OCI, language registries,
toolchains, and operating-system repositories. The console adds users and
quotas, source health, audit logs, offline GeoIP reporting, IP/CIDR allow/deny
rules, and ACME certificate management. GeoIP lookups stay in local XDB files.

## Choose a deployment mode

| Situation | Recommended mode |
| --- | --- |
| Existing Caddy/Nginx/Traefik | Keep TLS at the reverse proxy and bind MirrorProxy internally. |
| Single host without a reverse proxy | Enable ACME `direct_https`; MirrorProxy binds 80/443 and redirects HTTP to HTTPS. |
| Ordinary hostnames | Use HTTP-01; public port 80 must reach MirrorProxy. |
| Wildcard certificates | Use DNS-01; Cloudflare, Aliyun, DNSPod, Route53, and webhook are supported. |

Start with Getting Started; read Deployment and ACME Certificates before a
production cutover.

- [Getting Started](Getting-Started)
- [Deployment](Deployment)
- [Proxy Adapters](Proxy-Adapters)
- [Client](Client)
- [Client distribution](Distribution)
- [Administration](Administration)
- [GeoIP and IP Access](GeoIP-and-IP-Access-Control)
- [ACME Certificates](ACME-Certificates)
- [Development](Development-and-Roadmap)

[简体中文](Home-zh-CN)

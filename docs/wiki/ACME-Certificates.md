# Automatic ACME certificates

MirrorProxy can obtain and renew certificates through ACME and atomically write
`fullchain.pem` and `privkey.pem`. A reverse proxy can consume them, or MirrorProxy can bind
ports 80/443 and serve HTTPS itself. ACME is disabled by default. Account keys and certificates
stay in the configured local directory. DNS API credentials entered in the admin console are
stored in local SQLite as write-only secrets: APIs return only configured-state flags, never
plaintext credentials.

## Admin configuration

Super administrators can configure HTTP-01, DNS-01, certificate domains, renewal intervals,
and provider credentials under Advanced settings. The console writes a dedicated
`acme_settings` table and never rewrites `config.toml`. Restart MirrorProxy after saving to
replace the running ACME worker. Saving by itself never issues or replaces a certificate and
never changes Caddy.

When `MIRRORPROXY_ACME_*` or compatible acme.sh credential variables are present, environment
configuration keeps precedence and the admin form becomes read-only. Without those variables,
the first upgraded startup seeds database settings from the current TOML configuration so
existing values are retained.

## Serving HTTPS without a reverse proxy

With `direct_https` enabled, MirrorProxy manages an HTTP and an HTTPS listener. The HTTP
listener always serves `/.well-known/acme-challenge/*`. Once a certificate is ready, other HTTP
requests receive a 308 redirect that preserves the full path and query. Before the first
certificate is available, ordinary HTTP requests return 503 instead of redirecting clients to
an unavailable port.

```toml
[acme]
enabled = true
email = "admin@example.com"
domains = ["mirror.example.com"]
challenge = "http-01"
storage_directory = "/var/lib/mirrorproxy/acme"
direct_https = true
http_listen_addr = "0.0.0.0:80"
https_listen_addr = "0.0.0.0:443"
redirect_http_to_https = true
```

An existing valid certificate is loaded at startup. Port 443 starts automatically after the
first issuance, and renewed certificates are hot-reloaded without interrupting existing
connections. Invalid renewed files leave the previous certificate active. The two listen
addresses must differ. DNS-01 can still use the HTTP listener for redirects, although its
validation does not require port 80.

Do not run the whole service as root merely to bind low ports. A systemd service can grant only
the required capability:

```ini
[Service]
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=true
```

The Docker/Podman image runs as a non-root user. Prefer
`http_listen_addr = "0.0.0.0:3000"` and `https_listen_addr = "0.0.0.0:3443"`, then publish
`80:3000` and `443:3443`; no additional container capability is required. Changing listener
mode or addresses requires a restart; certificate renewal does not.

## HTTP-01

Use HTTP-01 for concrete hostnames. A/AAAA records must resolve to the public frontend. In
reverse-proxy mode, port 80 must forward `/.well-known/acme-challenge/*` unchanged to
MirrorProxy; native HTTPS mode serves it directly. HTTP-01 cannot issue wildcard certificates.

```toml
[acme]
enabled = true
email = "admin@example.com"
domains = ["mirror.example.com"]
challenge = "http-01"
storage_directory = "/var/lib/mirrorproxy/acme"
```

Nginx example:

```nginx
location ^~ /.well-known/acme-challenge/ {
    proxy_pass http://127.0.0.1:3000;
}
```

Caddy already manages certificates automatically. Enable MirrorProxy ACME behind Caddy only
when MirrorProxy must be the certificate-file source, and route challenges before redirects.

## DNS-01

DNS-01 supports concrete and wildcard names without requiring port 80. Built-in providers are
Cloudflare, Alibaba Cloud DNS, Tencent Cloud DNSPod, AWS Route53, and a generic webhook.

```toml
[acme]
enabled = true
email = "admin@example.com"
domains = ["example.com", "*.example.com"]
challenge = "dns-01"
storage_directory = "/var/lib/mirrorproxy/acme"

[acme.dns]
provider = "cloudflare"
cloudflare_zone_id = "zone-id"
propagation_delay_secs = 30
```

Provide a zone-scoped token through `MIRRORPROXY_ACME_CLOUDFLARE_API_TOKEN`. The acme.sh
environment names `CF_Zone_ID`, `CF_Token`, `CF_Key`, and `CF_Email` are also recognized; the
broader Global API Key is not recommended for new deployments.

Other native providers use an explicit managed zone so MirrorProxy never guesses public
suffixes such as `co.uk`:

```toml
# Alibaba Cloud DNS
[acme.dns]
provider = "aliyun"
aliyun_domain = "example.com"
# Credentials: MIRRORPROXY_ACME_ALIYUN_ACCESS_KEY_ID / ..._ACCESS_KEY_SECRET
# acme.sh aliases: Ali_Key / Ali_Secret
```

```toml
# Tencent Cloud DNSPod
[acme.dns]
provider = "tencent"
tencent_domain = "example.com"
# Credentials: MIRRORPROXY_ACME_TENCENT_SECRET_ID / ..._SECRET_KEY
# acme.sh aliases: Tencent_SecretId / Tencent_SecretKey
```

```toml
# AWS Route53
[acme.dns]
provider = "route53"
route53_hosted_zone_id = "Z0123456789"
# Credentials: AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY / AWS_SESSION_TOKEN
```

Grant only record-management access to the selected zone. Route53 preserves other TXT values
at the same owner name while adding and removing the ACME value.
Provider names from acme.sh are normalized as well: `dns_cf`, `dns_ali`, `dns_tencent`
(`dns_dp`), and `dns_aws`.

The webhook provider sends a JSON POST to `webhook_url`:

```json
{
  "action": "present",
  "record_type": "TXT",
  "record_name": "_acme-challenge.example.com",
  "value": "challenge-value",
  "ttl": 120
}
```

It sends `action=cleanup` after validation. Both calls must return 2xx. Configure Bearer
authentication with `MIRRORPROXY_ACME_DNS_WEBHOOK_BEARER_TOKEN`.

## Renewal and certificate consumption

MirrorProxy checks on startup and every `check_interval_hours`, renewing inside the
`renew_before_days` window. Super administrators can edit configuration, inspect status, and
queue issuance under Advanced settings. Use the Let's Encrypt staging directory while testing
to avoid production rate limits.

Configure the TLS frontend to consume:

- `<storage_directory>/fullchain.pem`
- `<storage_directory>/privkey.pem`

In reverse-proxy mode, MirrorProxy updates the files atomically. Nginx, Apache, and similar frontends still need a
systemd path unit, container orchestration, or configuration management to reload after file
changes. Caddy deployments should normally keep Caddy's native certificate automation.
Native HTTPS mode reloads renewed certificates itself.

[简体中文](ACME-Certificates-zh-CN)

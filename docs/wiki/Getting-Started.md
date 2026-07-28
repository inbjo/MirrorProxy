# Getting Started

This page starts a login-capable proxy on one host. Read [Deployment](Deployment)
and [ACME Certificates](ACME-Certificates) before a production TLS cutover.

## Docker Compose

Create `compose.yml`:

```yaml
services:
  mirrorproxy:
    image: kudang/mirrorproxy:latest
    restart: unless-stopped
    ports: ["127.0.0.1:3000:3000"]
    environment:
      MIRRORPROXY_ADMIN_PASSWORD: "replace-with-a-long-unique-password"
    volumes: ["mirrorproxy-data:/data"]
volumes:
  mirrorproxy-data:
```

```bash
docker compose up -d
curl -f http://127.0.0.1:3000/healthz
docker compose logs mirrorproxy
```

Open `/admin` and sign in as the initial administrator. Without
`MIRRORPROXY_ADMIN_PASSWORD`, a random password appears only in the first
startup log; save it and change it immediately. Keep `/data`: it contains
SQLite, cache, writable GeoIP data, and admin-stored ACME secrets.

## Binary or source

The server archive contains `mirrorproxy-server`, a configuration example, and
GeoIP data. Run it from persistent storage:

```bash
cp config.example.toml config.toml
./mirrorproxy-server --config ./config.toml serve
```

For a source checkout, fetch pinned GeoIP data first:

```bash
bash scripts/fetch-geoip.sh
cargo run -p mirrorproxy-server -- --config config.example.toml serve
```

The example listens on `127.0.0.1:3000`. Do not expose the admin endpoint until
authentication, trusted proxies, and TLS are intentionally configured.

## Next steps

1. Check required targets under **Source health**; `/api/sources` is the live catalog.
2. Configure enabled adapters, upstreams, rate limits, cache, and outbound proxy.
3. Put the service behind a trusted reverse proxy or enable native ACME HTTPS.
4. Run `mirrorproxy list`, then use `mirrorproxy set npm --base-url https://your-host`.

[简体中文](Getting-Started-zh-CN) · [Deployment](Deployment) · [Client](Client)

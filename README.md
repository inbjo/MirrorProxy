# MirrorProxy

MirrorProxy is a self-hosted mirror proxy platform written in Rust. The current working slice supports GitHub absolute URL proxying, Composer/Packagist metadata proxying, public Docker/OCI registry pull-through routing, npm registry proxying, Go module proxying, Cargo sparse registry proxying, and PyPI Simple API proxying, with a React + Vite + Tailwind web console embedded into the Rust binary.

The project is intentionally adapter-based: Docker/OCI, npm, PyPI, Cargo, Go modules, operating system mirrors, and other ecosystems can be added behind the same proxy core.

## Features

- Embedded web console at `/`
- Health endpoint at `/healthz`
- Runtime public config endpoint at `/api/config`
- GitHub proxy for repository pages, raw files, release assets, archives, and Composer GitHub dist URLs
- Composer proxy at `/composer`
- Docker/OCI proxy at `/v2/*` for Docker Hub, GHCR, Quay, and Kubernetes public images
- npm/yarn/pnpm proxy at `/npm`
- Go module proxy at `/goproxy`
- Cargo sparse registry proxy at `/crates-index`
- pip/PyPI proxy at `/pypi/simple`
- Streamed upstream responses with hop-by-hop header filtering
- Safe defaults that reject unsupported absolute proxy targets

## Quick Start

```bash
cargo run -- --config config.example.toml
```

Open:

```text
http://127.0.0.1:3000
```

Check health:

```bash
curl http://127.0.0.1:3000/healthz
```

## GitHub Proxy

MirrorProxy accepts supported GitHub absolute URLs under your own domain:

```text
http://127.0.0.1:3000/https://github.com/inbjo/Conductor
http://127.0.0.1:3000/https://github.com/inbjo/Conductor/releases/download/nightly/conductor-client-linux-amd64.deb
```

Allowed GitHub-related hosts in this slice:

- `github.com`
- `api.github.com`
- `raw.githubusercontent.com`
- `objects.githubusercontent.com`
- `codeload.github.com`

## Composer Proxy

Configure Composer to use MirrorProxy:

```bash
composer config repo.packagist composer http://127.0.0.1:3000/composer
composer require monolog/monolog
```

MirrorProxy proxies Packagist metadata and rewrites common GitHub/Packagist download URLs back through your MirrorProxy public base URL.

## Docker / OCI Proxy

Use your MirrorProxy host as the Docker registry:

```bash
docker pull 127.0.0.1:3000/nginx
docker pull 127.0.0.1:3000/user/image
docker pull 127.0.0.1:3000/ghcr.io/user/image
docker pull 127.0.0.1:3000/quay.io/org/image
docker pull 127.0.0.1:3000/registry.k8s.io/pause:3.8
```

Mapping rules:

- `name` maps to Docker Hub `library/name`
- `user/image` maps to Docker Hub `user/image`
- `ghcr.io/user/image` maps to GHCR
- `quay.io/org/image` maps to Quay
- `registry.k8s.io/name` maps to the Kubernetes registry

The first implementation handles public pull-through requests and upstream Bearer token challenges. Private registry credentials are intentionally left for a later adapter extension.

## npm / yarn / pnpm Proxy

Configure your package manager to use MirrorProxy:

```bash
npm config set registry http://127.0.0.1:3000/npm
npm install react

yarn config set npmRegistryServer http://127.0.0.1:3000/npm
yarn add react

pnpm config set registry http://127.0.0.1:3000/npm
pnpm add react
```

MirrorProxy proxies npm package metadata and rewrites `dist.tarball` URLs to keep tarball downloads flowing through `/npm`.

## Go Module Proxy

Use MirrorProxy as `GOPROXY`:

```bash
go env -w GOPROXY=http://127.0.0.1:3000/goproxy,direct
go list -m github.com/gin-gonic/gin@latest
```

The Go adapter forwards GOPROXY protocol paths such as `@v/list`, `.info`, `.mod`, and `.zip` to `proxy.golang.org`.

## Rust Crates Proxy

Configure Cargo to use MirrorProxy as a sparse registry mirror:

```toml
[source.crates-io]
replace-with = "mirrorproxy"

[source.mirrorproxy]
registry = "sparse+http://127.0.0.1:3000/crates-index/"
```

Then fetch dependencies:

```bash
cargo fetch
```

MirrorProxy serves a local sparse `config.json` and proxies crate downloads through `/crates/api/v1/crates/{crate}/{version}/download`.

## pip / PyPI Proxy

Configure pip to use MirrorProxy:

```bash
pip config set global.index-url http://127.0.0.1:3000/pypi/simple/
pip install requests
```

MirrorProxy proxies PyPI Simple API HTML and rewrites files.pythonhosted.org links back through `/pypi/files`.

## Configuration

Inspect or safely update an explicit TOML configuration file from the CLI. `set`
creates a sibling `.bak` backup before atomically replacing the file; use
`--dry-run` to inspect the change first.

```bash
mirrorproxy --config ./config.toml config get public_base_url
mirrorproxy --config ./config.toml config set public_base_url https://mirror.example
mirrorproxy --config ./config.toml config set quota.monthly_gb 100 --dry-run
```

## Local Source CLI

`sources set` writes user-level npm, pip, Cargo, Go, or Composer configuration
without invoking the package-manager executable. Before its first change it
records the complete previous file under `~/.local/state/mirrorproxy/sources/`;
`sources reset` restores that exact file. A non-empty configuration is never
replaced unless `--force` is explicit, and reset similarly refuses a file that
was changed after the command.

```bash
mirrorproxy sources set npm --mirror mirrorproxy --base-url http://127.0.0.1:3000
mirrorproxy sources set cargo --mirror mirrorproxy --base-url http://127.0.0.1:3000
mirrorproxy sources reset npm
```

Use `--config-root /tmp/mirrorproxy-home` for an isolated home directory in
automation or tests. System-level package-manager files and Docker daemon
configuration are deliberately not written by this slice yet; their catalog
entries remain available as generated guidance.

Copy `config.example.toml` and adjust the public URL for your deployment:

```toml
listen_addr = "127.0.0.1:3000"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer", "oci", "npm", "go", "crates", "pypi"]

[upstreams]
github = "https://github.com"
github_raw = "https://raw.githubusercontent.com"
packagist = "https://repo.packagist.org"
docker_hub = "https://registry-1.docker.io"
ghcr = "https://ghcr.io"
quay = "https://quay.io"
kubernetes = "https://registry.k8s.io"
npm = "https://registry.npmjs.org"
go_proxy = "https://proxy.golang.org"
crates_index = "https://index.crates.io"
crates_api = "https://crates.io"
pypi_simple = "https://pypi.org/simple"
pypi_files = "https://files.pythonhosted.org"
```

`public_base_url` is used by the web console and metadata rewriters. Set it to the externally reachable URL, especially when MirrorProxy is behind Nginx, Caddy, Traefik, or another reverse proxy.

Common environment overrides:

```bash
MIRRORPROXY_CONFIG=/etc/mirrorproxy/config.toml
MIRRORPROXY_DB=/var/lib/mirrorproxy/mirrorproxy.sqlite3
MIRRORPROXY_LISTEN_ADDR=0.0.0.0:3000
MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com
MIRRORPROXY_ENABLED_PROXIES=github,composer,oci,npm,go,crates,pypi
MIRRORPROXY_REQUEST_TIMEOUT_SECS=60
MIRRORPROXY_RATE_LIMIT_ENABLED=true
MIRRORPROXY_RATE_LIMIT_REQUESTS_PER_MINUTE=600
```

MirrorProxy validates `public_base_url`, all upstream URLs, enabled proxy names, and timeout values during startup. Invalid configuration fails fast with a field-specific error.

On the first startup, MirrorProxy creates its SQLite database and prints a one-time
random password for the `admin` account in the local startup log. Use it with
`POST /api/admin/login`, then send the returned token as `Authorization: Bearer
<token>` to protected endpoints such as `GET /api/admin/config`. The password is
stored only as an Argon2 hash; keep the startup output private.

`PUT /api/admin/config` accepts a validated full configuration document and
persists it in SQLite with an audit record. Public URL, enabled adapters,
upstreams, quota, and rate-limit settings apply to new requests immediately.
Changing `timeout.request_secs` is persisted but reported as restart-required;
`listen_addr` and `database_path` must be changed in the service configuration
and cannot be changed through this API.

Optional global rate limiting can be enabled with:

```toml
[rate_limit]
enabled = true
requests_per_minute = 600
```

When the limit is exceeded, MirrorProxy returns `429 Too Many Requests` with a `Retry-After` header.

## Traffic Accounting and Monthly Quota

Every proxied response is counted after its body has been streamed to the client;
downloads are never buffered merely for accounting. SQLite keeps daily per-target
request/byte/error totals and an aggregate monthly byte total. Health checks,
the web console, and management APIs are not counted or blocked.

```toml
[quota]
enabled = true
monthly_gb = 500
timezone = "Asia/Taipei" # or "local"
on_exceeded = "stop_proxy" # use "throttle" for HTTP 429 instead
```

Once the sent-body total reaches the monthly limit, new proxy requests receive
`503` (`stop_proxy`) or `429` (`throttle`) while the public and management
surfaces stay available. A new calendar month in the configured timezone starts
with a fresh quota automatically.

## Development

Build the web console:

```bash
cd web
npm ci
npm run build
```

Run Rust tests:

```bash
cargo test
```

Run the full local check:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
```

Run the local smoke test on Windows PowerShell:

```powershell
.\scripts\smoke-local.ps1
```

The smoke test builds the debug binary, starts MirrorProxy on a temporary local port, checks the embedded web UI and key proxy endpoints, then stops the process.

## Static Linux Build

On Linux:

```bash
./build.sh
```

The script builds the web console first, then builds a `x86_64-unknown-linux-musl` release binary.

## Reverse Proxy Deployment

MirrorProxy should usually run behind a TLS reverse proxy. Set `public_base_url` to the external HTTPS URL, not the internal bind address.

Nginx example:

```nginx
server {
    listen 443 ssl http2;
    server_name mirror.example.com;

    client_max_body_size 0;
    proxy_request_buffering off;
    proxy_buffering off;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;
    }
}
```

Caddy example:

```caddyfile
mirror.example.com {
    reverse_proxy 127.0.0.1:3000 {
        flush_interval -1
    }
}
```

For Docker/OCI and large release files, keep request buffering disabled in the reverse proxy so large blobs stream instead of being fully buffered.

## Security Notes

- MirrorProxy is not an open proxy.
- GitHub absolute URL proxying is restricted to a small allowlist of GitHub-related hosts.
- Hop-by-hop headers are filtered.
- Private registry credentials are not implemented in this first slice.

## Roadmap

- OS mirror source adapters
- Optional caching, rate limiting, and richer observability

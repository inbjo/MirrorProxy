# MirrorProxy

MirrorProxy is a self-hosted mirror proxy platform written in Rust. The first working slice supports GitHub absolute URL proxying and Composer/Packagist metadata proxying, with a React + Vite + Tailwind web console embedded into the Rust binary.

The project is intentionally adapter-based: Docker/OCI, npm, PyPI, Cargo, Go modules, operating system mirrors, and other ecosystems can be added behind the same proxy core.

## Features

- Embedded web console at `/`
- Health endpoint at `/healthz`
- Runtime public config endpoint at `/api/config`
- GitHub proxy for repository pages, raw files, release assets, archives, and Composer GitHub dist URLs
- Composer proxy at `/composer`
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

## Configuration

Copy `config.example.toml` and adjust the public URL for your deployment:

```toml
listen_addr = "127.0.0.1:3000"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer"]

[upstreams]
github = "https://github.com"
github_raw = "https://raw.githubusercontent.com"
packagist = "https://repo.packagist.org"
```

`public_base_url` is used by the web console and metadata rewriters. Set it to the externally reachable URL, especially when MirrorProxy is behind Nginx, Caddy, Traefik, or another reverse proxy.

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

## Static Linux Build

On Linux:

```bash
./build.sh
```

The script builds the web console first, then builds a `x86_64-unknown-linux-musl` release binary.

## Security Notes

- MirrorProxy is not an open proxy.
- GitHub absolute URL proxying is restricted to a small allowlist of GitHub-related hosts.
- Hop-by-hop headers are filtered.
- Private registry credentials and authenticated upstream flows are not implemented in this first slice.

## Roadmap

- Docker Hub, GHCR, Quay, and Kubernetes OCI registry proxying
- npm/yarn/pnpm registry proxying
- PyPI simple repository proxying
- Cargo sparse registry proxying
- Go module proxying
- OS mirror source adapters
- Optional caching, rate limiting, and richer observability


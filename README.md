# MirrorProxy

[English](README.md) | [简体中文](README_CN.md)

[![CI](https://img.shields.io/github/actions/workflow/status/inbjo/MirrorProxy/ci.yml?branch=main&style=flat-square&logo=githubactions&logoColor=white&label=CI)](https://github.com/inbjo/MirrorProxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/inbjo/MirrorProxy?style=flat-square&logo=github&label=Release)](https://github.com/inbjo/MirrorProxy/releases/latest)
[![License](https://img.shields.io/github/license/inbjo/MirrorProxy?style=flat-square&label=License)](LICENSE)
[![Docker Pulls](https://img.shields.io/docker/pulls/kudang/mirrorproxy?style=flat-square&logo=docker&logoColor=white&label=Docker%20Pulls)](https://hub.docker.com/r/kudang/mirrorproxy)
[![Clients](https://img.shields.io/badge/clients-Windows%20%7C%20macOS%20%7C%20Linux-2f81f7?style=flat-square)](#install-the-client)
[![GitHub Stars](https://img.shields.io/github/stars/inbjo/MirrorProxy?style=flat-square&logo=github&label=Stars)](https://github.com/inbjo/MirrorProxy/stargazers)

MirrorProxy is a self-hosted mirror proxy platform written in Rust. The `mirrorproxy-server` service and the `mirrorproxy` source-management client are independent binaries. The server embeds the React + Vite + Tailwind web console; the standalone client runs on Windows, macOS, and Linux.

The project uses an adapter-based proxy core and already ships adapters for GitHub, Docker/OCI, Composer, npm, PyPI, Cargo, Go modules, major language repositories, developer-tool distribution services, and operating system mirrors. New ecosystems can reuse the same routing, streaming, security, accounting, quota, and cache infrastructure.

## Features

- Embedded public and authenticated administration web console at `/`
- Health endpoint at `/healthz`
- Runtime public config endpoint at `/api/config`
- Source catalog endpoint at `/api/sources`
- SQLite-backed settings, administrator sessions, audit log, traffic accounting, monthly quota, rate limiting, and bounded disk cache
- Standalone Windows/macOS/Linux client with source editing, exact rollback, and GitHub HTTPS Git URL rewriting
- GitHub proxy for repository pages, raw files, release assets, archives, and Composer GitHub dist URLs
- Composer proxy at `/composer`
- Docker/OCI proxy at `/v2/*` for Docker Hub, GHCR, Quay, and Kubernetes public images
- npm/yarn/pnpm proxy at `/npm`
- Node.js distribution proxy for NVM at `/nvm`
- opam repository proxy at `/opam`
- Go module proxy at `/goproxy`
- Maven Central proxy at `/maven`
- RubyGems proxy at `/rubygems`
- Rustup distribution proxy at `/rustup`
- NuGet v3 proxy at `/nuget/v3/index.json`
- CPAN repository proxy at `/cpan`
- CRAN repository proxy at `/cran`
- Hackage repository proxy at `/hackage`
- Julia package server proxy at `/julia`
- LuaRocks repository proxy at `/luarocks`
- Clojars repository proxy at `/clojars`
- CocoaPods CDN proxy at `/cocoapods`
- Dart / Flutter Pub proxy at `/pub`
- Anaconda / Conda proxy at `/anaconda`
- TeX Live proxy at `/texlive`
- WinGet source proxy at `/winget`
- GNU ELPA proxy at `/elpa`
- Nix binary cache proxy at `/nix`
- GNU Guix substitute cache proxy at `/guix`
- Flatpak OSTree proxy at `/flatpak`
- Homebrew bottle proxy at `/homebrew`
- Allowlisted Linux, BSD, MSYS2, OpenWrt, Termux, ROS, and related operating-system repositories under `/os`
- Cargo sparse registry proxy at `/crates-index`
- pip/PyPI proxy at `/pypi/simple`
- Streamed upstream responses with hop-by-hop header filtering
- Safe defaults that reject unsupported absolute proxy targets

## Quick Start

```bash
cargo run -p mirrorproxy-server --bin mirrorproxy-server -- --config config.example.toml
```

Open:

```text
http://selfhost.com
```

Check health:

```bash
curl http://selfhost.com/healthz
```

## Docker Deployment

The server image runs as non-root UID/GID `10001`, listens on port `3000`, and
stores its SQLite database and optional cache under the `/data` volume.

The repository includes a ready-to-run [compose.yaml](compose.yaml). You can
also save the following as `docker-compose.yaml` in an empty deployment
directory:

```yaml
services:
  mirrorproxy:
    image: ${MIRRORPROXY_IMAGE:-kudang/mirrorproxy:latest}
    container_name: mirrorproxy
    restart: unless-stopped
    ports:
      - "${MIRRORPROXY_PORT:-3000}:3000"
    environment:
      MIRRORPROXY_PUBLIC_BASE_URL: ${MIRRORPROXY_PUBLIC_BASE_URL:-}
      MIRRORPROXY_TRUSTED_PROXIES: ${MIRRORPROXY_TRUSTED_PROXIES:-127.0.0.1,::1}
      MIRRORPROXY_QUOTA_TIMEZONE: ${MIRRORPROXY_QUOTA_TIMEZONE:-local}
      MIRRORPROXY_ADMIN_PASSWORD: ${MIRRORPROXY_ADMIN_PASSWORD:-}
      MIRRORPROXY_MAVEN_FALLBACKS: ${MIRRORPROXY_MAVEN_FALLBACKS-https://jcenter.bintray.com}
      OTEL_EXPORTER_OTLP_ENDPOINT: ${OTEL_EXPORTER_OTLP_ENDPOINT:-}
      OTEL_TRACES_SAMPLER: ${OTEL_TRACES_SAMPLER:-parentbased_traceidratio}
      OTEL_TRACES_SAMPLER_ARG: ${OTEL_TRACES_SAMPLER_ARG:-0.1}
      RUST_LOG: ${RUST_LOG:-mirrorproxy_server=info,tower_http=info}
    volumes:
      - mirrorproxy-data:/data
    healthcheck:
      test: ["CMD", "curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:3000/healthz"]
      interval: 30s
      timeout: 5s
      start_period: 10s
      retries: 3

volumes:
  mirrorproxy-data:
```

Optionally set a fixed external URL, host port, initial administrator password,
and trusted reverse-proxy peers in a `.env` file before startup. When the
public URL is unset or empty, MirrorProxy derives it from the browser request
address. Forwarded headers are accepted only from `MIRRORPROXY_TRUSTED_PROXIES`:

```dotenv
MIRRORPROXY_PORT=53000
MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com
# Keep the defaults for a host-local Nginx/Caddy; use the reverse proxy's
# Docker-network peer IP or CIDR when it runs in another container.
MIRRORPROXY_TRUSTED_PROXIES=127.0.0.1,::1
# Optional: uncomment to set the initial admin password yourself.
# MIRRORPROXY_ADMIN_PASSWORD=replace-with-a-strong-password
# Optional: comma-separated Maven fallback repositories; empty disables fallback.
# MIRRORPROXY_MAVEN_FALLBACKS=https://jcenter.bintray.com
# Optional: enable OTLP/gRPC trace export and sample 10% of root traces.
# OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
```

```bash
MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com docker compose up -d
docker compose logs mirrorproxy
curl http://127.0.0.1:3000/healthz
```

When the SQLite database is initialized for the first time, an unset or empty
`MIRRORPROXY_ADMIN_PASSWORD` makes MirrorProxy generate a random `admin`
password and print it prominently in the startup log. Set the variable to a
non-empty value to use that password instead; manually configured passwords are
not printed. The variable does not reset the password in an existing database.
Keep the named `mirrorproxy-data` volume when upgrading. To run without
Compose:

```bash
docker run -d --name mirrorproxy --restart unless-stopped \
  -p 3000:3000 \
  -e MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com \
  -e MIRRORPROXY_TRUSTED_PROXIES=127.0.0.1,::1 \
  -e MIRRORPROXY_ADMIN_PASSWORD='replace-with-a-strong-password' \
  -v mirrorproxy-data:/data \
  kudang/mirrorproxy:latest
```

Tagged multi-architecture images are published with an SPDX SBOM, a
BuildKit `mode=max` provenance attestation, and a keyless Sigstore signature
issued to the GitHub Actions workflow. Verify a released image by immutable
digest:

```bash
IMAGE=kudang/mirrorproxy:1.0.2
DIGEST="$(docker buildx imagetools inspect "$IMAGE" --format '{{json .Manifest}}' | jq -r '.digest')"
cosign verify \
  --certificate-identity-regexp '^https://github\.com/inbjo/MirrorProxy/\.github/workflows/docker\.yml@refs/tags/v[0-9].*$' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "kudang/mirrorproxy@${DIGEST}"
```

The named volume is recommended. If you replace it with a host bind mount such
as `/srv/mirrorproxy/data:/data`, that directory must be writable by container
UID/GID `10001:10001`:

```bash
sudo install -d -o 10001 -g 10001 -m 0750 /srv/mirrorproxy/data
sudo install -d -o 10001 -g 10001 -m 0750 /srv/mirrorproxy/data/cache
```

On an SELinux-enforcing host, append `:Z` to the bind mount. Otherwise SQLite
may fail at startup with `code: 14 unable to open database file`.

Environment variables such as `MIRRORPROXY_ENABLED_PROXIES`, quota, cache, and
rate-limit settings work in the container. For a complete TOML configuration,
mount it read-only and set its path explicitly:

```bash
docker run -d --name mirrorproxy --restart unless-stopped \
  -p 3000:3000 \
  -e MIRRORPROXY_CONFIG=/etc/mirrorproxy/config.toml \
  -e MIRRORPROXY_LISTEN_ADDR=0.0.0.0:3000 \
  -e MIRRORPROXY_DB=/data/mirrorproxy.sqlite3 \
  -v mirrorproxy-data:/data \
  -v "$PWD/config.toml:/etc/mirrorproxy/config.toml:ro" \
  kudang/mirrorproxy:latest
```

Build the current source as a local `linux/amd64` image:

```bash
./scripts/docker-build.sh
```

If Docker Hub is slow or unavailable, route base-image pulls through a
MirrorProxy instance:

```bash
MIRRORPROXY_DOCKER_BASE_REGISTRY=sina.dev/library ./scripts/docker-build.sh
```

For a local multi-platform build, register ARM64 emulation once (GitHub Actions
does this automatically). The same image can be pulled through MirrorProxy when
Docker Hub is unavailable:

```bash
docker run --privileged --rm sina.dev/tonistiigi/binfmt --install arm64
```

Then publish `linux/amd64` and `linux/arm64` manifests after `docker login`:

```bash
./scripts/docker-build.sh --push --image <dockerhub-user>/mirrorproxy
```

The `Docker` GitHub Actions workflow performs the same multi-platform publish.
Set repository variable `DOCKERHUB_USERNAME` and repository secret
`DOCKERHUB_TOKEN`; pushing a `v*` tag publishes the semantic-version and
`latest` tags, while workflow dispatch supports an explicit version.

## GitHub Proxy

MirrorProxy accepts supported GitHub absolute URLs under your own domain:

```text
http://selfhost.com/https://github.com/inbjo/Conductor
http://selfhost.com/https://github.com/inbjo/Conductor/releases/download/nightly/conductor-client-linux-amd64.deb
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
composer config repo.packagist composer http://selfhost.com/composer
composer require monolog/monolog
```

MirrorProxy proxies Packagist metadata and rewrites common GitHub/Packagist download URLs back through your MirrorProxy public base URL.

## Docker / OCI Proxy

Use your MirrorProxy host as the Docker registry:

```bash
docker pull selfhost.com/nginx
docker pull selfhost.com/user/image
docker pull selfhost.com/ghcr.io/user/image
docker pull selfhost.com/quay.io/org/image
docker pull selfhost.com/registry.k8s.io/pause:3.8
```

Mapping rules:

- `name` maps to Docker Hub `library/name`
- `user/image` maps to Docker Hub `user/image`
- `ghcr.io/user/image` maps to GHCR
- `quay.io/org/image` maps to Quay
- `registry.k8s.io/name` maps to the Kubernetes registry

The proxy handles public pull-through requests and upstream Bearer token challenges. Private upstream credentials can be configured as described in [Security](#security).

## npm / yarn / pnpm Proxy

Configure your package manager to use MirrorProxy:

```bash
npm config set registry http://selfhost.com/npm
npm install react

yarn config set npmRegistryServer http://selfhost.com/npm
yarn add react

pnpm config set registry http://selfhost.com/npm
pnpm add react
```

MirrorProxy proxies npm package metadata and rewrites `dist.tarball` URLs to keep tarball downloads flowing through `/npm`.

## Go Module Proxy

Use MirrorProxy as `GOPROXY`:

```bash
go env -w GOPROXY=http://selfhost.com/goproxy,direct
go list -m github.com/gin-gonic/gin@latest
```

The Go adapter forwards GOPROXY protocol paths such as `@v/list`, `.info`, `.mod`, and `.zip` to `proxy.golang.org`.

## Maven Central Proxy

Configure Maven's user settings to mirror Central through MirrorProxy:

```xml
<settings>
  <mirrors>
    <mirror>
      <id>mirrorproxy</id>
      <url>http://selfhost.com/maven/</url>
      <mirrorOf>central</mirrorOf>
    </mirror>
  </mirrors>
</settings>
```

Save this under `~/.m2/settings.xml`, or let the CLI write it with rollback protection:

```bash
mirrorproxy set maven --mirror mirrorproxy --base-url http://selfhost.com
mvn dependency:resolve
```

The Maven adapter streams Maven2 repository paths, including POMs, metadata,
artifacts, checksums, and signatures. A client still configures only the single
`/maven/` endpoint; MirrorProxy tries the primary `upstreams.maven` repository
first and advances through `upstreams.maven_fallbacks` only for an explicit
HTTP 404. Authentication failures, rate limits, server errors, and transport
errors are not hidden by fallback. The default order is Maven Central followed
by the read-only JCenter repository. Set an empty list to disable fallback:

```toml
[upstreams]
maven = "https://repo.maven.apache.org/maven2"
maven_fallbacks = ["https://jcenter.bintray.com"]
```

For containers, set the ordered fallback list as comma-separated URLs with
`MIRRORPROXY_MAVEN_FALLBACKS`; an empty value disables fallback.

## RubyGems Proxy

Configure RubyGems to use MirrorProxy as its source:

```yaml
---
:sources:
- http://selfhost.com/rubygems/
```

Save this under `~/.gemrc`, or let the CLI write it with rollback protection:

```bash
mirrorproxy set rubygems --mirror mirrorproxy --base-url http://selfhost.com
gem install rake
```

The RubyGems adapter streams the compact index (`/versions`, `/info/*`), legacy indexes, API responses, and `.gem` downloads while preserving Range and ETag headers used by Bundler.

## NuGet v3 Proxy

Configure NuGet to use MirrorProxy as a v3 package source:

```xml
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="mirrorproxy" value="http://selfhost.com/nuget/v3/index.json" protocolVersion="3" />
  </packageSources>
</configuration>
```

Save it to `%APPDATA%\NuGet\NuGet.Config` on Windows or `~/.nuget/NuGet/NuGet.Config` on Linux/macOS. The CLI writes the same file with rollback protection:

```bash
mirrorproxy set nuget --mirror mirrorproxy --base-url http://selfhost.com
dotnet restore
```

The adapter rewrites NuGet v3 service-index resource URLs to MirrorProxy, then streams flat-container packages, registration metadata, search results, and package downloads through `/nuget`.

## CPAN Proxy

Use the CPAN static-mirror endpoint with `cpanm`:

```bash
cpanm --mirror http://selfhost.com/cpan/ --mirror-only Moo
```

The CLI can save a rollback-protected CPAN mirror list to `~/.cpan/CPAN/MyConfig.pm`:

```bash
mirrorproxy set cpan --mirror mirrorproxy --base-url http://selfhost.com
```

The adapter streams CPAN indexes and distributions such as `modules/02packages.details.txt.gz` and `authors/id/...` while rejecting traversal paths.

## CRAN Proxy

Set R's CRAN repository to MirrorProxy:

```r
options(repos = c(CRAN = "http://selfhost.com/cran/"))
install.packages("digest")
```

`mirrorproxy set cran --mirror mirrorproxy --base-url http://selfhost.com` writes a rollback-protected `~/.Rprofile`. Source indexes, archives, and platform binary paths are streamed through `/cran`.

## Hackage Proxy

Configure Cabal's user repository to use MirrorProxy:

```yaml
repository hackage.haskell.org
  url: http://selfhost.com/hackage/
  secure: True
```

`mirrorproxy set hackage --mirror mirrorproxy --base-url http://selfhost.com` writes and can restore `~/.cabal/config`. The adapter streams the package index and package tarballs while rejecting traversal paths.

## Rustup Distribution Proxy

Point Rustup's distribution and self-update roots at MirrorProxy:

```bash
export RUSTUP_DIST_SERVER=http://selfhost.com/rustup
export RUSTUP_UPDATE_ROOT=http://selfhost.com/rustup/rustup
rustup update stable
```

Channel manifests, components, checksums, and signatures are streamed from the official Rust distribution service through normalized paths.

## Julia Package Server Proxy

Set `JULIA_PKG_SERVER=http://selfhost.com/julia` before running Julia's package manager. Registry and package-server protocol paths are forwarded to the configured Julia package server.

## LuaRocks Proxy

On Linux, install from MirrorProxy with `luarocks install --server=http://selfhost.com/luarocks/ <module>`.

## NVM / Node.js Distribution Proxy

On Linux, point NVM at the proxied Node.js release files before installing a version:

```bash
export NVM_NODEJS_ORG_MIRROR=http://selfhost.com/nvm/
nvm install --lts
```

## opam Proxy

On Linux, configure `opam repository set-url default http://selfhost.com/opam/`.

## Clojars Proxy

Configure the Clojure CLI user `deps.edn` to route Clojars through MirrorProxy:

```clojure
{:mvn/repos {"clojars" {:url "http://selfhost.com/clojars/"}}}
```

`mirrorproxy set clojars --mirror mirrorproxy --base-url http://selfhost.com` writes and can restore `~/.clojure/deps.edn`. The adapter streams Clojars POMs, metadata, and JARs with normalized repository paths only.

## CocoaPods CDN Proxy

Use `source 'http://selfhost.com/cocoapods/'` in a Podfile when routing the CocoaPods CDN through MirrorProxy. CDN index shards and podspec files are accepted only through normalized paths.

## WinGet Source Proxy

Add the MirrorProxy endpoint as a pre-indexed WinGet source:

```powershell
winget source add --name mirrorproxy --arg http://selfhost.com/winget/ --type Microsoft.PreIndexed.Package --accept-source-agreements
```

The adapter streams the official WinGet source indexes and package metadata through `/winget`.

## Pub / Flutter Proxy

```bash
PUB_HOSTED_URL=http://selfhost.com/pub/ flutter pub get
```

Pub package metadata and official archives stay on MirrorProxy; archive URLs are rewritten only for the official Google Cloud Storage host.

## Anaconda / Conda Proxy

Use MirrorProxy as a Conda channel base, for example `http://selfhost.com/anaconda/main`. The adapter streams `repodata.json` and package artifacts while rejecting traversal paths.

## TeX Live Proxy

Use `http://selfhost.com/texlive/` as a TeX Live network installer mirror. The adapter streams `tlpkg/texlive.tlpdb` and archive files using normalized paths only.

## GNU ELPA Proxy

Use `http://selfhost.com/elpa/` as an Emacs package archive URL. The adapter streams `archive-contents` and package archives through normalized paths only.

## Nix Binary Cache Proxy

Use `http://selfhost.com/nix/` as a Nix substituter. `.narinfo` signatures and relative cache URLs remain unchanged, so Nix continues to verify cache signatures normally.

## GNU Guix Substitute Cache Proxy

Use `http://selfhost.com/guix/` as a Guix substitute URL, for example `guix build --substitute-urls=http://selfhost.com/guix/ hello`. Narinfo signatures and substitute payloads are streamed unchanged, so Guix continues to verify authorized cache keys.

## Flatpak OSTree Proxy

Use `http://selfhost.com/flatpak/` as a Flatpak remote URL. OSTree summaries and GPG signatures are streamed unchanged, preserving client-side repository verification.

## Homebrew Bottle Proxy

Set `HOMEBREW_BOTTLE_DOMAIN=http://selfhost.com/homebrew` before running `brew install`. The default upstream is Homebrew's public GHCR OCI bottle repository; manifest and blob requests are streamed unchanged, including Range requests.

## OS Static Repository Proxy

Use fixed target paths such as `http://selfhost.com/os/debian/`, `/os/ubuntu/`, `/os/fedora/`, `/os/archlinux/`, `/os/opensuse/`, `/os/void/`, `/os/gentoo/`, `/os/freebsd/`, `/os/alpine/`, `/os/openwrt/`, `/os/termux/`, `/os/kali/`, `/os/rocky/`, `/os/alma/`, `/os/manjaro/`, `/os/msys2/`, `/os/raspios/`, `/os/armbian/`, `/os/openeuler/`, `/os/anolis/`, `/os/deepin/`, `/os/linuxmint/`, `/os/solus/`, `/os/trisquel/`, `/os/linuxlite/`, `/os/ros/`, `/os/netbsd/`, or `/os/openbsd/`. Only these targets are accepted; each has a separately configurable upstream. Additional OS targets use the `[upstreams.additional_os]` TOML map. Linux Mint defaults to the Kernel.org HTTPS package mirror; the ROS target proxies the ROS 2 Ubuntu APT repository; Solus uses `/os/solus/polaris/eopkg-index.xml.xz`.

## Rust Crates Proxy

Configure Cargo to use MirrorProxy as a sparse registry mirror:

```toml
[source.crates-io]
replace-with = "mirrorproxy"

[source.mirrorproxy]
registry = "sparse+http://selfhost.com/crates-index/"
```

Then fetch dependencies:

```bash
cargo fetch
```

MirrorProxy serves a local sparse `config.json` and proxies crate downloads through `/crates/api/v1/crates/{crate}/{version}/download`.

## pip / PyPI Proxy

Configure pip to use MirrorProxy:

```bash
pip config set global.index-url http://selfhost.com/pypi/simple/
pip install requests
```

MirrorProxy proxies PyPI Simple API HTML and rewrites files.pythonhosted.org links back through `/pypi/files`.

## Configuration

Inspect or safely update an explicit TOML configuration file from the CLI. `set`
creates a sibling `.bak` backup before atomically replacing the file; use
`--dry-run` to inspect the change first.

```bash
mirrorproxy-server --config ./config.toml config get public_base_url
mirrorproxy-server --config ./config.toml config set public_base_url https://mirror.example
mirrorproxy-server --config ./config.toml config set quota.monthly_gb 100 --dry-run
```

## Install the Client

Linux and macOS share one installer. It detects the operating system and CPU
architecture, downloads the matching asset from the latest stable GitHub
Release, verifies its SHA-256 checksum, and installs `mirrorproxy` under
`/usr/local/bin` (using `sudo` only when required).
The host must provide `curl`, `tar`, `gzip`, `install`, and either `sha256sum`
or `shasum`; the installer reports a missing prerequisite before downloading.

Install it with:

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
```

To accelerate both the installer and release asset through a MirrorProxy
instance, pass `--mirror`:

```bash
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- --mirror https://sina.dev
```

Windows uses a separate PowerShell installer. Windows may block remote scripts
by default, so allow them for the current PowerShell process only, then run the
installer:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
```

Accelerated Windows installation:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
$env:MIRRORPROXY_DOWNLOAD_MIRROR='https://sina.dev'
irm https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
```

The PowerShell installer places `mirrorproxy.exe` under the current user's
local programs directory and adds it to the user `PATH`. Both installers accept
`MIRRORPROXY_VERSION` for a specific tag and `MIRRORPROXY_INSTALL_DIR` for a
custom location. The `latest` route intentionally selects a stable release, not
the rolling `nightly` prerelease; it becomes available after the first `v*` tag
is published.

## Local Source CLI

`mirrorproxy` is a standalone client without Axum, the database, or the embedded
web console. GitHub Releases provide Windows, macOS, and Linux builds; the
independent `mirrorproxy-server` artifact is released for Linux.

Source commands use the chsrc-style top-level `set`, `get`, `reset`, `list`,
and `mirrors` forms. The legacy `sources` namespace remains accepted for
backward compatibility.

```bash
mirrorproxy set bun --mirror mirrorproxy --base-url https://sina.dev --scope user
```

`set` writes supported user-level package-manager configuration, including npm,
pip, Cargo, GitHub HTTPS Git URL rewriting, Go, Composer, Maven, RubyGems,
NuGet, CPAN, CRAN, Hackage, Clojars, Anaconda, LuaRocks, Homebrew bottles, and
Nix binary caches, without invoking the package-manager executable. Before its
first change it
records the complete previous file in the platform-native user state directory
(`~/.local/state/mirrorproxy/sources/` by default on Linux); `reset` restores
that exact file. A non-empty configuration is never
replaced unless `--force` is explicit, and reset similarly refuses a file that
was changed after the command.

```bash
mirrorproxy set npm --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set cargo --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set github --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set lua --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set homebrew --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set nix --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy reset npm
mirrorproxy reset github
mirrorproxy reset lua
mirrorproxy reset homebrew
mirrorproxy reset nix
```

`set github` appends a `url.<MirrorProxy>.insteadOf` rule to the user's
`~/.gitconfig`, so Git clones and package-manager Git dependencies that use
`https://github.com/` automatically go through MirrorProxy. Existing Git config
is preserved without requiring `--force`, and `reset github` restores the exact
file recorded by the rollback. SSH-form GitHub URLs are not rewritten.

Use `--config-root /tmp/mirrorproxy-config` for an isolated configuration root in
automation or tests. APT, DNF/YUM, pacman, and Docker additionally support
explicit `--scope system`: MirrorProxy only manages the relevant configuration
file and keeps a rollback record under `/var/lib/mirrorproxy/sources/` (or the
supplied root). APT requires a release codename; system writes normally require
root access and are enabled only on Linux hosts.

```bash
mirrorproxy set apt --mirror tuna --scope system --distribution jammy
mirrorproxy set apt --mirror mirrorproxy --base-url https://mirror.example --scope system --distribution debian/bookworm
mirrorproxy reset apt --scope system
mirrorproxy set alpine --mirror mirrorproxy --base-url https://mirror.example --scope system --distribution v3.21
mirrorproxy reset alpine --scope system
mirrorproxy set xbps --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy reset xbps --scope system
mirrorproxy set zypper --mirror mirrorproxy --base-url https://mirror.example --scope system --distribution distribution/leap/15.6
mirrorproxy reset zypper --scope system
mirrorproxy set gentoo --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy reset gentoo --scope system
mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy reset docker --scope system
```

Docker writes `/etc/docker/daemon.json` with `registry-mirrors`. It never
replaces an existing daemon configuration without `--force`, and reset restores
the exact previous file. Restart Docker after applying the configuration.

Copy `config.example.toml` and adjust the public URL for your deployment:

```toml
listen_addr = "0.0.0.0:3000"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer", "oci", "npm", "nvm", "opam", "go", "maven", "rubygems", "rustup", "nuget", "cpan", "cran", "hackage", "julia", "luarocks", "clojars", "cocoapods", "pub", "anaconda", "texlive", "winget", "elpa", "nix", "guix", "flatpak", "homebrew", "os", "crates", "pypi"]

[upstreams]
github = "https://github.com"
github_raw = "https://raw.githubusercontent.com"
packagist = "https://repo.packagist.org"
docker_hub = "https://registry-1.docker.io"
ghcr = "https://ghcr.io"
quay = "https://quay.io"
kubernetes = "https://registry.k8s.io"
npm = "https://registry.npmjs.org"
nvm = "https://nodejs.org/dist"
opam = "https://opam.ocaml.org"
go_proxy = "https://proxy.golang.org"
maven = "https://repo.maven.apache.org/maven2"
# Tried in order only when the primary repository returns HTTP 404.
maven_fallbacks = ["https://jcenter.bintray.com"]
rubygems = "https://rubygems.org"
rustup = "https://static.rust-lang.org"
nuget = "https://api.nuget.org"
cpan = "https://cpan.metacpan.org"
cran = "https://cloud.r-project.org"
hackage = "https://hackage.haskell.org"
julia = "https://pkg.julialang.org"
luarocks = "https://luarocks.org"
clojars = "https://repo.clojars.org"
cocoapods = "https://cdn.cocoapods.org"
pub_repository = "https://pub.dev"
anaconda = "https://repo.anaconda.com/pkgs"
texlive = "https://mirrors.ctan.org/systems/texlive/tlnet"
winget = "https://cdn.winget.microsoft.com"
elpa = "https://elpa.gnu.org/packages"
nix = "https://cache.nixos.org"
guix = "https://ci.guix.gnu.org"
flatpak = "https://dl.flathub.org/repo"
homebrew = "https://ghcr.io/v2/homebrew/core"
alpine = "https://dl-cdn.alpinelinux.org/alpine"
openwrt = "https://downloads.openwrt.org"
termux = "https://packages.termux.dev/apt/termux-main"
debian = "https://deb.debian.org/debian"
ubuntu = "https://archive.ubuntu.com/ubuntu"
fedora = "https://download.fedoraproject.org/pub/fedora/linux"
archlinux = "https://geo.mirror.pkgbuild.com"
opensuse = "https://download.opensuse.org"
void = "https://repo-default.voidlinux.org"
gentoo = "https://distfiles.gentoo.org"
freebsd = "https://pkg.freebsd.org"
crates_index = "https://index.crates.io"
crates_api = "https://crates.io"
pypi_simple = "https://pypi.org/simple"
pypi_files = "https://files.pythonhosted.org"
```

`public_base_url` is used by the web console and metadata rewriters. When it is unset or empty, MirrorProxy derives it from each browser request's host and scheme (including `X-Forwarded-Host` and `X-Forwarded-Proto`). Set it to an external URL to use one fixed address instead.

Common environment overrides:

```bash
MIRRORPROXY_CONFIG=/etc/mirrorproxy/config.toml
MIRRORPROXY_DB=/var/lib/mirrorproxy/mirrorproxy.sqlite3
MIRRORPROXY_LISTEN_ADDR=0.0.0.0:3000
MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com
MIRRORPROXY_TRUSTED_PROXIES=127.0.0.1,::1,172.18.0.0/16
MIRRORPROXY_ENABLED_PROXIES=github,composer,oci,npm,nvm,opam,go,maven,rubygems,rustup,nuget,cpan,cran,hackage,julia,luarocks,clojars,cocoapods,pub,anaconda,texlive,winget,elpa,nix,guix,flatpak,homebrew,os,crates,pypi
MIRRORPROXY_REQUEST_TIMEOUT_SECS=60
MIRRORPROXY_RATE_LIMIT_ENABLED=true
MIRRORPROXY_RATE_LIMIT_REQUESTS_PER_MINUTE=600
MIRRORPROXY_CACHE_ENABLED=true
MIRRORPROXY_CACHE_DIRECTORY=/var/cache/mirrorproxy
MIRRORPROXY_CACHE_MAX_ENTRY_MB=8
```

MirrorProxy validates a non-empty `public_base_url`, all upstream URLs, enabled proxy names, and timeout values during startup. Invalid configuration fails fast with a field-specific error.

Optional disk caching is disabled by default. When enabled, it stores only successful public GET responses with an explicit `Content-Length` no larger than `cache.max_entry_mb`; `cache.max_total_mb` bounds disk usage and evicts least-recently-used entries. Requests carrying `Authorization`, `Cookie`, or `Range` bypass the cache. Large or unknown-length responses stay streamed and are never buffered for caching.

On the first startup, MirrorProxy creates its SQLite database and prints a one-time
random password for the `admin` account in the local startup log. When
`MIRRORPROXY_ADMIN_PASSWORD` has a value, it uses that value instead. Use the
password with `POST /api/admin/login`, then send the returned token as
`Authorization: Bearer <token>` to protected endpoints such as `GET
/api/admin/config`. The password is stored only as an Argon2 hash; keep the
startup output private.

`PUT /api/admin/config` accepts a validated full configuration document and
persists it in SQLite with an audit record. Public URL, enabled adapters,
upstreams, quota, and rate-limit settings apply to new requests immediately.
Changing `timeout.request_secs` is persisted but reported as restart-required;
`listen_addr` and `database_path` must be changed in the service configuration
and cannot be changed through this API.

`GET /api/admin/stats` returns the current configured-month summary, quota
remaining bytes, per-day/per-target traffic points, and the ten busiest proxy
targets. It requires the same administrator Bearer token.

`POST /api/admin/password` accepts `current_password` and a new password of at
least 12 characters. A successful change revokes every administrator session,
including the one that made the request.

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

## Observability

Prometheus metrics are available at `GET /metrics`. The endpoint exports
normalized route, response status, request duration, delivered proxy bytes,
stream errors, quota/rate-limit rejections, and build information. Labels never
contain raw URLs, query strings, request headers, credentials, or tokens.

Example Prometheus scrape configuration:

```yaml
scrape_configs:
  - job_name: mirrorproxy
    static_configs:
      - targets: ["mirrorproxy:3000"]
```

The ready-to-load alert rules in
[`deploy/prometheus/alerts.yml`](deploy/prometheus/alerts.yml) cover sustained
5xx responses, proxy stream errors, and quota rejections. Tune their thresholds
for the expected traffic volume before production use.

Set `OTEL_EXPORTER_OTLP_ENDPOINT` (or the trace-specific
`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`) to enable OTLP/gRPC trace export. Leaving
both variables empty disables exporting. The standard `OTEL_TRACES_SAMPLER` and
`OTEL_TRACES_SAMPLER_ARG` variables control sampling; Compose defaults to a 10%
parent-based ratio when export is enabled:

```dotenv
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
OTEL_TRACES_SAMPLER=parentbased_traceidratio
OTEL_TRACES_SAMPLER_ARG=0.1
```

MirrorProxy extracts incoming W3C `traceparent`/`tracestate` context and injects
the active context into upstream requests. Request spans use normalized route
names and deliberately exclude raw paths, query strings, `Authorization`,
cookies, and baggage values.

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

GitHub Actions runs formatting, clippy, Rust tests, the frontend production build, browser end-to-end tests, and native client tests on Linux, Windows, and macOS. Tagging `v*` builds three-platform client artifacts plus Linux server artifacts and publishes a GitHub release with per-artifact checksums and `SHA256SUMS`.

For a local real-client protocol check (Git, npm/yarn/pnpm, Go, Cargo, pip, CPAN cpanm, RubyGems, Maven, NuGet, CRAN, Cabal/Hackage, LuaRocks, and Composer, with optional Docker), run:

```bash
./scripts/smoke-clients.sh
```

The script starts a temporary local server, uses temporary client homes/caches, and removes them on exit.
CI additionally enables `MIRRORPROXY_SMOKE_NATIVE_EXTENDED=1`: the standalone
CLI writes and precisely rolls back LuaRocks, Nix, and Homebrew configuration;
their native clients then access MirrorProxy, and `rustup check` verifies the
Rustup distribution/update endpoints. This mode requires `brew`, `nix`,
`rustup`, and `luarocks`.

### Verified platforms and clients

The following matrix records checks that were actually run; a plain HTTP GET/HEAD
probe is not presented as a native-client test. The 2026-07-16 OS checks pulled
their container images and package repositories through `sina.dev`. Images that
can run the Linux client also exercised the one-line installer, source changes,
index refreshes, and a real package download.

| Verification level | Verified targets |
| --- | --- |
| Native language/development clients | Git, npm, Yarn, pnpm, Go modules, Cargo, pip, CPAN/cpanm, RubyGems, Maven, NuGet, CRAN/R, Cabal/Hackage, LuaRocks (including CLI-written config), Composer, Rustup, Nix, Homebrew, and Docker/OCI |
| Native package manager in the matching OS container | Debian 12 APT, Ubuntu 24.04 APT, Fedora 42 DNF, Arch Linux pacman, Alpine 3.21 apk, openSUSE Leap 15.6 zypper, Void Linux xbps, Gentoo emerge, Kali rolling APT, Rocky Linux 9 DNF, AlmaLinux 9 DNF, Manjaro pacman, openEuler 24.03 LTS DNF, Anolis OS 8.8 DNF, Deepin 23 APT, ROS 2 Jazzy APT, OpenWrt 24.10.5 opkg, and Termux x86_64 APT |
| Compatible package-manager container | Linux Mint, Trisquel, and Linux Lite through APT; Raspberry Pi OS and Armbian through arm64 APT indexes and packages; MSYS2 through its mingw64 repository with pacman |
| Public protocol endpoint only | FreeBSD, Solus, NetBSD, and OpenBSD; their native userlands/kernels cannot run on a Linux Docker daemon, so these are not labelled native package-manager tests |

The OS checks download at least one real package, not only repository metadata:
`.deb` for Debian-family targets, `.rpm` for Fedora/RHEL-family targets,
`.pkg.tar.zst` for Arch/Manjaro/MSYS2, `.apk` for Alpine, `.ipk` for OpenWrt,
`.xbps` for Void, and a distfile through `emerge --fetchonly` for Gentoo. When a
minimal image lacks CA certificates, `tar`, or `gzip`, only those bootstrap
dependencies may be installed from the image's original repository before its
sources are cleared and MirrorProxy is tested.

Run the repeatable core OS package-manager matrix with:

```bash
./scripts/smoke-os-clients.sh
```

The default matrix covers Debian, Ubuntu, Fedora, Arch Linux, Alpine, openSUSE,
Void, and Gentoo. Select a subset with `MIRRORPROXY_OS_SMOKE_TARGETS`. To verify
an unpublished client fix, point `MIRRORPROXY_OS_SMOKE_CLIENT_BINARY` at a local
static client. The script still runs the public one-line installer first, then
uses the candidate binary for the source-change regression.

## Static Linux Build

On Linux:

```bash
./build.sh
```

The script builds the web console first, then builds `mirrorproxy-server` and `mirrorproxy` release binaries for `x86_64-unknown-linux-musl`.
Install `musl-tools` first so `musl-gcc` is available.

## Reverse Proxy Deployment

MirrorProxy should usually run behind a TLS reverse proxy. Leave `public_base_url` empty to derive the external scheme and host per request, so changing `a.example.com` to `b.example.com` only requires reloading the reverse proxy. Use a fixed `public_base_url` for path-prefix deployments such as `https://example.com/mirrorproxy`.

Forwarded headers are accepted only from `trusted_proxies`, which defaults to `127.0.0.1` and `::1`. Add the proxy's actual peer IP or CIDR when it runs in another host/container. Configure the proxy to **overwrite** `Host`, `X-Forwarded-Host`, and `X-Forwarded-Proto`; never pass client-supplied values through. Keep MirrorProxy's listener private to the proxy where possible.

```toml
# IPs and CIDRs are supported. This setting takes effect immediately in the admin console.
trusted_proxies = ["127.0.0.1", "::1", "172.18.0.0/16"]
```

Equivalent environment setting: `MIRRORPROXY_TRUSTED_PROXIES=127.0.0.1,::1,172.18.0.0/16`.

Nginx example:

```nginx
server {
    listen 443 ssl http2;
    server_name mirror.example.com;

    client_max_body_size 0;
    proxy_request_buffering off;
    proxy_buffering off;

    location / {
        proxy_pass http://selfhost.com;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
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

Caddy overwrites the standard forwarded headers automatically. For a separate container/network, add Caddy's peer address or subnet to `trusted_proxies`.

Traefik (Docker labels) example:

```yaml
labels:
  - traefik.enable=true
  - traefik.http.routers.mirrorproxy.rule=Host(`mirror.example.com`)
  - traefik.http.routers.mirrorproxy.entrypoints=websecure
  - traefik.http.routers.mirrorproxy.tls=true
  - traefik.http.services.mirrorproxy.loadbalancer.server.port=3000
```

Traefik supplies `X-Forwarded-Host` and `X-Forwarded-Proto`; set `trusted_proxies` to its Docker-network address/range, not the public client range.

Apache HTTP Server example:

```apache
ProxyPreserveHost On
ProxyPass / http://127.0.0.1:3000/
ProxyPassReverse / http://127.0.0.1:3000/
RequestHeader set X-Forwarded-Host "%{HTTP_HOST}s"
RequestHeader set X-Forwarded-Proto "https"
```

HAProxy example:

```haproxy
frontend https_in
    bind :443 ssl crt /etc/haproxy/certs/mirror.example.com.pem
    http-request set-header X-Forwarded-Host %[req.hdr(Host)]
    http-request set-header X-Forwarded-Proto https
    default_backend mirrorproxy

backend mirrorproxy
    server app 127.0.0.1:3000
```

Envoy example:

```yaml
static_resources:
  clusters:
    - name: mirrorproxy
      connect_timeout: 1s
      type: STATIC
      load_assignment:
        cluster_name: mirrorproxy
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address: { address: 127.0.0.1, port_value: 3000 }
```

Use Envoy's HTTP connection manager with `use_remote_address: true` and ensure its generated `x-forwarded-proto` / authority headers reach MirrorProxy; add Envoy's peer IP to `trusted_proxies`.

For Docker/OCI and large release files, keep request buffering disabled in the reverse proxy so large blobs stream instead of being fully buffered.

## Security Notes

- MirrorProxy is not an open proxy.
- GitHub absolute URL proxying is restricted to a small allowlist of GitHub-related hosts.
- Hop-by-hop headers are filtered.
- Private upstream registries can use static Basic or Bearer credentials from
  `upstream_auth` in the service TOML. Credentials are matched only to the
  configured upstream host, are never exposed through the admin API or SQLite,
  and client-supplied `Authorization` and `Cookie` headers are never forwarded.
- To use a client's own GitHub, npm, or PyPI token with its matching upstream,
  set `forward_client_authorization = true`. This is disabled by default; a
  configured static `upstream_auth` credential always takes precedence.
- Request-level diagnostic events are retained for 30 days by default; set
  `quota.request_event_retention_days` (or `MIRRORPROXY_REQUEST_EVENT_RETENTION_DAYS`)
  to tune the retention window.

## Roadmap

Version 1.0 already includes the multi-ecosystem and OS repository adapters,
SQLite-backed administration and traffic accounting, global monthly quota,
rate limiting, bounded disk caching, native client releases, the embedded web
console, documented native-client and public-protocol smoke matrices, and
Docker deployment support.

Planned v1.x work:

- Keep signed multi-architecture Docker Hub images with SBOM and provenance
  attestations verifiable as part of every tagged release.
- Add per-user or per-subdomain traffic ownership and independent quotas.
- Keep native-client smoke coverage current across Linux, Windows, macOS, Nix,
  Homebrew, Rustup, and less common language ecosystems.
- Keep Prometheus/OpenTelemetry metrics, structured request tracing, and
  credential-safe alerting examples compatible with supported releases.
- Maintain package-manager-specific source editing and exact rollback for every
  target that advertises `local-config`; keep command/state-database targets
  explicitly marked as templates.
- Evaluate high-availability metadata storage while retaining SQLite as the
  zero-dependency default.

### Spark volunteer mirror network

The planned **Spark** network will let operators voluntarily contribute public
MirrorProxy capacity while clients discover and fail over between nodes without
depending on a single mirror endpoint:

- `spark-mirrors.sina.dev` DNS TXT records publish only a small, versioned set
  of core bootstrap peers; the DNS record is not a global node list.
- Bootstrap peers introduce clients to signed node advertisements through a
  libp2p control plane based on Kademlia discovery, Identify, Ping, and optional
  Gossipsub events.
- A local MirrorProxy agent will score eligible nodes by health, latency,
  declared capacity, and recent request success, then route package-manager
  requests with circuit breaking and failover.
- Volunteer nodes remain restricted MirrorProxy adapters rather than arbitrary
  URL forward proxies. Peer identities, expiring advertisements, integrity
  checks, bandwidth limits, and operator-controlled quotas are required.

Spark deliberately targets directly reachable servers. Every volunteer node
must have a public domain name, a valid publicly trusted HTTPS certificate, and
the required inbound ports open from the Internet. Nodes behind NAT or CGNAT
are out of scope; the roadmap does not include relay bandwidth, UPnP, NAT-PMP,
hole punching, or a NAT-traversing data plane.

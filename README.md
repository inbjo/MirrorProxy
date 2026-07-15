# MirrorProxy

MirrorProxy is a self-hosted mirror proxy platform written in Rust. The `mirrorproxy-server` service and the `mirrorproxy` source-management client are independent binaries. The server embeds the React + Vite + Tailwind web console; the standalone client runs on Windows, macOS, and Linux.

The project is intentionally adapter-based: Docker/OCI, npm, PyPI, Cargo, Go modules, operating system mirrors, and other ecosystems can be added behind the same proxy core.

## Features

- Embedded web console at `/`
- Health endpoint at `/healthz`
- Runtime public config endpoint at `/api/config`
- GitHub proxy for repository pages, raw files, release assets, archives, and Composer GitHub dist URLs
- Composer proxy at `/composer`
- Docker/OCI proxy at `/v2/*` for Docker Hub, GHCR, Quay, and Kubernetes public images
- npm/yarn/pnpm proxy at `/npm`
- Node.js distribution proxy for NVM at `/nvm`
- Go module proxy at `/goproxy`
- Maven Central proxy at `/maven`
- RubyGems proxy at `/rubygems`
- NuGet v3 proxy at `/nuget/v3/index.json`
- CPAN repository proxy at `/cpan`
- CRAN repository proxy at `/cran`
- Hackage repository proxy at `/hackage`
- Clojars repository proxy at `/clojars`
- CocoaPods CDN proxy at `/cocoapods`
- Dart / Flutter Pub proxy at `/pub`
- Anaconda / Conda proxy at `/anaconda`
- TeX Live proxy at `/texlive`
- GNU ELPA proxy at `/elpa`
- Nix binary cache proxy at `/nix`
- GNU Guix substitute cache proxy at `/guix`
- Flatpak OSTree proxy at `/flatpak`
- Homebrew bottle proxy at `/homebrew`
- Debian / Ubuntu / Fedora / Arch Linux / openSUSE / Void / Gentoo / FreeBSD / Alpine / OpenWrt / Termux static proxy at `/os`
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
mirrorproxy sources set maven --mirror mirrorproxy --base-url http://selfhost.com
mvn dependency:resolve
```

The Maven adapter streams Maven2 repository paths, including POMs, metadata, artifacts, checksums, and signatures, from Maven Central.

## RubyGems Proxy

Configure RubyGems to use MirrorProxy as its source:

```yaml
---
:sources:
- http://selfhost.com/rubygems/
```

Save this under `~/.gemrc`, or let the CLI write it with rollback protection:

```bash
mirrorproxy sources set rubygems --mirror mirrorproxy --base-url http://selfhost.com
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
mirrorproxy sources set nuget --mirror mirrorproxy --base-url http://selfhost.com
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
mirrorproxy sources set cpan --mirror mirrorproxy --base-url http://selfhost.com
```

The adapter streams CPAN indexes and distributions such as `modules/02packages.details.txt.gz` and `authors/id/...` while rejecting traversal paths.

## CRAN Proxy

Set R's CRAN repository to MirrorProxy:

```r
options(repos = c(CRAN = "http://selfhost.com/cran/"))
install.packages("digest")
```

`mirrorproxy sources set cran --mirror mirrorproxy --base-url http://selfhost.com` writes a rollback-protected `~/.Rprofile`. Source indexes, archives, and platform binary paths are streamed through `/cran`.

## Hackage Proxy

Configure Cabal's user repository to use MirrorProxy:

```yaml
repository hackage.haskell.org
  url: http://selfhost.com/hackage/
  secure: True
```

`mirrorproxy sources set hackage --mirror mirrorproxy --base-url http://selfhost.com` writes and can restore `~/.cabal/config`. The adapter streams the package index and package tarballs while rejecting traversal paths.

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

`mirrorproxy sources set clojars --mirror mirrorproxy --base-url http://selfhost.com` writes and can restore `~/.clojure/deps.edn`. The adapter streams Clojars POMs, metadata, and JARs with normalized repository paths only.

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

Use fixed target paths such as `http://selfhost.com/os/debian/`, `/os/ubuntu/`, `/os/fedora/`, `/os/archlinux/`, `/os/opensuse/`, `/os/void/`, `/os/gentoo/`, `/os/freebsd/`, `/os/alpine/`, `/os/openwrt/`, `/os/termux/`, `/os/kali/`, `/os/rocky/`, `/os/alma/`, `/os/manjaro/`, `/os/msys2/`, `/os/raspios/`, `/os/armbian/`, `/os/openeuler/`, `/os/anolis/`, `/os/deepin/`, `/os/linuxmint/`, `/os/solus/`, `/os/trisquel/`, `/os/linuxlite/`, `/os/ros/`, `/os/netbsd/`, or `/os/openbsd/`. Only these targets are accepted; each has a separately configurable upstream. Additional OS targets use the `[upstreams.additional_os]` TOML map. The ROS target proxies the ROS 2 Ubuntu APT repository; Solus uses `/os/solus/polaris/eopkg-index.xml.xz`.

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

## Local Source CLI

`mirrorproxy` is a standalone client without Axum, the database, or the embedded
web console. GitHub Releases provide Windows, macOS, and Linux builds; the
independent `mirrorproxy-server` artifact is released for Linux.

Source commands can be invoked chsrc-style as top-level `set`, `get`, `reset`,
`list`, and `mirrors` commands. The fully qualified `sources` form remains
supported; these two commands are equivalent:

```bash
mirrorproxy set bun --mirror mirrorproxy --base-url https://sina.dev --scope user
mirrorproxy sources set bun --mirror mirrorproxy --base-url https://sina.dev --scope user
```

`set` writes user-level npm, pip, Cargo, Go, or Composer configuration
without invoking the package-manager executable. Before its first change it
records the complete previous file in the platform-native user state directory
(`~/.local/state/mirrorproxy/sources/` by default on Linux); `reset` restores
that exact file. A non-empty configuration is never
replaced unless `--force` is explicit, and reset similarly refuses a file that
was changed after the command.

```bash
mirrorproxy set npm --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set cargo --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy reset npm
```

Use `--config-root /tmp/mirrorproxy-config` for an isolated configuration root in
automation or tests. APT, DNF/YUM, pacman, and Docker additionally support
explicit `--scope system`: MirrorProxy only manages the relevant configuration
file and keeps a rollback record under `/var/lib/mirrorproxy/sources/` (or the
supplied root). APT requires a release codename; system writes normally require
root access and are enabled only on Linux hosts.

```bash
mirrorproxy sources set apt --mirror tuna --scope system --distribution jammy
mirrorproxy sources set apt --mirror mirrorproxy --base-url https://mirror.example --scope system --distribution debian/bookworm
mirrorproxy sources reset apt --scope system
mirrorproxy sources set alpine --mirror mirrorproxy --base-url https://mirror.example --scope system --distribution v3.21
mirrorproxy sources reset alpine --scope system
mirrorproxy sources set xbps --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy sources reset xbps --scope system
mirrorproxy sources set zypper --mirror mirrorproxy --base-url https://mirror.example --scope system --distribution distribution/leap/15.6
mirrorproxy sources reset zypper --scope system
mirrorproxy sources set gentoo --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy sources reset gentoo --scope system
mirrorproxy sources set docker --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy sources reset docker --scope system
```

Docker writes `/etc/docker/daemon.json` with `registry-mirrors`. It never
replaces an existing daemon configuration without `--force`, and reset restores
the exact previous file. Restart Docker after applying the configuration.

Copy `config.example.toml` and adjust the public URL for your deployment:

```toml
listen_addr = "selfhost.com"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer", "oci", "npm", "nvm", "opam", "go", "maven", "rubygems", "rustup", "nuget", "cpan", "cran", "hackage", "luarocks", "clojars", "pub", "anaconda", "texlive", "winget", "elpa", "nix", "guix", "flatpak", "homebrew", "os", "crates", "pypi"]

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
go_proxy = "https://proxy.golang.org"
maven = "https://repo.maven.apache.org/maven2"
rubygems = "https://rubygems.org"
nuget = "https://api.nuget.org"
cpan = "https://cpan.metacpan.org"
cran = "https://cloud.r-project.org"
hackage = "https://hackage.haskell.org"
clojars = "https://repo.clojars.org"
pub_repository = "https://pub.dev"
anaconda = "https://repo.anaconda.com/pkgs"
texlive = "https://mirror.ctan.org/systems/texlive/tlnet"
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
MIRRORPROXY_ENABLED_PROXIES=github,composer,oci,npm,go,maven,rubygems,nuget,cpan,cran,hackage,clojars,pub,anaconda,texlive,elpa,nix,guix,flatpak,homebrew,os,crates,pypi
MIRRORPROXY_REQUEST_TIMEOUT_SECS=60
MIRRORPROXY_RATE_LIMIT_ENABLED=true
MIRRORPROXY_RATE_LIMIT_REQUESTS_PER_MINUTE=600
MIRRORPROXY_CACHE_ENABLED=true
MIRRORPROXY_CACHE_DIRECTORY=/var/cache/mirrorproxy
MIRRORPROXY_CACHE_MAX_ENTRY_MB=8
```

MirrorProxy validates `public_base_url`, all upstream URLs, enabled proxy names, and timeout values during startup. Invalid configuration fails fast with a field-specific error.

Optional disk caching is disabled by default. When enabled, it stores only successful public GET responses with an explicit `Content-Length` no larger than `cache.max_entry_mb`; `cache.max_total_mb` bounds disk usage and evicts least-recently-used entries. Requests carrying `Authorization`, `Cookie`, or `Range` bypass the cache. Large or unknown-length responses stay streamed and are never buffered for caching.

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

For a local real-client protocol check (Git, npm/yarn/pnpm, Go, Cargo, pip, CPAN cpanm, RubyGems, Maven, NuGet, CRAN, Cabal/Hackage, and Composer), run:

```bash
./scripts/smoke-clients.sh
```

The script starts a temporary local server, uses temporary client homes/caches, and removes them on exit.

## Static Linux Build

On Linux:

```bash
./build.sh
```

The script builds the web console first, then builds `mirrorproxy-server` and `mirrorproxy` release binaries for `x86_64-unknown-linux-musl`.
Install `musl-tools` first so `musl-gcc` is available.

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
        proxy_pass http://selfhost.com;
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
    reverse_proxy selfhost.com {
        flush_interval -1
    }
}
```

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

- OS mirror source adapters
- Optional caching, rate limiting, and richer observability

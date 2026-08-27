# MirrorProxy

[English](README.md) | [简体中文](README_CN.md)

[![CI](https://img.shields.io/github/actions/workflow/status/inbjo/MirrorProxy/ci.yml?branch=main&style=flat-square&logo=githubactions&label=CI)](https://github.com/inbjo/MirrorProxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/inbjo/MirrorProxy?style=flat-square&logo=github&label=Release)](https://github.com/inbjo/MirrorProxy/releases/latest)
[![License](https://img.shields.io/github/license/inbjo/MirrorProxy?style=flat-square&label=License)](LICENSE)
[![Docker Pulls](https://img.shields.io/docker/pulls/kudang/mirrorproxy?style=flat-square&logo=docker&logoColor=white&label=Docker%20Pulls)](https://hub.docker.com/r/kudang/mirrorproxy)

**Official sources unreachable and public mirrors unreliable? Deploy
MirrorProxy—one self-hosted gateway for every source your builds depend on.**

In restricted networks, official sources are often unreachable or too slow for
normal use. Even after finding a public mirror, it may be inaccessible, unstable,
or gone without warning—and every tool has a different way to change sources.
Developers keep switching configurations while CI builds can stall at any time.
The real cost is not just download time, but the time of the entire team.

Instead of continually searching for and maintaining somebody else's mirrors,
deploy your own MirrorProxy on a server with reliable upstream connectivity. It
brings GitHub, container registries, language packages, developer toolchains,
and operating-system repositories behind your domain: one project, one console,
and one stable gateway for the mainstream software sources used every day.

MirrorProxy fetches and caches content on demand, so it does not require a full
copy of every repository. When one upstream becomes unavailable, it can switch
to another automatically. Individuals can stop hunting, benchmarking, and
reconfiguring mirrors; teams also gain consistent source settings, access
control, quotas, health monitoring, and traffic reporting—turning a patchwork of
public mirrors into infrastructure they truly control.

## What you gain by deploying MirrorProxy

- **Stop hunting for public mirrors:** proxy code hosts, containers, language
  packages, toolchains, and operating-system repositories with one project
  instead of collecting mirror URLs or deploying a service for every ecosystem.
- **Stop benchmarking and switching by hand:** configure multiple upstreams for
  one source, fail over on timeouts, rate limits, and outages, and continuously
  monitor endpoint health.
- **Fewer failures and faster builds:** run MirrorProxy where upstream access is
  reliable, point workstations and CI at your own stable domain, and reuse cached
  dependencies instead of repeating slow cross-border downloads.
- **Consistent configuration on every machine:** use the Windows, macOS, and
  Linux client to inspect, switch, and restore sources without relearning each
  package manager's configuration on every workstation or build host.
- **A mirror you can safely share:** assign monthly quotas and allowed sources to
  users or teams, then combine rate limits, private user endpoints, and IP/CIDR
  rules to prevent anonymous abuse and surprise bandwidth bills.
- **Operations you can actually see:** inspect traffic, trends, visitor regions,
  and the health of every source; receive email or webhook alerts before quotas
  run out or upstream failures become prolonged outages.
- **Ownership of your data:** keep accounts, policies, and statistics locally,
  including fully offline IP geolocation that never sends visitor addresses to
  a third-party lookup service.
- **A path from personal service to team platform:** start quickly with public
  access and sensible defaults, then enable accounts, invitations, OAuth/OIDC,
  team quotas, auditing, Prometheus, and automatic HTTPS as needs grow.

## Supported sources

MirrorProxy includes around 30 proxy adapter types and supports additional
administrator-defined static operating-system repositories:

| Category | Supported sources and ecosystems |
| --- | --- |
| Code and release assets | GitHub, GitHub Raw, read-only Git smart HTTP clone |
| Containers and OCI | Docker Hub, GitHub Container Registry, GitLab Container Registry, Quay, Kubernetes Registry, GCR, Microsoft MCR, Elastic Registry, NVIDIA NVCR, Oracle Container Registry, Homebrew OCI |
| JavaScript / Node.js | npm, NVM, Bun through the npm protocol |
| Python | PyPI / pip, Poetry, uv, PDM, Anaconda |
| Rust / Go | crates.io / Cargo, Rustup, Go Modules |
| JVM / .NET | Maven Central, Clojars, NuGet |
| Other language ecosystems | Composer, RubyGems, CPAN, CRAN, Hackage, Julia, LuaRocks, CocoaPods, Pub, OPAM |
| Developer tools and application sources | Homebrew, WinGet, TeX Live, ELPA, Nix, GNU Guix, Flatpak |
| Linux / BSD / operating-system sources | Debian, Ubuntu, Fedora, Arch Linux, Alpine, openSUSE, Void Linux, Gentoo, FreeBSD, Kali, Rocky Linux, AlmaLinux, Manjaro, Raspberry Pi OS, Armbian, openEuler, Anolis OS, Deepin, Linux Mint, Solus, Trisquel, Linux Lite, NetBSD, OpenBSD |
| Specialized systems and tool sources | OpenWrt, Termux, MSYS2, ROS, and administrator-defined static repositories |

## Capabilities

- **Mirror acceleration:** on-demand proxying, bounded disk caching, and HTTP
  conditional revalidation without the storage cost of a full mirror.
- **Resilient upstreams:** multiple endpoints per source, ordered or adaptive
  selection, automatic failover, circuit recovery, and scheduled health checks.
- **Unified source management:** enable and configure built-in sources or add
  custom static OS repositories in the web console; safely switch and roll back
  local settings with the Windows, macOS, and Linux client.
- **User and team operations:** invitations, email login, OAuth/OIDC,
  billing groups/teams, global-team-user monthly quotas, and per-team source
  restrictions.
- **Secure access:** rate limiting, IP/CIDR allow and deny rules, private user
  subdomains, private-upstream credentials, trusted proxies, administrator roles,
  passkeys, session revocation, and audit logs.
- **Traffic and health insight:** usage trends, offline country and regional
  GeoIP reports, source health, Prometheus metrics, structured logs, optional
  OTLP tracing, and quota/source-failure alerts.
- **Flexible network access:** a shared outbound HTTP/SOCKS5 proxy, enterprise
  CA certificates, and credentials for private npm, OCI, and other upstreams.
- **Simple, reliable deployment:** Docker Compose, Linux release archives, and
  systemd installation; use an existing Caddy/Nginx/Traefik proxy or automate
  HTTPS certificates through ACME HTTP-01/DNS-01.

## Deploy the server

### Docker Compose

Download the included [compose.yaml](compose.yaml), set an administrator password,
and start the service. The named volume keeps the database, cache, and GeoIP data
across container upgrades.

```bash
MIRRORPROXY_ADMIN_PASSWORD='choose-a-strong-password' docker compose up -d
```

The console is available at `http://localhost:3000/admin` when running locally.

### Server archive

Download the server archive for your Linux architecture from the
[latest release](https://github.com/inbjo/MirrorProxy/releases/latest), verify its
SHA-256 file, extract it into a durable directory, then copy and adapt the
configuration before starting the server:

```bash
cp config.example.toml config.toml
./mirrorproxy-server --config ./config.toml serve
```

For systemd, TLS, reverse proxies, persistent storage, and production settings,
see [Deployment](https://github.com/inbjo/MirrorProxy/wiki/Deployment).

## Install the client

### One-command installer

```bash
# macOS / Linux
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
```

```powershell
# Windows PowerShell
irm https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
```

The scripts install only the standalone client, select the current platform
release automatically, and verify its SHA-256 checksum. Run `mirrorproxy --version`
after installation to confirm it is available.

### Package managers

```bash
# macOS / Linux
brew install inbjo/tap/mirrorproxy
```

```powershell
# Windows (available after the WinGet submission is merged)
winget install --id Inbjo.MirrorProxy --exact
```

```bash
# Debian / Ubuntu: add the signed MirrorProxy APT repository once
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/apt/mirrorproxy-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/mirrorproxy-archive-keyring.gpg >/dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/mirrorproxy-archive-keyring.gpg] https://raw.githubusercontent.com/inbjo/MirrorProxy/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/mirrorproxy.list >/dev/null
sudo apt update
sudo apt install mirrorproxy
```

See [Client distribution](https://github.com/inbjo/MirrorProxy/wiki/Distribution) for details.

## Documentation

- [Wiki home](https://github.com/inbjo/MirrorProxy/wiki)
- [Getting started](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started)
- [Docker, binary, and reverse-proxy deployment](https://github.com/inbjo/MirrorProxy/wiki/Deployment)
- [Supported proxy adapters](https://github.com/inbjo/MirrorProxy/wiki/Proxy-Adapters)
- [Standalone client and source management](https://github.com/inbjo/MirrorProxy/wiki/Client)
- [Client distribution and installation](https://github.com/inbjo/MirrorProxy/wiki/Distribution)
- [Administration, identity, quotas, and observability](https://github.com/inbjo/MirrorProxy/wiki/Administration)
- [GeoIP, regional traffic, and IP access control](https://github.com/inbjo/MirrorProxy/wiki/GeoIP-and-IP-Access-Control)
- [Automatic ACME certificates (HTTP-01 and DNS-01)](https://github.com/inbjo/MirrorProxy/wiki/ACME-Certificates)
- [Development, verification, and roadmap](https://github.com/inbjo/MirrorProxy/wiki/Development-and-Roadmap)

## Project links

- [Latest release](https://github.com/inbjo/MirrorProxy/releases/latest)
- [Docker image](https://hub.docker.com/r/kudang/mirrorproxy)
- [Configuration example](config.example.toml)
- [Changelog](CHANGELOG.md)
- [License](LICENSE)

# MirrorProxy

[English](README.md) | [简体中文](README_CN.md)

[![CI](https://img.shields.io/github/actions/workflow/status/inbjo/MirrorProxy/ci.yml?branch=main&style=flat-square&logo=githubactions&label=CI)](https://github.com/inbjo/MirrorProxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/inbjo/MirrorProxy?style=flat-square&logo=github&label=Release)](https://github.com/inbjo/MirrorProxy/releases/latest)
[![License](https://img.shields.io/github/license/inbjo/MirrorProxy?style=flat-square&label=License)](LICENSE)
[![Docker Pulls](https://img.shields.io/docker/pulls/kudang/mirrorproxy?style=flat-square&logo=docker&logoColor=white&label=Docker%20Pulls)](https://hub.docker.com/r/kudang/mirrorproxy)
[![Linux.do Community](https://img.shields.io/badge/Linux.do-Community-FFB300?style=flat-square)](https://linux.do)

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

**Live demo: [https://sina.dev](https://sina.dev)** · [Quick start](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started) · [Latest release](https://github.com/inbjo/MirrorProxy/releases/latest)

## Supported sources

MirrorProxy includes around 30 proxy adapter types and supports additional
administrator-defined static operating-system repositories:

| Category | Supported sources and ecosystems |
| --- | --- |
| Code and release assets | GitHub, GitHub Raw, read-only Git smart HTTP clone |
| Containers and OCI | Docker Hub, GitHub Container Registry, GitLab Container Registry, Quay, Kubernetes Registry, GCR, Microsoft MCR, Elastic Registry, NVIDIA NVCR, Oracle Container Registry, Homebrew OCI |
| JavaScript / Node.js | npm, NVM, Bun through the npm protocol |
| Python | PyPI / pip, Poetry, uv, PDM, Anaconda |
| AI models / datasets | Hugging Face public HTTP downloads (no server cache; set `HF_HUB_DISABLE_XET=1`). |
| Rust / Go | crates.io / Cargo, Rustup, Go Modules |
| JVM / .NET | Maven Central, Clojars, NuGet |
| Other language ecosystems | Composer, RubyGems, CPAN, CRAN, Hackage, Julia, LuaRocks, CocoaPods, Pub, OPAM |
| Developer tools and application sources | Homebrew, WinGet, TeX Live, ELPA, Nix, GNU Guix, Flatpak |
| Linux / BSD / operating-system sources | Debian, Ubuntu, Fedora, Arch Linux, Alpine, openSUSE, Void Linux, Gentoo, FreeBSD, Kali, Rocky Linux, AlmaLinux, Manjaro, Raspberry Pi OS, Armbian, openEuler, Anolis OS, Deepin, Linux Mint, Solus, Trisquel, Linux Lite, NetBSD, OpenBSD |
| Specialized systems and tool sources | OpenWrt, Termux, MSYS2, ROS, and administrator-defined static repositories |
| DNS over HTTPS (DoH) | Optional `/dns-query` proxy with common/custom HTTPS upstreams, configurable ECS, DNS TTL caching and health-based upstream selection; enable and configure in Administration → Advanced. [Configuration](docs/wiki/Proxy-Adapters.md#dns-over-https-doh). |

## Capabilities

- **Mirror acceleration:** on-demand proxying, bounded disk caching, and HTTP
  conditional revalidation without the storage cost of a full mirror.
- **Resilient upstreams:** multiple endpoints per source, ordered or adaptive
  selection, automatic failover, circuit recovery, and scheduled health checks.
- **Unified source management:** enable and configure built-in sources or add
  custom static OS repositories in the web console; safely switch and roll back
  local settings with the Windows, macOS, and Linux client.
- **AI Agent integration:** configure instance-bound Skills and native MCP for
  Codex, Claude Code, Cursor, Gemini CLI and OpenCode; export activation files
  for VS Code/Copilot, TRAE, WorkBuddy and dsh. Discover live mirror capabilities, generate
  configuration commands and use separately authorized download tools.
- **Controlled custom downloads:** fetch public HTTPS files outside the source
  catalog. Disabled by default, with user/group grants, download-only tokens,
  quotas, domain statistics and blocking, and CLI/MCP access.
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
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- --mirror https://sina.dev
```

```powershell
# Windows PowerShell
& ([scriptblock]::Create((irm 'https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1'))) -Mirror 'https://sina.dev'
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
# Windows
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

After installing from your MirrorProxy site, run `sudo mirrorproxy set debian`
or `sudo mirrorproxy set system` to detect the distribution automatically.
See the [OS/version matrix and rollback guide](https://github.com/inbjo/MirrorProxy/wiki/System-Sources).

## AI agents: Skill and MCP

Connect your coding agents to your instance using direct configuration or client-specific exports. The Skill provides usage
instructions, while native MCP tools discover live sources, enabled adapters and
access policy instead of relying on hard-coded mirror URLs.
The setup workflow requires client **v1.7.0 or newer**; the homepage installer
can upgrade an older client before configuration. Additional Agent adapters require
a client built from this revision or a later release; the published v1.7.0 client
only offers Codex, Claude Code and Cursor.

On the [live demo](https://sina.dev) or your own instance, open **Set up your AI
agents** and copy the command for your platform. It installs the client and
starts the Skill + MCP wizard. The wizard asks which Agents and scope to use,
strongly recommends the current user's global scope by default, preserves
existing configuration and provides backups and rollback.

With the client already installed, run this with your instance address:

```bash
mirrorproxy setup --base-url https://mirror.example.com
```

Restart your Agent afterward and retain its normal workspace trust and MCP
approval checks. The native stdio entry point is
`mirrorproxy mcp --base-url https://mirror.example.com`. To install only the Skill,
run `npx skills add inbjo/MirrorProxy --skill mirrorproxy`; this does not install
the native client or configure its MCP connection. See
[Agent Skill and MCP](docs/wiki/Agent-Integration.md) for details.

## Controlled custom downloads

Use the instance to download public HTTPS artifacts such as source archives and
installers beyond the supported repository catalog. This channel is **disabled
by default**: an administrator enables it and grants access to a user or group,
then the user creates a download-only token in their account. Controls include
file-size, traffic, concurrency and rate limits, domain statistics, blocking and
active-download cancellation.

After configuring credentials locally, download to a new file with the CLI;
successful results include its size and SHA-256:

```bash
mirrorproxy download --base-url https://mirror.example.com fetch https://example.org/source.tar.xz --output source.tar.xz
```

The same MCP connection exposes `check_download` and `download_file`. File bodies
are written locally rather than entering model context. Keep credentials local,
never send them to an Agent or place them in a Skill; download tools require
separate permission and consume quota. This is not an unrestricted forward proxy
and does not support private-network targets, uploads or forwarding client
credentials. See [Custom downloads](docs/wiki/Custom-Downloads.md) for setup,
credential storage and verification.

## Documentation

- [Wiki home](https://github.com/inbjo/MirrorProxy/wiki)
- [Getting started](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started)
- [Docker, binary, and reverse-proxy deployment](https://github.com/inbjo/MirrorProxy/wiki/Deployment)
- [Supported proxy adapters](https://github.com/inbjo/MirrorProxy/wiki/Proxy-Adapters)
- [Standalone client and source management](https://github.com/inbjo/MirrorProxy/wiki/Client)
- [Agent Skill and MCP](docs/wiki/Agent-Integration.md)
- [Controlled custom downloads](docs/wiki/Custom-Downloads.md)
- [Client distribution and installation](https://github.com/inbjo/MirrorProxy/wiki/Distribution)
- [Administration, identity, quotas, and observability](https://github.com/inbjo/MirrorProxy/wiki/Administration)
- [GeoIP, regional traffic, and IP access control](https://github.com/inbjo/MirrorProxy/wiki/GeoIP-and-IP-Access-Control)
- [Automatic ACME certificates (HTTP-01 and DNS-01)](https://github.com/inbjo/MirrorProxy/wiki/ACME-Certificates)
- [Development, verification, and roadmap](https://github.com/inbjo/MirrorProxy/wiki/Development-and-Roadmap)

## Project links

- [Live demo](https://sina.dev)
- [Latest release](https://github.com/inbjo/MirrorProxy/releases/latest)
- [Docker image](https://hub.docker.com/r/kudang/mirrorproxy)
- [Configuration example](config.example.toml)
- [Changelog](CHANGELOG.md)
- [License](LICENSE)

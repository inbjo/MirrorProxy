# MirrorProxy

[English](README.md) | [简体中文](README_CN.md)

[![CI](https://img.shields.io/github/actions/workflow/status/inbjo/MirrorProxy/ci.yml?branch=main&style=flat-square&logo=githubactions&label=CI)](https://github.com/inbjo/MirrorProxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/inbjo/MirrorProxy?style=flat-square&logo=github&label=Release)](https://github.com/inbjo/MirrorProxy/releases/latest)
[![License](https://img.shields.io/github/license/inbjo/MirrorProxy?style=flat-square&label=License)](LICENSE)
[![Docker Pulls](https://img.shields.io/docker/pulls/kudang/mirrorproxy?style=flat-square&logo=docker&logoColor=white&label=Docker%20Pulls)](https://hub.docker.com/r/kudang/mirrorproxy)

MirrorProxy is a self-hosted mirror proxy platform written in Rust. The
`mirrorproxy-server` service embeds its React administration console, while the
standalone `mirrorproxy` client manages package sources on Windows, macOS, and
Linux.

Its adapter-based proxy core supports GitHub, Docker/OCI, language package
registries, developer toolchains, and operating-system repositories. SQLite
provides accounts, quotas, traffic accounting, regional reports, and IP/CIDR
access control; offline ip2region databases keep IP location private and fast.

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

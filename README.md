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

## Documentation

- [Wiki home](https://github.com/inbjo/MirrorProxy/wiki)
- [Getting started](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started)
- [Docker, binary, and reverse-proxy deployment](https://github.com/inbjo/MirrorProxy/wiki/Deployment)
- [Supported proxy adapters](https://github.com/inbjo/MirrorProxy/wiki/Proxy-Adapters)
- [Standalone client and source management](https://github.com/inbjo/MirrorProxy/wiki/Client)
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

# MirrorProxy

[English](README.md) | [简体中文](README_CN.md)

[![CI](https://img.shields.io/github/actions/workflow/status/inbjo/MirrorProxy/ci.yml?branch=main&style=flat-square&logo=githubactions&label=CI)](https://github.com/inbjo/MirrorProxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/inbjo/MirrorProxy?style=flat-square&logo=github&label=Release)](https://github.com/inbjo/MirrorProxy/releases/latest)
[![License](https://img.shields.io/github/license/inbjo/MirrorProxy?style=flat-square&label=License)](LICENSE)
[![Docker Pulls](https://img.shields.io/docker/pulls/kudang/mirrorproxy?style=flat-square&logo=docker&logoColor=white&label=Docker%20Pulls)](https://hub.docker.com/r/kudang/mirrorproxy)

MirrorProxy 是一个使用 Rust 编写的自部署镜像代理平台。服务端
`mirrorproxy-server` 内嵌 React 管理控制台，独立的 `mirrorproxy` 客户端负责在
Windows、macOS 和 Linux 上管理软件源。

项目通过 adapter 架构支持 GitHub、Docker/OCI、语言包仓库、开发工具链和操作系统
软件源。SQLite 提供账号、配额、流量统计、地域报表和 IP/CIDR 访问控制；离线
ip2region 数据库保证 IP 定位快速且不向第三方发送请求。

## 安装客户端

Linux 和 macOS 一键安装：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
mirrorproxy --version
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
mirrorproxy --version
```

安装脚本只安装独立客户端，会自动选择当前平台的 Release 资产并验证 SHA-256；也可以通过
以下包管理器安装：

```bash
# macOS / Linux（首次使用需先执行 brew tap inbjo/tap）
brew install mirrorproxy
```

Debian 和 Ubuntu 用户首次安装需添加签名密钥和 MirrorProxy 软件源：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/apt/mirrorproxy-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/mirrorproxy-archive-keyring.gpg >/dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/mirrorproxy-archive-keyring.gpg] https://raw.githubusercontent.com/inbjo/MirrorProxy/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/mirrorproxy.list >/dev/null
sudo apt update
sudo apt install mirrorproxy
```

```powershell
winget install --id Inbjo.MirrorProxy --exact
```

首次添加 Tap、APT 签名仓库和 WinGet 上架状态见[客户端分发与安装](https://github.com/inbjo/MirrorProxy/wiki/Distribution-zh-CN)。

## 文档导航

- [Wiki 首页](https://github.com/inbjo/MirrorProxy/wiki/Home-zh-CN)
- [快速开始](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started-zh-CN)
- [Docker、二进制和反向代理部署](https://github.com/inbjo/MirrorProxy/wiki/Deployment-zh-CN)
- [支持的代理适配器](https://github.com/inbjo/MirrorProxy/wiki/Proxy-Adapters-zh-CN)
- [独立客户端与本机改源](https://github.com/inbjo/MirrorProxy/wiki/Client-zh-CN)
- [客户端分发与安装](https://github.com/inbjo/MirrorProxy/wiki/Distribution-zh-CN)
- [后台、身份认证、配额和可观测性](https://github.com/inbjo/MirrorProxy/wiki/Administration-zh-CN)
- [GeoIP、地域流量与 IP 访问控制](https://github.com/inbjo/MirrorProxy/wiki/GeoIP-and-IP-Access-Control-zh-CN)
- [ACME 自动证书（HTTP-01 与 DNS-01）](https://github.com/inbjo/MirrorProxy/wiki/ACME-Certificates-zh-CN)
- [开发、验证与路线图](https://github.com/inbjo/MirrorProxy/wiki/Development-and-Roadmap-zh-CN)

## 项目链接

- [最新版本](https://github.com/inbjo/MirrorProxy/releases/latest)
- [Docker 镜像](https://hub.docker.com/r/kudang/mirrorproxy)
- [配置示例](config.example.toml)
- [更新记录](CHANGELOG.md)
- [许可证](LICENSE)

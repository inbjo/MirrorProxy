# MirrorProxy

[English](README.md) | [简体中文](README_CN.md)

[![CI](https://img.shields.io/github/actions/workflow/status/inbjo/MirrorProxy/ci.yml?branch=main&style=flat-square&logo=githubactions&label=CI)](https://github.com/inbjo/MirrorProxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/inbjo/MirrorProxy?style=flat-square&logo=github&label=Release)](https://github.com/inbjo/MirrorProxy/releases/latest)
[![License](https://img.shields.io/github/license/inbjo/MirrorProxy?style=flat-square&label=License)](LICENSE)
[![Docker Pulls](https://img.shields.io/docker/pulls/kudang/mirrorproxy?style=flat-square&logo=docker&logoColor=white&label=Docker%20Pulls)](https://hub.docker.com/r/kudang/mirrorproxy)
[![Linux.do 社区](https://img.shields.io/badge/Linux.do-%E7%A4%BE%E5%8C%BA-FFB300?style=flat-square)](https://linux.do)

**官方源访问不了，第三方镜像不可靠？部署一个 MirrorProxy，解决所有常用软件源代理。**

在受限的网络环境中，官方源常常无法连接，或者速度慢到难以正常使用。好不容易找到第三方镜像，
又可能无法访问、速度不稳定，甚至随时失效；不同软件还各有一套改源方式。开发者频繁切换配置，
CI 构建随时卡住，最终浪费的往往不只是下载时间，而是整个团队的时间。

与其长期寻找和维护别人的镜像，不如部署一个属于自己的 MirrorProxy。只需一台网络通畅的服务器，
就能把 GitHub、容器镜像、语言包仓库、开发工具链和操作系统软件源统一代理到自己的域名。
一个项目、一套后台、一个稳定入口，覆盖日常开发所需的主流软件源。

MirrorProxy 按需获取并缓存内容，无需预先完整同步庞大的镜像仓库；当某个上游不可用时，还能自动
切换到备用地址。个人开发者可以告别反复找源、测速和修改配置，团队则可以进一步获得统一改源、
账号权限、流量配额、健康检测和使用统计，把四处拼凑的镜像方案升级为自己真正掌控的基础设施。

## 部署 MirrorProxy，你将获得

- **不再到处寻找第三方镜像**：常用代码托管、容器、语言包、工具链和系统仓库由一个项目统一
  代理，不再收藏大量镜像地址，也不必为每种生态搭建一套服务。
- **不再反复测速和手工切换**：同一个源可以配置多个上游；遇到超时、限流或故障时自动尝试
  备用地址，并通过持续健康检测及时发现不可用入口。
- **更少失败、更快完成的构建**：把服务部署在网络条件更好的位置，开发机和 CI 只访问自己的
  稳定域名；已下载的依赖可由缓存复用，减少跨境请求和重复等待。
- **所有设备使用一致配置**：Windows、macOS、Linux 客户端帮助查看、切换和恢复软件源，
  新电脑、构建机和 CI 环境无需重新研究每个包管理器该如何改源。
- **可放心共享的团队镜像**：为用户或团队分配月度额度和可用源，结合限速、用户专属入口及
  IP/CIDR 黑白名单，既能共享服务，也能避免匿名滥用和意外流量账单。
- **真正看得见的运行状态**：集中查看流量消耗、使用趋势、访问地域和每个镜像源的健康状态；
  配额即将耗尽或上游持续故障时，通过邮件或 Webhook 主动通知。
- **数据与访问记录留在自己手中**：账号、策略和统计保存在本地，IP 地域识别也完全离线完成，
  不向第三方定位接口发送访问者 IP。
- **从个人服务平滑扩展到团队平台**：先以公开代理和默认配置快速启动，需要时再开启账号、邀请、
  OAuth/OIDC、团队配额、审计、Prometheus、自动 HTTPS 等能力，无需更换系统。

**在线演示：[https://sina.dev](https://sina.dev)** · [快速开始](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started-zh-CN) · [最新版本](https://github.com/inbjo/MirrorProxy/releases/latest)

## 支持的软件源

MirrorProxy 提供约 30 类内置代理适配器，并允许在后台添加自定义操作系统静态仓库：

| 类别                 | 已支持的源与生态                                                                                                                                                                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 代码与发布文件       | GitHub、GitHub Raw、Git Smart HTTP 只读克隆                                                                                                                                                                                                |
| 容器与 OCI           | Docker Hub、GitHub Container Registry、GitLab Container Registry、Quay、Kubernetes Registry、GCR、Microsoft MCR、Elastic Registry、NVIDIA NVCR、Oracle Container Registry、Homebrew OCI                                                                                                                        |
| JavaScript / Node.js | npm、NVM、Bun（复用 npm 协议）                                                                                                                                                                                                             |
| Python               | PyPI / pip、Poetry、uv、PDM、Anaconda                                                                                                                                                                                                      |
| AI 模型 / 数据集 | Hugging Face 公开 HTTP 下载（服务端不缓存；设置 `HF_HUB_DISABLE_XET=1`）。 |
| Rust / Go            | crates.io / Cargo、Rustup、Go Modules                                                                                                                                                                                                      |
| JVM / .NET           | Maven Central、Clojars、NuGet                                                                                                                                                                                                              |
| 其他语言生态         | Composer、RubyGems、CPAN、CRAN、Hackage、Julia、LuaRocks、CocoaPods、Pub、OPAM                                                                                                                                                             |
| 开发工具与应用源     | Homebrew、WinGet、TeX Live、ELPA、Nix、GNU Guix、Flatpak                                                                                                                                                                                   |
| Linux / BSD / 系统源 | Debian、Ubuntu、Fedora、Arch Linux、Alpine、openSUSE、Void Linux、Gentoo、FreeBSD、Kali、Rocky Linux、AlmaLinux、Manjaro、Raspberry Pi OS、Armbian、openEuler、Anolis OS、Deepin、Linux Mint、Solus、Trisquel、Linux Lite、NetBSD、OpenBSD |
| 专用系统与工具源     | OpenWrt、Termux、MSYS2、ROS，以及管理员配置的自定义静态仓库                                                                                                                                                                                |
| DNS over HTTPS（DoH） | 可选 `/dns-query` 代理，支持常用及自定义 HTTPS 上游、自动补充客户端网段 ECS、DNS TTL 缓存和首页配置引导；在后台高级设置中启用和配置。[配置说明](docs/wiki/Proxy-Adapters-zh-CN.md#dns-over-httpsdoh)。 |

## MirrorProxy 提供的能力

- **镜像加速**：按需代理、磁盘缓存、HTTP 条件重验证，避免完整同步镜像站带来的存储和维护成本。
- **高可用上游**：为同一个源配置多个地址，支持顺序或自适应选择、自动故障转移、熔断恢复与
  定时健康检测。
- **统一源管理**：在 Web 后台启停内置源、调整上游，并添加自定义操作系统静态仓库；客户端可在
  Windows、macOS 和 Linux 上安全改源与回滚。
- **AI Agent 集成**：为 Codex、Claude Code、Cursor、Gemini CLI 和 OpenCode 配置 Skill 与 MCP，并为
  VS Code/Copilot、TRAE、WorkBuddy 和 dsh 导出激活文件，
  查询实时镜像能力、生成配置命令，并在单独授权后使用下载工具。
- **受控自定义下载**：下载镜像目录之外的公网 HTTPS 文件；默认关闭，支持用户/分组授权、
  下载专用 token、流量限额、域名统计与封禁，可通过 CLI 或 MCP 使用。
- **用户与团队运营**：邀请注册、邮箱登录、OAuth/OIDC、计费组/团队、全局—团队—用户三级
  月度配额，以及团队可用镜像范围控制。
- **安全访问控制**：请求限速、IP/CIDR 黑白名单、用户专属子域名、私有上游凭据、可信反向代理、
  管理员分权、Passkey、会话撤销和审计日志。
- **流量与健康洞察**：用量与趋势统计、离线 GeoIP 国家/省市报表、源健康检测、Prometheus 指标、
  结构化日志、可选 OTLP 追踪，以及配额和源故障告警。
- **灵活网络接入**：支持统一出站 HTTP/SOCKS5 代理、企业内部 CA，并可为私有 npm、OCI 等上游
  配置凭据。
- **简单可靠的部署**：提供 Docker Compose、Linux 发布包和 systemd 安装方式；既可接入现有
  Caddy/Nginx/Traefik，也可通过 ACME HTTP-01 / DNS-01 自动申请和续期 HTTPS 证书。

## 安装服务端

### Docker Compose

下载仓库内的 [compose.yaml](compose.yaml)，设置管理员密码后启动。命名卷会在容器升级时保留
数据库、缓存和 GeoIP 数据。

```bash
MIRRORPROXY_ADMIN_PASSWORD='设置高强度密码' docker compose up -d
```

本机运行时，管理后台地址为 `http://localhost:3000/admin`。

### 二进制发布包

从[最新 Release](https://github.com/inbjo/MirrorProxy/releases/latest)下载对应 Linux 架构的服务端
归档及 SHA-256 校验文件；校验后解压到持久化目录，复制并调整配置，再启动服务：

```bash
cp config.example.toml config.toml
./mirrorproxy-server --config ./config.toml serve
```

systemd、TLS、反向代理、持久化存储和生产配置详见
[服务端部署](https://github.com/inbjo/MirrorProxy/wiki/Deployment-zh-CN)。

## 安装客户端

### 一键安装脚本

```bash
# macOS / Linux
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- --mirror https://sina.dev
```

```powershell
# Windows PowerShell
& ([scriptblock]::Create((irm 'https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1'))) -Mirror 'https://sina.dev'
```

脚本只安装独立客户端，会自动选择当前平台的 Release 资产并验证 SHA-256 校验和。安装后执行
`mirrorproxy --version` 确认客户端可用。

### 包管理器

```bash
# macOS / Linux
brew install inbjo/tap/mirrorproxy
```

```powershell
# Windows
winget install --id Inbjo.MirrorProxy --exact
```

```bash
# Debian / Ubuntu：首次使用时添加已签名的 MirrorProxy APT 仓库
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/apt/mirrorproxy-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/mirrorproxy-archive-keyring.gpg >/dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/mirrorproxy-archive-keyring.gpg] https://raw.githubusercontent.com/inbjo/MirrorProxy/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/mirrorproxy.list >/dev/null
sudo apt update
sudo apt install mirrorproxy
```

详情见[客户端分发与安装](https://github.com/inbjo/MirrorProxy/wiki/Distribution-zh-CN)。

Linux 客户端还提供 `mirrorproxy systems`，统一迁移 28 类发行版的原生软件源。
版本范围、特殊仓库条件及回滚方法见[系统与版本支持矩阵](https://github.com/inbjo/MirrorProxy/wiki/System-Sources-zh-CN)。

## AI Agent：Skill 与 MCP

支持直接配置 Codex、Claude Code、Cursor、Gemini CLI、OpenCode，并为
VS Code/Copilot、TRAE、WorkBuddy、dsh 提供导出激活流程。新增适配器需要使用此修订
构建的客户端或后续 Release；已发布的 v1.7.0 客户端仅提供 Codex、Claude Code、Cursor。Skill 提供实例使用说明，原生 MCP 工具让 Agent 查询
实时软件源、可用适配器和访问策略，无需依赖写死的镜像地址。
配置向导要求客户端 **v1.7.0 及以上**；已有旧版客户端时，可先通过首页安装命令升级。

在[演示站](https://sina.dev)或自己的实例首页，打开 **一键配置 AI Agent**，复制对应系统的
命令，即可安装客户端并启动 Skill + MCP 配置向导。向导询问 Agent 与安装范围，默认并
强烈推荐当前用户全局安装，保留已有配置，并提供备份与回滚。

已有客户端时可直接运行，将地址替换为自己的实例：

```bash
mirrorproxy setup --base-url https://mirror.example.com
```

配置后重启 Agent，保留其正常的工作区信任与 MCP 授权检查。原生 stdio 服务入口为
`mirrorproxy mcp --base-url https://mirror.example.com`。只安装 Skill 可运行
`npx skills add inbjo/MirrorProxy --skill mirrorproxy`；这不会安装原生客户端或配置 MCP 连接。
详细步骤见 [Agent Skill 与 MCP](docs/wiki/Agent-Integration-zh-CN.md)。

## 受控自定义下载

除已支持的软件仓库外，也可通过实例下载源码归档、安装包等公网 HTTPS 文件。该通道
**默认关闭**：管理员需先启用并为用户或分组授权，用户再在用户中心创建下载专用 token。
支持大小、流量、并发和速率限制，以及域名统计、封禁与下载取消。

在本机配置下载凭据后，使用 CLI 下载到一个新文件；成功后输出大小与 SHA-256：

```bash
mirrorproxy download --base-url https://mirror.example.com fetch https://example.org/source.tar.xz --output source.tar.xz
```

同一 MCP 连接也提供 `check_download` 和 `download_file`，文件内容直接写入本机，不进入
模型上下文。凭据仅保存在本机，不要发给 Agent 或写入 Skill；下载工具需单独授权并消耗
配额。此功能不是任意转发代理，不支持私网目标、上传或客户端凭据透传。
配置、凭据保存与验证方法见[自定义下载](docs/wiki/Custom-Downloads-zh-CN.md)。

## 文档导航

- [Wiki 首页](https://github.com/inbjo/MirrorProxy/wiki/Home-zh-CN)
- [快速开始](https://github.com/inbjo/MirrorProxy/wiki/Getting-Started-zh-CN)
- [Docker、二进制和反向代理部署](https://github.com/inbjo/MirrorProxy/wiki/Deployment-zh-CN)
- [支持的代理适配器](https://github.com/inbjo/MirrorProxy/wiki/Proxy-Adapters-zh-CN)
- [独立客户端与本机改源](https://github.com/inbjo/MirrorProxy/wiki/Client-zh-CN)
- [Agent Skill 与 MCP](docs/wiki/Agent-Integration-zh-CN.md)
- [受控自定义下载](docs/wiki/Custom-Downloads-zh-CN.md)
- [客户端分发与安装](https://github.com/inbjo/MirrorProxy/wiki/Distribution-zh-CN)
- [后台、身份认证、配额和可观测性](https://github.com/inbjo/MirrorProxy/wiki/Administration-zh-CN)
- [GeoIP、地域流量与 IP 访问控制](https://github.com/inbjo/MirrorProxy/wiki/GeoIP-and-IP-Access-Control-zh-CN)
- [ACME 自动证书（HTTP-01 与 DNS-01）](https://github.com/inbjo/MirrorProxy/wiki/ACME-Certificates-zh-CN)
- [开发、验证与路线图](https://github.com/inbjo/MirrorProxy/wiki/Development-and-Roadmap-zh-CN)

## 项目链接

- [在线演示](https://sina.dev)
- [最新版本](https://github.com/inbjo/MirrorProxy/releases/latest)
- [Docker 镜像](https://hub.docker.com/r/kudang/mirrorproxy)
- [配置示例](config.example.toml)
- [更新记录](CHANGELOG.md)
- [许可证](LICENSE)

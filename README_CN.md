# MirrorProxy

MirrorProxy 是一个基于 Rust 的自部署镜像代理平台。服务端 `mirrorproxy-server` 与改源客户端 `mirrorproxy` 是两个独立二进制；服务端内嵌 React + Vite + Tailwind Web 控制台，客户端可单独下载并运行于 Windows、macOS 和 Linux。

项目按 adapter 扩展：后续 Docker/OCI、npm、PyPI、Cargo、Go modules、操作系统镜像源等生态可以复用同一套代理核心。

## 功能

- `/` 内嵌 Web 控制台
- `/healthz` 健康检查
- `/api/config` 运行时公开配置
- GitHub 代理：仓库页面、raw 文件、release 文件、archive、Composer 常见 GitHub dist 地址
- `/composer` Composer 镜像代理
- `/v2/*` Docker/OCI 镜像代理，支持 Docker Hub、GHCR、Quay、Kubernetes 公开镜像
- `/npm` npm/yarn/pnpm 镜像代理
- `/goproxy` Go module 代理
- `/maven` Maven Central 代理
- `/rubygems` RubyGems 代理
- `/nuget/v3/index.json` NuGet v3 代理
- `/cpan` CPAN 仓库代理
- `/cran` CRAN 仓库代理
- `/hackage` Hackage 仓库代理
- `/clojars` Clojars 仓库代理
- `/cocoapods` CocoaPods CDN 代理
- `/pub` Dart / Flutter Pub 代理
- `/anaconda` Anaconda / Conda 代理
- `/texlive` TeX Live 代理
- `/elpa` GNU ELPA 代理
- `/nix` Nix binary cache 代理
- `/guix` GNU Guix substitute cache 代理
- `/flatpak` Flatpak OSTree 代理
- `/homebrew` Homebrew bottles 代理
- `/os` Debian / Ubuntu / Fedora / Arch Linux / openSUSE / Void / Gentoo / FreeBSD / Alpine / OpenWrt / Termux 静态目录代理
- `/crates-index` Cargo sparse registry 代理
- `/pypi/simple` pip/PyPI 代理
- 上游响应流式转发，并过滤 hop-by-hop headers
- 默认拒绝不支持的绝对 URL 代理目标，避免开放代理风险

## 快速开始

```bash
cargo run -p mirrorproxy-server --bin mirrorproxy-server -- --config config.example.toml
```

打开：

```text
http://selfhost.com
```

健康检查：

```bash
curl http://selfhost.com/healthz
```

## Docker 部署

服务端镜像使用非 root 的 UID/GID `10001` 运行，监听 `3000` 端口，并把 SQLite
数据库和可选缓存统一保存到 `/data` 持久卷。

```bash
MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com docker compose up -d
docker compose logs mirrorproxy
curl http://127.0.0.1:3000/healthz
```

首次启动日志会输出一次性的随机 `admin` 密码，升级时请保留命名卷
`mirrorproxy-data`。不使用 Compose 也可以直接启动：

```bash
docker run -d --name mirrorproxy --restart unless-stopped \
  -p 3000:3000 \
  -e MIRRORPROXY_PUBLIC_BASE_URL=https://mirror.example.com \
  -v mirrorproxy-data:/data \
  inbjo/mirrorproxy:latest
```

容器支持 `MIRRORPROXY_ENABLED_PROXIES`、配额、缓存和限流等环境变量。如需完整
TOML 配置，可只读挂载配置文件并显式指定路径：

```bash
docker run -d --name mirrorproxy --restart unless-stopped \
  -p 3000:3000 \
  -e MIRRORPROXY_CONFIG=/etc/mirrorproxy/config.toml \
  -e MIRRORPROXY_LISTEN_ADDR=0.0.0.0:3000 \
  -e MIRRORPROXY_DB=/data/mirrorproxy.sqlite3 \
  -v mirrorproxy-data:/data \
  -v "$PWD/config.toml:/etc/mirrorproxy/config.toml:ro" \
  inbjo/mirrorproxy:latest
```

从当前源码构建本机 `linux/amd64` 镜像：

```bash
./scripts/docker-build.sh
```

如果 Docker Hub 访问缓慢或不可用，可以通过 MirrorProxy 拉取构建基础镜像：

```bash
MIRRORPROXY_DOCKER_BASE_REGISTRY=sina.dev/library ./scripts/docker-build.sh
```

执行 `docker login` 后发布 `linux/amd64` 和 `linux/arm64` 多架构 manifest：

```bash
./scripts/docker-build.sh --push --image <dockerhub-user>/mirrorproxy
```

GitHub Actions 的 `Docker` 工作流会执行相同的多架构发布。仓库需要设置变量
`DOCKERHUB_USERNAME` 和机密 `DOCKERHUB_TOKEN`；推送 `v*` tag 会发布语义化版本与
`latest` 标签，手动触发工作流时也可以指定版本。

## GitHub 代理

将支持的 GitHub 绝对 URL 放在你的 MirrorProxy 域名后：

```text
http://selfhost.com/https://github.com/inbjo/Conductor
http://selfhost.com/https://github.com/inbjo/Conductor/releases/download/nightly/conductor-client-linux-amd64.deb
```

当前允许的 GitHub 相关 host：

- `github.com`
- `api.github.com`
- `raw.githubusercontent.com`
- `objects.githubusercontent.com`
- `codeload.github.com`

## Composer 代理

配置 Composer：

```bash
composer config repo.packagist composer http://selfhost.com/composer
composer require monolog/monolog
```

MirrorProxy 会代理 Packagist 元数据，并将常见 GitHub/Packagist 下载 URL 重写回你的 MirrorProxy 公开访问地址。

## Docker / OCI 代理

将 MirrorProxy host 当作 Docker registry 使用：

```bash
docker pull selfhost.com/nginx
docker pull selfhost.com/user/image
docker pull selfhost.com/ghcr.io/user/image
docker pull selfhost.com/quay.io/org/image
docker pull selfhost.com/registry.k8s.io/pause:3.8
```

映射规则：

- `name` 映射到 Docker Hub `library/name`
- `user/image` 映射到 Docker Hub `user/image`
- `ghcr.io/user/image` 映射到 GHCR
- `quay.io/org/image` 映射到 Quay
- `registry.k8s.io/name` 映射到 Kubernetes registry

代理支持公开镜像拉取和上游 Bearer token challenge；私有上游凭据的配置方式见[安全说明](#安全说明)。

## npm / yarn / pnpm 代理

配置包管理器使用 MirrorProxy：

```bash
npm config set registry http://selfhost.com/npm
npm install react

yarn config set npmRegistryServer http://selfhost.com/npm
yarn add react

pnpm config set registry http://selfhost.com/npm
pnpm add react
```

MirrorProxy 会代理 npm 包元数据，并将 `dist.tarball` URL 重写到 `/npm`，确保 tarball 下载也走代理。

## Go 模块代理

将 MirrorProxy 设置为 `GOPROXY`：

```bash
go env -w GOPROXY=http://selfhost.com/goproxy,direct
go list -m github.com/gin-gonic/gin@latest
```

Go adapter 会将 GOPROXY 协议路径，例如 `@v/list`、`.info`、`.mod`、`.zip` 转发到 `proxy.golang.org`。

## Maven Central 代理

在 Maven 用户级 settings 中将 Central 指向 MirrorProxy：

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

保存到 `~/.m2/settings.xml`，或通过带 rollback 保护的 CLI 写入：

```bash
mirrorproxy set maven --mirror mirrorproxy --base-url http://selfhost.com
mvn dependency:resolve
```

Maven adapter 会从 Maven Central 流式转发 Maven2 路径，包括 POM、metadata、artifact、checksum 和签名文件。

## RubyGems 代理

在 RubyGems 用户级配置中将 source 指向 MirrorProxy：

```yaml
---
:sources:
- http://selfhost.com/rubygems/
```

保存到 `~/.gemrc`，或通过带 rollback 保护的 CLI 写入：

```bash
mirrorproxy set rubygems --mirror mirrorproxy --base-url http://selfhost.com
gem install rake
```

RubyGems adapter 会流式转发 compact index（`/versions`、`/info/*`）、旧索引、API 响应和 `.gem` 下载，并保留 Bundler 所需的 Range 与 ETag 头。

## NuGet v3 代理

将 NuGet v3 package source 指向 MirrorProxy：

```xml
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="mirrorproxy" value="http://selfhost.com/nuget/v3/index.json" protocolVersion="3" />
  </packageSources>
</configuration>
```

Windows 使用 `%APPDATA%\NuGet\NuGet.Config`，Linux/macOS 使用 `~/.nuget/NuGet/NuGet.Config`。CLI 会以 rollback 保护写入相同位置：

```bash
mirrorproxy set nuget --mirror mirrorproxy --base-url http://selfhost.com
dotnet restore
```

adapter 会把 NuGet v3 service index 中的资源 URL 重写到 MirrorProxy，并通过 `/nuget` 流式转发 flat container、registration 元数据、搜索结果和包下载。

## CPAN 代理

使用 `cpanm` 指向 CPAN 静态镜像端点：

```bash
cpanm --mirror http://selfhost.com/cpan/ --mirror-only Moo
```

CLI 可将带 rollback 保护的 CPAN 镜像列表写入 `~/.cpan/CPAN/MyConfig.pm`：

```bash
mirrorproxy set cpan --mirror mirrorproxy --base-url http://selfhost.com
```

adapter 会流式转发 `modules/02packages.details.txt.gz`、`authors/id/...` 等 CPAN 索引和发行包，同时拒绝路径穿越请求。

## CRAN 代理

将 R 的 CRAN 仓库设置为 MirrorProxy：

```r
options(repos = c(CRAN = "http://selfhost.com/cran/"))
install.packages("digest")
```

`mirrorproxy set cran --mirror mirrorproxy --base-url http://selfhost.com` 会写入可回滚的 `~/.Rprofile`；源码索引、归档包和平台二进制路径均通过 `/cran` 流式代理。

## Hackage 代理

在 Cabal 用户配置中加入：

```yaml
repository hackage.haskell.org
  url: http://selfhost.com/hackage/
  secure: True
```

`mirrorproxy set hackage --mirror mirrorproxy --base-url http://selfhost.com` 会写入并可恢复 `~/.cabal/config`。adapter 流式转发 package index 与 tarball，同时拒绝路径穿越。

## Clojars 代理

在 Clojure CLI 用户级 `deps.edn` 中配置：

```clojure
{:mvn/repos {"clojars" {:url "http://selfhost.com/clojars/"}}}
```

`mirrorproxy set clojars --mirror mirrorproxy --base-url http://selfhost.com` 会写入并可恢复 `~/.clojure/deps.edn`。adapter 仅通过规范化的仓库路径流式转发 Clojars POM、metadata 和 JAR。

## Pub / Flutter 代理

```bash
PUB_HOSTED_URL=http://selfhost.com/pub/ flutter pub get
```

Pub 元数据和官方 archive 下载都会留在 MirrorProxy；仅重写官方 Google Cloud Storage archive host。

## Anaconda / Conda 代理

将 Conda channel base 设置为例如 `http://selfhost.com/anaconda/main`。adapter 会流式转发 `repodata.json` 与包文件，并拒绝路径穿越。

## TeX Live 代理

将 `http://selfhost.com/texlive/` 用作 TeX Live 网络安装镜像。adapter 会通过规范化路径流式转发 `tlpkg/texlive.tlpdb` 和 archive 文件。

## GNU ELPA 代理

将 `http://selfhost.com/elpa/` 用作 Emacs package archive URL。adapter 仅通过规范化路径流式转发 `archive-contents` 和包归档。

## Nix binary cache 代理

将 `http://selfhost.com/nix/` 用作 Nix substituter。`.narinfo` 签名和相对 cache URL 保持不变，Nix 仍会正常验证缓存签名。

## GNU Guix Substitute Cache 代理

将 `http://selfhost.com/guix/` 用作 Guix substitute URL，例如 `guix build --substitute-urls=http://selfhost.com/guix/ hello`。Narinfo 签名与 substitute payload 会原样流式转发，Guix 仍会验证已授权的缓存密钥。

## Flatpak OSTree 代理

将 `http://selfhost.com/flatpak/` 用作 Flatpak remote URL。OSTree summary 与 GPG 签名会原样流式转发，保留客户端仓库校验。

## Homebrew bottles 代理

在运行 `brew install` 前设置 `HOMEBREW_BOTTLE_DOMAIN=http://selfhost.com/homebrew`。默认上游为 Homebrew 的公开 GHCR OCI bottles 仓库；manifest、blob 和 Range 请求均会原样流式转发。

## OS 静态目录代理

使用固定 target 路径，例如 `http://selfhost.com/os/debian/`、`/os/ubuntu/`、`/os/fedora/`、`/os/archlinux/`、`/os/opensuse/`、`/os/void/`、`/os/gentoo/`、`/os/freebsd/`、`/os/alpine/`、`/os/openwrt/`、`/os/termux/`、`/os/kali/`、`/os/rocky/`、`/os/alma/`、`/os/manjaro/`、`/os/msys2/`、`/os/raspios/`、`/os/armbian/`、`/os/openeuler/`、`/os/anolis/`、`/os/deepin/`、`/os/linuxmint/`、`/os/solus/`、`/os/trisquel/`、`/os/linuxlite/`、`/os/ros/`、`/os/netbsd/` 或 `/os/openbsd/`。仅允许这些 target，且每项都有独立可配置 upstream；新增目标通过 TOML 的 `[upstreams.additional_os]` 映射配置。Linux Mint 默认使用 Kernel.org HTTPS 软件包镜像；ROS target 代理 ROS 2 Ubuntu APT 仓库；Solus 使用 `/os/solus/polaris/eopkg-index.xml.xz`。

## Rust Crates 代理

配置 Cargo 使用 MirrorProxy 作为 sparse registry 镜像：

```toml
[source.crates-io]
replace-with = "mirrorproxy"

[source.mirrorproxy]
registry = "sparse+http://selfhost.com/crates-index/"
```

然后拉取依赖：

```bash
cargo fetch
```

MirrorProxy 会提供本地 sparse `config.json`，并通过 `/crates/api/v1/crates/{crate}/{version}/download` 代理 crate 下载。

## pip / PyPI 代理

配置 pip 使用 MirrorProxy：

```bash
pip config set global.index-url http://selfhost.com/pypi/simple/
pip install requests
```

MirrorProxy 会代理 PyPI Simple API HTML，并将 files.pythonhosted.org 链接重写到 `/pypi/files`。

## 配置

可以通过 CLI 查看或安全修改指定的 TOML 配置文件。`set` 会先创建同目录
`.bak` 备份，再原子替换配置文件；可先加 `--dry-run` 预览变更。

```bash
mirrorproxy-server --config ./config.toml config get public_base_url
mirrorproxy-server --config ./config.toml config set public_base_url https://mirror.example
mirrorproxy-server --config ./config.toml config set quota.monthly_gb 100 --dry-run
```

## 一键安装客户端

Linux 和 macOS 共用同一份安装脚本。脚本会自动识别操作系统与 CPU 架构，从
GitHub 最新稳定版 Release 下载对应客户端，校验 SHA-256 后安装到
`/usr/local/bin`；仅在目录不可写时调用 `sudo`：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
```

也可以通过 MirrorProxy 同时加速安装脚本和稳定版客户端资产：

```bash
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- --mirror https://sina.dev
```

Windows 使用独立 PowerShell 安装器。Windows 默认可能阻止远程脚本，先仅为
当前 PowerShell 进程允许脚本执行，再运行安装命令：

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
```

Windows 加速安装：

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
$env:MIRRORPROXY_DOWNLOAD_MIRROR='https://sina.dev'
irm https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
```

PowerShell 安装器会把 `mirrorproxy.exe` 放到当前用户的本地程序目录，并加入用户
`PATH`。两份脚本都支持通过 `MIRRORPROXY_VERSION` 指定版本、通过
`MIRRORPROXY_INSTALL_DIR` 修改安装目录。`latest` 只选择稳定版本，不会选择滚动的
`nightly` 预发布版本；仓库发布第一个 `v*` tag 后该地址才会可用。

## 本机改源 CLI

`mirrorproxy` 是不包含 Axum、数据库和 Web 控制台的独立客户端。GitHub Release
分别提供 Windows、macOS 和 Linux 构建；`mirrorproxy-server` 则作为独立的 Linux
服务端产物发布。

改源命令统一使用与 chsrc 类似的顶层 `set`、`get`、`reset`、`list` 和 `mirrors`
写法；旧的 `sources` 命名空间仍保留向后兼容。

```bash
mirrorproxy set bun --mirror mirrorproxy --base-url https://sina.dev --scope user
```

`set` 会直接写入用户级 npm、pip、Cargo、Go 或 Composer 配置，不依赖
执行包管理器命令。首次写入前会把完整原文件记录到系统原生的用户状态目录
（Linux 默认为 `~/.local/state/mirrorproxy/sources/`），`reset` 可精确恢复。非空配置默认
拒绝覆盖，必须显式使用 `--force`；如果 set 之后文件又被修改，reset 同样会拒绝
覆盖，避免误删用户内容。

```bash
mirrorproxy set npm --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy set cargo --mirror mirrorproxy --base-url http://selfhost.com
mirrorproxy reset npm
```

自动化或测试可使用 `--config-root /tmp/mirrorproxy-config` 指定隔离的配置根目录。APT、
DNF/YUM、pacman 和 Docker 额外支持显式的 `--scope system`：MirrorProxy 只管理对应的
配置文件，并在 `/var/lib/mirrorproxy/sources/`（或指定 root）保存 rollback 记录。
系统级写入仅在 Linux 主机启用且通常需要 root 权限；APT 必须提供发行版代号。

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

Docker 会写入包含 `registry-mirrors` 的 `/etc/docker/daemon.json`。已有 daemon 配置
不会在未显式传入 `--force` 时被覆盖；reset 会精确恢复原文件。配置生效后需要重启 Docker。

复制 `config.example.toml` 并修改公开访问地址：

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

`public_base_url` 会用于 Web 控制台和元数据重写。部署在 Nginx、Caddy、Traefik 等反向代理后时，请设置为用户实际访问的外部地址。

常用环境变量覆盖：

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

MirrorProxy 会在启动时校验 `public_base_url`、所有上游 URL、启用的代理名称和超时配置。配置非法会快速失败，并提示具体字段。

可选磁盘缓存默认关闭。启用后，仅缓存带明确 `Content-Length` 且不大于 `cache.max_entry_mb` 的成功公开 GET 响应；`cache.max_total_mb` 限制总磁盘用量并按最近最少使用淘汰。携带 `Authorization`、`Cookie` 或 `Range` 的请求会绕过缓存。大文件或长度未知的响应保持流式转发，绝不会为了缓存整块读入内存。

首次启动时，MirrorProxy 会创建 SQLite 数据库，并在本机启动日志中仅输出一次
`admin` 账号的随机密码。使用它调用 `POST /api/admin/login`，再将返回 token 作为
`Authorization: Bearer <token>` 访问 `GET /api/admin/config` 等受保护接口。数据库仅
保存 Argon2 密码哈希，请妥善保护启动日志。

`PUT /api/admin/config` 接收完整且校验通过的配置，写入 SQLite 并记录审计日志。
公开地址、启用 adapter、上游、配额和限流会立即影响后续请求；
`timeout.request_secs` 会持久化但响应会标记需要重启。`listen_addr` 与
`database_path` 必须通过服务配置修改，不能使用该 API 热更新。

`GET /api/admin/stats` 返回当前配置月的摘要、配额剩余字节、按日/按 target 的流量
数据及流量最高的十个代理 target，使用同一个管理员 Bearer token 鉴权。

`POST /api/admin/password` 接收 `current_password` 和至少 12 位的新密码。修改成功后
会撤销全部管理员会话，包括发起修改的当前会话。

可选全局限流配置：

```toml
[rate_limit]
enabled = true
requests_per_minute = 600
```

超过限制时，MirrorProxy 会返回 `429 Too Many Requests`，并带上 `Retry-After` 响应头。

## 流量统计与月度配额

每个代理响应都会在 body 实际流式发送给客户端后计量；不会为了统计而把下载内容读入
内存。SQLite 会保存按日、按代理类型的请求数/字节数/错误数，以及当月总发送字节数。
健康检查、Web 控制台和管理 API 不计入流量，也不会被配额封停。

```toml
[quota]
enabled = true
monthly_gb = 500
timezone = "Asia/Taipei" # 或 "local"
on_exceeded = "stop_proxy" # 使用 "throttle" 时返回 HTTP 429
```

当月已发送 body 字节达到限制后，新的代理请求会根据配置返回 `503`
（`stop_proxy`）或 `429`（`throttle`），而公开页面和管理接口仍可用。配置时区进入新
日历月后会自动使用新的月度统计与配额。

## 开发

构建 Web 控制台：

```bash
cd web
npm ci
npm run build
```

运行 Rust 测试：

```bash
cargo test
```

完整本地检查：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
```

在 Windows PowerShell 运行本地 smoke test：

```powershell
.\scripts\smoke-local.ps1
```

smoke test 会构建 debug 二进制，在临时本地端口启动 MirrorProxy，检查内嵌 Web UI 和关键代理端点，然后自动停止进程。

GitHub Actions 会在 push 和 pull request 中运行格式化、clippy、Rust 测试、前端生产构建，并在 Linux、Windows、macOS 原生测试客户端。推送 `v*` tag 时会构建三平台客户端和 Linux 服务端，并发布带逐文件 checksum 和 `SHA256SUMS` 的 GitHub Release。

本地运行真实客户端协议 smoke（Git、npm/yarn/pnpm、Go、Cargo、pip、CPAN cpanm、RubyGems、Maven、NuGet、CRAN、Cabal/Hackage、Composer）：

```bash
./scripts/smoke-clients.sh
```

脚本会启动临时本地服务，使用临时 client home/cache，并在结束时清理。

## Linux 静态构建

在 Linux 上运行：

```bash
./build.sh
```

脚本会先构建 Web 控制台，再构建 `x86_64-unknown-linux-musl` 的 `mirrorproxy-server` 和 `mirrorproxy` release 二进制。
需先安装 `musl-tools`，以提供 `musl-gcc`。

## 反向代理部署

MirrorProxy 通常应部署在 TLS 反向代理之后。`public_base_url` 请设置为用户实际访问的外部 HTTPS 地址，而不是内部监听地址。

Nginx 示例：

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

Caddy 示例：

```caddyfile
mirror.example.com {
    reverse_proxy selfhost.com {
        flush_interval -1
    }
}
```

Docker/OCI blob 和 GitHub release 大文件建议关闭反向代理请求缓冲，确保大文件流式转发，而不是先完整缓存在反向代理中。

## 安全说明

- MirrorProxy 不是开放代理。
- GitHub 绝对 URL 代理限制在少量 GitHub 相关 host 白名单内。
- 会过滤 hop-by-hop headers。
- 私有上游 registry 可在服务 TOML 的 `upstream_auth` 中配置静态 Basic 或 Bearer 凭据。凭据仅注入到完全匹配的已配置上游主机，不会通过管理 API 返回或写入 SQLite；客户端请求中的 `Authorization` 和 `Cookie` 也不会被转发。
- 如需让 GitHub、npm 或 PyPI 客户端使用自己的 Token，可设置 `forward_client_authorization = true`。该选项默认关闭；已配置的静态 `upstream_auth` 凭据始终优先。
- 请求级诊断明细默认保留 30 天；可通过 `quota.request_event_retention_days` 或环境变量 `MIRRORPROXY_REQUEST_EVENT_RETENTION_DAYS` 调整。

## 路线图

v1.0 已包含多生态与操作系统仓库 adapter、SQLite 管理与流量统计、全站月度配额、
限流、容量受控的磁盘缓存、原生客户端发布、内嵌 Web 控制台、公网全源 smoke，
以及 Docker 部署支持。

v1.x 后续计划：

- 持续发布带签名、SBOM 和 provenance 证明的 Docker Hub 多架构镜像。
- 增加按用户或子域名归属流量及独立配额的能力。
- 为目录中尚未覆盖的目标补齐真实原生客户端 smoke，重点覆盖 Windows、macOS
  和较少使用的语言生态。
- 增加 Prometheus/OpenTelemetry 指标、结构化请求追踪和告警示例，同时避免记录凭据。
- 为剩余目录目标补齐包管理器专用的本机改源与精确回滚能力。
- 在保留 SQLite 零依赖默认方案的前提下，评估高可用 metadata 存储。

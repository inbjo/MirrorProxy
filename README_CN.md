# MirrorProxy

MirrorProxy 是一个基于 Rust 的自部署镜像代理平台。当前可运行切片支持 GitHub 绝对 URL 代理、Composer/Packagist 元数据代理、公开 Docker/OCI registry 拉取代理、npm registry 代理、Go module 代理、Maven Central 代理、RubyGems 代理、NuGet v3 代理、CPAN 代理、Cargo sparse registry 代理，以及 PyPI Simple API 代理，并将 React + Vite + Tailwind Web 控制台内嵌到 Rust 二进制中。

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
- `/pub` Dart / Flutter Pub 代理
- `/anaconda` Anaconda / Conda 代理
- `/texlive` TeX Live 代理
- `/elpa` GNU ELPA 代理
- `/nix` Nix binary cache 代理
- `/flatpak` Flatpak OSTree 代理
- `/homebrew` Homebrew bottles 代理
- `/os` Debian / Ubuntu / Fedora / Arch Linux / openSUSE / Void / Gentoo / FreeBSD / Alpine / OpenWrt / Termux 静态目录代理
- `/crates-index` Cargo sparse registry 代理
- `/pypi/simple` pip/PyPI 代理
- 上游响应流式转发，并过滤 hop-by-hop headers
- 默认拒绝不支持的绝对 URL 代理目标，避免开放代理风险

## 快速开始

```bash
cargo run -- --config config.example.toml
```

打开：

```text
http://127.0.0.1:3000
```

健康检查：

```bash
curl http://127.0.0.1:3000/healthz
```

## GitHub 代理

将支持的 GitHub 绝对 URL 放在你的 MirrorProxy 域名后：

```text
http://127.0.0.1:3000/https://github.com/inbjo/Conductor
http://127.0.0.1:3000/https://github.com/inbjo/Conductor/releases/download/nightly/conductor-client-linux-amd64.deb
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
composer config repo.packagist composer http://127.0.0.1:3000/composer
composer require monolog/monolog
```

MirrorProxy 会代理 Packagist 元数据，并将常见 GitHub/Packagist 下载 URL 重写回你的 MirrorProxy 公开访问地址。

## Docker / OCI 代理

将 MirrorProxy host 当作 Docker registry 使用：

```bash
docker pull 127.0.0.1:3000/nginx
docker pull 127.0.0.1:3000/user/image
docker pull 127.0.0.1:3000/ghcr.io/user/image
docker pull 127.0.0.1:3000/quay.io/org/image
docker pull 127.0.0.1:3000/registry.k8s.io/pause:3.8
```

映射规则：

- `name` 映射到 Docker Hub `library/name`
- `user/image` 映射到 Docker Hub `user/image`
- `ghcr.io/user/image` 映射到 GHCR
- `quay.io/org/image` 映射到 Quay
- `registry.k8s.io/name` 映射到 Kubernetes registry

当前第一版处理公开镜像拉取和上游 Bearer token challenge。私有 registry 凭证会作为后续 adapter 扩展。

## npm / yarn / pnpm 代理

配置包管理器使用 MirrorProxy：

```bash
npm config set registry http://127.0.0.1:3000/npm
npm install react

yarn config set npmRegistryServer http://127.0.0.1:3000/npm
yarn add react

pnpm config set registry http://127.0.0.1:3000/npm
pnpm add react
```

MirrorProxy 会代理 npm 包元数据，并将 `dist.tarball` URL 重写到 `/npm`，确保 tarball 下载也走代理。

## Go 模块代理

将 MirrorProxy 设置为 `GOPROXY`：

```bash
go env -w GOPROXY=http://127.0.0.1:3000/goproxy,direct
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
      <url>http://127.0.0.1:3000/maven/</url>
      <mirrorOf>central</mirrorOf>
    </mirror>
  </mirrors>
</settings>
```

保存到 `~/.m2/settings.xml`，或通过带 rollback 保护的 CLI 写入：

```bash
mirrorproxy sources set maven --mirror mirrorproxy --base-url http://127.0.0.1:3000
mvn dependency:resolve
```

Maven adapter 会从 Maven Central 流式转发 Maven2 路径，包括 POM、metadata、artifact、checksum 和签名文件。

## RubyGems 代理

在 RubyGems 用户级配置中将 source 指向 MirrorProxy：

```yaml
---
:sources:
- http://127.0.0.1:3000/rubygems/
```

保存到 `~/.gemrc`，或通过带 rollback 保护的 CLI 写入：

```bash
mirrorproxy sources set rubygems --mirror mirrorproxy --base-url http://127.0.0.1:3000
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
    <add key="mirrorproxy" value="http://127.0.0.1:3000/nuget/v3/index.json" protocolVersion="3" />
  </packageSources>
</configuration>
```

Windows 使用 `%APPDATA%\NuGet\NuGet.Config`，Linux/macOS 使用 `~/.config/NuGet/NuGet.Config`。CLI 会以 rollback 保护写入相同位置：

```bash
mirrorproxy sources set nuget --mirror mirrorproxy --base-url http://127.0.0.1:3000
dotnet restore
```

adapter 会把 NuGet v3 service index 中的资源 URL 重写到 MirrorProxy，并通过 `/nuget` 流式转发 flat container、registration 元数据、搜索结果和包下载。

## CPAN 代理

使用 `cpanm` 指向 CPAN 静态镜像端点：

```bash
cpanm --mirror http://127.0.0.1:3000/cpan/ --mirror-only Moo
```

CLI 可将带 rollback 保护的 CPAN 镜像列表写入 `~/.cpan/CPAN/MyConfig.pm`：

```bash
mirrorproxy sources set cpan --mirror mirrorproxy --base-url http://127.0.0.1:3000
```

adapter 会流式转发 `modules/02packages.details.txt.gz`、`authors/id/...` 等 CPAN 索引和发行包，同时拒绝路径穿越请求。

## CRAN 代理

将 R 的 CRAN 仓库设置为 MirrorProxy：

```r
options(repos = c(CRAN = "http://127.0.0.1:3000/cran/"))
install.packages("digest")
```

`mirrorproxy sources set cran --mirror mirrorproxy --base-url http://127.0.0.1:3000` 会写入可回滚的 `~/.Rprofile`；源码索引、归档包和平台二进制路径均通过 `/cran` 流式代理。

## Hackage 代理

在 Cabal 用户配置中加入：

```yaml
repository hackage.haskell.org
  url: http://127.0.0.1:3000/hackage/
  secure: True
```

`mirrorproxy sources set hackage --mirror mirrorproxy --base-url http://127.0.0.1:3000` 会写入并可恢复 `~/.cabal/config`。adapter 流式转发 package index 与 tarball，同时拒绝路径穿越。

## Clojars 代理

在 Clojure CLI 用户级 `deps.edn` 中配置：

```clojure
{:mvn/repos {"clojars" {:url "http://127.0.0.1:3000/clojars/"}}}
```

`mirrorproxy sources set clojars --mirror mirrorproxy --base-url http://127.0.0.1:3000` 会写入并可恢复 `~/.clojure/deps.edn`。adapter 仅通过规范化的仓库路径流式转发 Clojars POM、metadata 和 JAR。

## Pub / Flutter 代理

```bash
PUB_HOSTED_URL=http://127.0.0.1:3000/pub/ flutter pub get
```

Pub 元数据和官方 archive 下载都会留在 MirrorProxy；仅重写官方 Google Cloud Storage archive host。

## Anaconda / Conda 代理

将 Conda channel base 设置为例如 `http://127.0.0.1:3000/anaconda/main`。adapter 会流式转发 `repodata.json` 与包文件，并拒绝路径穿越。

## TeX Live 代理

将 `http://127.0.0.1:3000/texlive/` 用作 TeX Live 网络安装镜像。adapter 会通过规范化路径流式转发 `tlpkg/texlive.tlpdb` 和 archive 文件。

## GNU ELPA 代理

将 `http://127.0.0.1:3000/elpa/` 用作 Emacs package archive URL。adapter 仅通过规范化路径流式转发 `archive-contents` 和包归档。

## Nix binary cache 代理

将 `http://127.0.0.1:3000/nix/` 用作 Nix substituter。`.narinfo` 签名和相对 cache URL 保持不变，Nix 仍会正常验证缓存签名。

## Flatpak OSTree 代理

将 `http://127.0.0.1:3000/flatpak/` 用作 Flatpak remote URL。OSTree summary 与 GPG 签名会原样流式转发，保留客户端仓库校验。

## Homebrew bottles 代理

在运行 `brew install` 前设置 `HOMEBREW_BOTTLE_DOMAIN=http://127.0.0.1:3000/homebrew`。默认上游为 Homebrew 的公开 GHCR OCI bottles 仓库；manifest、blob 和 Range 请求均会原样流式转发。

## OS 静态目录代理

使用固定 target 路径，例如 `http://127.0.0.1:3000/os/debian/`、`/os/ubuntu/`、`/os/fedora/`、`/os/archlinux/`、`/os/opensuse/`、`/os/void/`、`/os/gentoo/`、`/os/freebsd/`、`/os/alpine/`、`/os/openwrt/` 或 `/os/termux/`。仅允许这些 target，且每项都有独立可配置 upstream。

## Rust Crates 代理

配置 Cargo 使用 MirrorProxy 作为 sparse registry 镜像：

```toml
[source.crates-io]
replace-with = "mirrorproxy"

[source.mirrorproxy]
registry = "sparse+http://127.0.0.1:3000/crates-index/"
```

然后拉取依赖：

```bash
cargo fetch
```

MirrorProxy 会提供本地 sparse `config.json`，并通过 `/crates/api/v1/crates/{crate}/{version}/download` 代理 crate 下载。

## pip / PyPI 代理

配置 pip 使用 MirrorProxy：

```bash
pip config set global.index-url http://127.0.0.1:3000/pypi/simple/
pip install requests
```

MirrorProxy 会代理 PyPI Simple API HTML，并将 files.pythonhosted.org 链接重写到 `/pypi/files`。

## 配置

可以通过 CLI 查看或安全修改指定的 TOML 配置文件。`set` 会先创建同目录
`.bak` 备份，再原子替换配置文件；可先加 `--dry-run` 预览变更。

```bash
mirrorproxy --config ./config.toml config get public_base_url
mirrorproxy --config ./config.toml config set public_base_url https://mirror.example
mirrorproxy --config ./config.toml config set quota.monthly_gb 100 --dry-run
```

## 本机改源 CLI

`sources set` 会直接写入用户级 npm、pip、Cargo、Go 或 Composer 配置，不依赖
执行包管理器命令。首次写入前会把完整原文件记录到
`~/.local/state/mirrorproxy/sources/`，`sources reset` 可精确恢复。非空配置默认
拒绝覆盖，必须显式使用 `--force`；如果 set 之后文件又被修改，reset 同样会拒绝
覆盖，避免误删用户内容。

```bash
mirrorproxy sources set npm --mirror mirrorproxy --base-url http://127.0.0.1:3000
mirrorproxy sources set cargo --mirror mirrorproxy --base-url http://127.0.0.1:3000
mirrorproxy sources reset npm
```

自动化或测试可使用 `--config-root /tmp/mirrorproxy-home` 指定隔离的主目录。APT、
DNF/YUM、pacman 和 Docker 额外支持显式的 `--scope system`：MirrorProxy 只管理对应的
配置文件，并在 `/var/lib/mirrorproxy/sources/`（或指定 root）保存 rollback 记录。
APT 必须提供发行版代号；系统级写入通常需要 root 权限。

```bash
mirrorproxy sources set apt --mirror tuna --scope system --distribution jammy
mirrorproxy sources reset apt --scope system
mirrorproxy sources set docker --mirror mirrorproxy --base-url https://mirror.example --scope system
mirrorproxy sources reset docker --scope system
```

Docker 会写入包含 `registry-mirrors` 的 `/etc/docker/daemon.json`。已有 daemon 配置
不会在未显式传入 `--force` 时被覆盖；reset 会精确恢复原文件。配置生效后需要重启 Docker。

复制 `config.example.toml` 并修改公开访问地址：

```toml
listen_addr = "127.0.0.1:3000"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer", "oci", "npm", "go", "maven", "rubygems", "nuget", "cpan", "cran", "hackage", "clojars", "pub", "anaconda", "texlive", "elpa", "nix", "flatpak", "homebrew", "os", "crates", "pypi"]

[upstreams]
github = "https://github.com"
github_raw = "https://raw.githubusercontent.com"
packagist = "https://repo.packagist.org"
docker_hub = "https://registry-1.docker.io"
ghcr = "https://ghcr.io"
quay = "https://quay.io"
kubernetes = "https://registry.k8s.io"
npm = "https://registry.npmjs.org"
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
MIRRORPROXY_ENABLED_PROXIES=github,composer,oci,npm,go,maven,rubygems,nuget,cpan,cran,hackage,clojars,pub,anaconda,texlive,elpa,nix,flatpak,homebrew,os,crates,pypi
MIRRORPROXY_REQUEST_TIMEOUT_SECS=60
MIRRORPROXY_RATE_LIMIT_ENABLED=true
MIRRORPROXY_RATE_LIMIT_REQUESTS_PER_MINUTE=600
MIRRORPROXY_CACHE_ENABLED=true
MIRRORPROXY_CACHE_DIRECTORY=/var/cache/mirrorproxy
MIRRORPROXY_CACHE_MAX_ENTRY_MB=8
```

MirrorProxy 会在启动时校验 `public_base_url`、所有上游 URL、启用的代理名称和超时配置。配置非法会快速失败，并提示具体字段。

可选磁盘缓存默认关闭。启用后，仅缓存带明确 `Content-Length` 且不大于 `cache.max_entry_mb` 的成功公开 GET 响应；携带 `Authorization`、`Cookie` 或 `Range` 的请求会绕过缓存。大文件或长度未知的响应保持流式转发，绝不会为了缓存整块读入内存。

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

GitHub Actions 会在 push 和 pull request 中运行格式化、clippy、Rust 测试、前端生产构建与 Windows smoke test。推送 `v*` tag 时会构建 Linux musl/ARM64、macOS ARM64、Windows artifact，并发布带逐文件 checksum 和 `SHA256SUMS` 的 GitHub Release。

本地运行真实客户端协议 smoke（Git、npm/yarn/pnpm、Go、Cargo、pip、CPAN cpanm、RubyGems、Maven、NuGet、CRAN、Composer）：

```bash
./scripts/smoke-clients.sh
```

脚本会启动临时本地服务，使用临时 client home/cache，并在结束时清理。

## Linux 静态构建

在 Linux 上运行：

```bash
./build.sh
```

脚本会先构建 Web 控制台，再构建 `x86_64-unknown-linux-musl` release 二进制。
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
        proxy_pass http://127.0.0.1:3000;
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
    reverse_proxy 127.0.0.1:3000 {
        flush_interval -1
    }
}
```

Docker/OCI blob 和 GitHub release 大文件建议关闭反向代理请求缓冲，确保大文件流式转发，而不是先完整缓存在反向代理中。

## 安全说明

- MirrorProxy 不是开放代理。
- GitHub 绝对 URL 代理限制在少量 GitHub 相关 host 白名单内。
- 会过滤 hop-by-hop headers。
- 当前切片尚未实现私有 registry 凭证。

## 路线图

- 操作系统镜像源 adapter
- 可选缓存、限流和更完整的可观测性

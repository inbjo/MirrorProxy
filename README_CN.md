# MirrorProxy

MirrorProxy 是一个基于 Rust 的自部署镜像代理平台。当前可运行切片支持 GitHub 绝对 URL 代理、Composer/Packagist 元数据代理、公开 Docker/OCI registry 拉取代理、npm registry 代理、Go module 代理、Cargo sparse registry 代理，以及 PyPI Simple API 代理，并将 React + Vite + Tailwind Web 控制台内嵌到 Rust 二进制中。

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

自动化或测试可使用 `--config-root /tmp/mirrorproxy-home` 指定隔离的主目录。
本轮暂不直接写入系统级包管理器文件和 Docker daemon 配置；目录中仍会展示对应的
配置指引。

复制 `config.example.toml` 并修改公开访问地址：

```toml
listen_addr = "127.0.0.1:3000"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer", "oci", "npm", "go", "crates", "pypi"]

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
MIRRORPROXY_ENABLED_PROXIES=github,composer,oci,npm,go,crates,pypi
MIRRORPROXY_REQUEST_TIMEOUT_SECS=60
MIRRORPROXY_RATE_LIMIT_ENABLED=true
MIRRORPROXY_RATE_LIMIT_REQUESTS_PER_MINUTE=600
```

MirrorProxy 会在启动时校验 `public_base_url`、所有上游 URL、启用的代理名称和超时配置。配置非法会快速失败，并提示具体字段。

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

## Linux 静态构建

在 Linux 上运行：

```bash
./build.sh
```

脚本会先构建 Web 控制台，再构建 `x86_64-unknown-linux-musl` release 二进制。

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

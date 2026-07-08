# MirrorProxy

MirrorProxy 是一个基于 Rust 的自部署镜像代理平台。当前可运行切片支持 GitHub 绝对 URL 代理、Composer/Packagist 元数据代理、公开 Docker/OCI registry 拉取代理、npm registry 代理，以及 Go module 代理，并将 React + Vite + Tailwind Web 控制台内嵌到 Rust 二进制中。

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

## 配置

复制 `config.example.toml` 并修改公开访问地址：

```toml
listen_addr = "127.0.0.1:3000"
public_base_url = "https://mirror.example.com"
enabled_proxies = ["github", "composer", "oci", "npm", "go"]

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
```

`public_base_url` 会用于 Web 控制台和元数据重写。部署在 Nginx、Caddy、Traefik 等反向代理后时，请设置为用户实际访问的外部地址。

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

## Linux 静态构建

在 Linux 上运行：

```bash
./build.sh
```

脚本会先构建 Web 控制台，再构建 `x86_64-unknown-linux-musl` release 二进制。

## 安全说明

- MirrorProxy 不是开放代理。
- GitHub 绝对 URL 代理限制在少量 GitHub 相关 host 白名单内。
- 会过滤 hop-by-hop headers。
- 当前切片尚未实现私有 registry 凭证。

## 路线图

- PyPI simple repository 代理
- Cargo sparse registry 代理
- 操作系统镜像源 adapter
- 可选缓存、限流和更完整的可观测性

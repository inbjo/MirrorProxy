# MirrorProxy 开源镜像代理平台实施计划

## Summary

基于 Rust 实现一个可自部署的多源镜像代理平台，提供 GitHub、OCI/Docker Registry、Composer、npm/yarn/pnpm、Go、Rust crates、pip 等镜像代理能力，并提供内嵌在 Rust 二进制中的 React + Vite 8 + Tailwind CSS Web 使用说明界面。项目交付包含 GitHub Actions 自动构建、静态编译脚本、英文 README 与中文 README_CN。

默认技术选型：Rust stable、Tokio、Axum、Reqwest、Rustls、Tower、Tracing、Serde、Clap、config、React + TypeScript + Vite 8、Tailwind CSS、i18next。

## Key Changes

### 1. 项目初始化与工程骨架

目标：
- 创建 Rust workspace 与前端目录。
- 建立统一配置、日志、错误处理、健康检查、版本信息和静态资源嵌入基础。
- 初始化开源项目结构。

建议结构：
- `crates/server`：Rust 服务主程序。
- `web`：React + Vite 8 + Tailwind 前端。
- `.github/workflows`：CI 与 release 构建。
- `README.md`、`README_CN.md`、`build.sh`、`config.example.toml`、`LICENSE`。

验收标准：
- `cargo run` 可启动服务。
- `GET /healthz` 返回健康状态。
- `GET /version` 返回版本、commit、构建时间。
- 未配置任何上游时服务能启动并显示明确警告。
- 前端构建产物可被 Rust 二进制内嵌并通过 `/` 访问。

测试用例：
- 单元测试：配置默认值、环境变量覆盖、非法配置报错。
- 集成测试：启动测试服务后访问 `/healthz`、`/version`、`/`。
- 构建测试：`cargo build` 与前端 `npm run build` 成功。

### 2. 通用代理核心

目标：
- 实现安全、可复用的 HTTP 代理核心，用于所有镜像源。
- 支持请求转发、响应流式传输、Header 过滤、重定向处理、超时、限速、缓存控制和上游错误映射。
- 避免开放代理风险。

关键行为：
- 只允许配置中的 upstream host 或内置白名单源。
- 默认移除 hop-by-hop headers。
- 支持 `GET`、`HEAD`，按协议需要选择性支持其他方法。
- 对大文件使用流式转发，不整块读入内存。
- 提供统一错误 JSON 和 tracing 日志。
- 可选磁盘缓存：默认关闭，后续可按 host/path 配置启用。

验收标准：
- 上游 2xx、3xx、4xx、5xx 能正确透传或映射。
- 大文件下载不造成明显内存增长。
- 非白名单目标被拒绝。
- Range 请求可用于 release、blob、tarball、wheel 等大文件下载。

测试用例：
- Header 过滤测试。
- Range 下载测试。
- 上游超时测试。
- 非法 host、非法 scheme、路径穿越测试。
- 并发请求压力测试。

### 3. GitHub 代理

目标：
- 支持 `https://abc.com/https://github.com/user/repo` 访问仓库页面。
- 支持 GitHub release 下载代理，例如 `/https://github.com/user/repo/releases/download/tag/file.deb`。
- 支持 Git clone 所需的 smart HTTP 基础路径。

关键行为：
- 解析 `/https://github.com/...` 与 `/https://raw.githubusercontent.com/...` 等 GitHub 相关 URL。
- release asset 下载跟随 GitHub 重定向并流式转发最终文件。
- 对 HTML 页面、raw 文件、archive、release 文件保持原始 Content-Type。
- 对 Git clone 相关路径保留查询参数，例如 `info/refs?service=git-upload-pack`。

验收标准：
- 示例 URL 可访问并下载 release 文件。
- `git clone https://abc.com/https://github.com/org/repo.git` 在公开仓库上可工作。
- 上游 404、rate limit、redirect 均有可理解响应。

测试用例：
- URL 解析单元测试。
- release redirect 集成测试。
- `git ls-remote` 集成测试。
- 大文件 Range 下载测试。

### 4. OCI / Docker Registry 代理

目标：
- 支持 Docker Hub、ghcr.io、quay.io、registry.k8s.io。
- 支持用户示例：
  - `docker pull abc.com/nginx`
  - `docker pull abc.com/user/image`
  - `docker pull abc.com/ghcr.io/user/image`
  - `docker pull abc.com/quay.io/org/image`
  - `docker pull abc.com/registry.k8s.io/pause:3.8`

关键行为：
- 实现 OCI Distribution v2 兼容路由。
- 默认无前缀镜像映射到 Docker Hub。
- `library/*` 自动补全官方镜像命名。
- 透传 manifest、blob、tag list。
- 支持 Bearer token challenge 流程。
- 对 blob 使用 redirect 或流式代理，优先保证 Docker 客户端兼容。
- 预留私有 registry 凭证配置，但 v1 默认只支持公开镜像。

验收标准：
- Docker 官方镜像、Docker Hub 用户镜像、GHCR、Quay、Kubernetes 镜像均能 pull。
- manifest list / 多架构镜像可正常解析。
- Docker 客户端不会看到内部上游地址作为最终 registry 地址。
- 常见错误如 unauthorized、not found、rate limit 有明确日志。

测试用例：
- 镜像名解析单元测试。
- token challenge 单元测试。
- 使用本地 mock registry 的 manifest/blob 集成测试。
- Docker CLI smoke test：`docker pull` 五类示例镜像。
- 并发拉取同一 blob 测试。

### 5. Composer 代理

目标：
- 提供 `https://abc.com/composer` 作为 Composer repository/mirror。
- 代理 Packagist 元数据和 dist/source 下载地址。

关键行为：
- 代理 `packages.json`、provider metadata、package metadata。
- 重写 metadata 中的 dist/source URL，使下载继续走 MirrorProxy。
- 支持 Composer v1/v2 元数据格式读取和透传。
- 默认上游为 Packagist。

验收标准：
- Composer 可配置该代理并安装公开包。
- 元数据中的下载地址不泄漏为不可用的内网路径。
- 缓存头合理，避免每次安装全量请求上游。

测试用例：
- metadata URL 重写单元测试。
- `composer config repo.packagist composer https://abc.com/composer` 后安装包 smoke test。
- package not found、上游超时测试。

### 6. npm / yarn / pnpm 代理

目标：
- 提供 npm registry 兼容代理，例如 `https://abc.com/npm/`。
- 支持 npm、yarn、pnpm 使用同一 registry URL。

关键行为：
- 代理 package metadata、dist-tags、tarball。
- 重写 `dist.tarball` 到 MirrorProxy 地址。
- 支持 scoped package，例如 `@scope/name`。
- 默认只支持公开包，认证透传作为后续增强项。

验收标准：
- `npm install`、`yarn add`、`pnpm add` 可从代理安装公开包。
- scoped package 可正常安装。
- tarball 下载支持 Range 和流式传输。

测试用例：
- package name 与 scoped package 路由解析测试。
- metadata tarball URL 重写测试。
- npm/yarn/pnpm smoke test。
- 404、dist-tag missing、上游限流测试。

### 7. Go 模块代理

目标：
- 提供 Go GOPROXY 兼容接口，例如 `https://abc.com/goproxy`。
- 支持公开 Go modules 下载。

关键行为：
- 实现 GOPROXY 协议路径：`/@v/list`、`/@v/{version}.info`、`/@v/{version}.mod`、`/@v/{version}.zip`。
- 默认上游代理为 `https://proxy.golang.org`。
- 保留 `GONOSUMDB`、`GOSUMDB` 由用户侧配置，不在 v1 内实现 checksum 服务。

验收标准：
- `GOPROXY=https://abc.com/goproxy go get module@version` 可工作。
- 不存在模块返回 Go 客户端可识别的 404/410。
- zip、mod、info 内容正确透传。

测试用例：
- GOPROXY 路由解析测试。
- `go env GOPROXY=... go list -m` smoke test。
- invalid module path 测试。

### 8. Rust crates 代理

目标：
- 提供 Cargo sparse registry 代理与 crate 下载代理。
- 支持用户配置 Cargo 使用 MirrorProxy 作为 crates.io 镜像。

关键行为：
- 代理 crates.io sparse index。
- 重写 index `config.json` 中的 `dl` 到 MirrorProxy。
- 代理 `.crate` 下载。
- 默认不支持 publish API。

验收标准：
- Cargo 可通过代理下载公开 crate。
- `cargo fetch`、`cargo build` 对公开依赖可工作。
- index 和 crate 下载路径符合 Cargo sparse registry 预期。

测试用例：
- sparse index path 计算测试。
- `config.json` 重写测试。
- `cargo fetch` smoke test。
- crate not found、checksum 不匹配场景记录清晰错误。

### 9. pip / PyPI 代理

目标：
- 提供 pip simple repository 兼容代理，例如 `https://abc.com/pypi/simple/`。
- 支持 wheel、sdist 下载。

关键行为：
- 代理 PyPI Simple API HTML/JSON。
- 重写 package file href 到 MirrorProxy。
- 支持 normalized package name。
- 默认上游为 `https://pypi.org/simple/` 和 files.pythonhosted.org。

验收标准：
- `pip install -i https://abc.com/pypi/simple package` 可工作。
- wheel 与 sdist 下载走代理。
- HTML simple index 链接正确重写。

测试用例：
- Python package name normalization 测试。
- HTML link rewrite 测试。
- pip smoke test。
- package not found、file 404 测试。

### 10. Web 界面

目标：
- 构建首页即使用说明界面，不做营销落地页。
- 支持中英文、暗黑/明亮主题切换。
- 展示每类镜像源的配置方式、复制命令、状态检查和示例。
- 构建后内嵌到 Rust 二进制。

界面要求：
- React + TypeScript + Vite 8 + Tailwind CSS。
- 使用紧凑、工程工具风格 UI。
- 首页包含：服务状态、代理类型导航、快速复制命令、配置示例、常见问题。
- 语言和主题偏好保存在 localStorage。
- 移动端和桌面端均可用。
- 不在页面中展示无效或未启用的代理项；若配置禁用某代理，前端显示为 disabled。

验收标准：
- `/` 可访问内嵌前端。
- 中英文切换立即生效。
- 明亮/暗黑主题无闪烁或严重对比度问题。
- 所有复制按钮可用。
- 前端构建产物被 Rust release 二进制包含，无需单独部署静态文件。

测试用例：
- React 单元测试：语言切换、主题切换、命令生成。
- Playwright：桌面和移动截图检查、复制按钮、路由刷新。
- Rust 集成测试：静态资源 fallback、缓存头、SPA 路由。

### 11. 配置、部署与安全

目标：
- 让用户用一个二进制和一个配置文件完成部署。
- 提供明确的安全默认值。

关键配置：
- `listen_addr`
- `public_base_url`
- `enabled_proxies`
- `upstreams`
- `timeout`
- `cache`
- `rate_limit`
- `access_log`
- `trusted_proxy_headers`

安全要求：
- 默认拒绝任意 URL 开放代理。
- 默认不透传敏感请求头到非对应上游。
- URL 重写必须防止 SSRF、路径穿越、CRLF 注入。
- 支持反向代理部署时的 `X-Forwarded-*` 配置，但默认不信任。

验收标准：
- 示例配置可直接运行。
- 错误配置启动失败并提示具体字段。
- 安全测试用例全部通过。

测试用例：
- SSRF host 变体测试。
- encoded path traversal 测试。
- header injection 测试。
- public_base_url 缺失或非法测试。

### 12. CI、发布与文档

目标：
- GitHub Actions 自动完成测试、构建、release artifact。
- 提供静态编译 `build.sh`。
- 提供完整英文 README 和中文 README_CN。

GitHub Actions：
- PR/Push：fmt、clippy、test、frontend build。
- Release：Linux x86_64 musl 静态二进制、Linux arm64、Windows、macOS。
- 生成 checksums。
- 上传 artifact。

`build.sh`：
- 安装/检查 Rust target。
- 构建前端。
- 执行 `cargo build --release --target x86_64-unknown-linux-musl`。
- 输出二进制路径和 sha256。

README 内容：
- 项目介绍。
- 功能矩阵。
- 快速开始。
- Docker/OCI 使用示例。
- GitHub release 与 git clone 示例。
- Composer/npm/Go/Rust/pip 配置示例。
- 配置文件说明。
- 反向代理部署示例。
- 安全说明。
- 开发与贡献指南。

验收标准：
- 新用户按 README 可在 10 分钟内启动服务。
- GitHub Actions 在干净环境通过。
- release artifact 可直接运行。
- README 与 README_CN 内容同步。

测试用例：
- CI 全流程跑通。
- release workflow dry-run 或 tag 测试。
- `build.sh` 在 Linux 环境成功产物。
- README 示例命令抽样验证。

## Test Plan

整体测试分层：
- 单元测试：URL 解析、镜像名解析、metadata 重写、配置校验、安全过滤。
- 集成测试：使用 mock upstream 验证代理核心、header、redirect、range、错误映射。
- 客户端 smoke test：`git`、`docker`、`composer`、`npm`、`yarn`、`pnpm`、`go`、`cargo`、`pip`。
- 前端测试：React 单测、Playwright 桌面/移动端、暗黑/明亮、中英文切换。
- 发布测试：CI matrix、静态构建、二进制启动、内嵌静态资源访问。

最低 v1 通过标准：
- GitHub release 下载、Docker Hub/ghcr/quay/k8s pull、npm install、pip install、cargo fetch、go list、composer install 至少各有一个公开包 smoke test 通过。
- 所有 SSRF 与开放代理防护测试通过。
- README 示例命令与实际路由一致。

## Assumptions

- 当前仓库为空目录，计划从零初始化项目。
- 默认服务域名示例统一使用 `https://abc.com`，实现中通过 `public_base_url` 配置生成真实命令。
- Vite 8 使用当前官方 Vite 8 系列；Node.js 版本按 Vite 8 要求使用 20.19+ 或 22.12+。
- v1 默认只代理公开资源；私有 registry、GitHub token、npm token、PyPI token 作为后续增强，不阻塞首版。
- v1 默认不启用持久缓存；先保证协议兼容、安全和可观测性，再增加缓存策略。
- Docker/OCI 兼容性优先级最高，其次是 GitHub、npm、pip、Rust、Go、Composer。

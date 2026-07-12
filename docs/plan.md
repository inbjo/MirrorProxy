# MirrorProxy 开源镜像代理平台实施计划

## Summary

基于 Rust 实现一个可自部署的多源镜像代理平台，提供 GitHub、OCI/Docker Registry、Composer、npm/yarn/pnpm、Go、Rust crates、pip 等镜像代理能力，并提供内嵌在 Rust 二进制中的 React + Vite 8 + Tailwind CSS Web 使用说明界面。项目交付包含 GitHub Actions 自动构建、静态编译脚本、英文 README 与中文 README_CN。

默认技术选型：Rust stable、Tokio、Axum、Reqwest、Rustls、Tower、Tracing、Serde、Clap、config、React + TypeScript + Vite 8、Tailwind CSS、i18next。

## Current Baseline

当前仓库已经不是空目录，已有一个可运行的基础版本：

- Rust workspace 已存在，服务端在 `crates/server`，前端在 `web`。
- 已实现并注册的代理类型：GitHub、Composer/Packagist、Docker/OCI、npm、Go module、Cargo sparse registry、PyPI Simple API、Maven、RubyGems、NuGet v3、CPAN、CRAN、Hackage、Clojars、Pub/Flutter、Anaconda/Conda。
- 已有运行时配置读取与持久化：`config.example.toml`、环境变量覆盖、SQLite 运行时配置、`/api/public-config` 公开摘要与受保护的管理配置 API。
- 已有 Web 控制台：React + Vite 内嵌到 Rust 二进制，支持说明页、源目录、登录、代理/上游/配额配置、CLI 命令生成、统计与审计日志。
- 已有请求限流、SQLite 流量统计和按月流量封停；代理响应默认保持流式计量，并可选缓存有明确长度的小型公开 GET 响应到磁盘。

未完成或需要重做的部分：

- 仍缺少部分计划中的生态 adapter；主流 Linux 发行版（含 Debian、Ubuntu、Fedora、Arch、openSUSE、Void、Gentoo、FreeBSD）已加入受限 OS 静态目录代理，Homebrew bottles 已通过 `/homebrew` 提供 GHCR OCI bottle 流式代理，GNU Guix substitute cache 已通过 `/guix` 提供受限流式代理，Rustup 发布与自更新资源已通过 `/rustup` 提供流式代理。
- chsrc 主要目标现已完成 catalog 登记；当前 CLI 写入/回滚覆盖 npm、pip、cargo、go、composer、docker、apt、dnf、pacman、Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Clojars、Anaconda；Guix 会生成官方 `--substitute-urls` 单次命令，其他登记目标明确标为仅配置/计划中。
- 真实客户端 smoke 已在 CI 覆盖 Git、Composer、npm/yarn/pnpm、Go、Cargo、pip、Docker、CPAN cpanm、RubyGems、Maven、NuGet、CRAN 和 Cabal/Hackage；其余生态客户端仍待补齐，并需持续保留路由/单元测试。
- 小对象可选磁盘缓存已完成（默认关闭，跳过认证、Cookie 与 Range 请求），并具备总容量限制与 LRU 淘汰；私有 registry 凭证和按用户配额仍属于后续增强。月配额已使用 SQLite 原子预留窗口控制并发超卖，超大单流仍按流式计量结算。

当前完成度估算：

- 代理服务基础能力：约 96%（主流开发生态、Rustup、CocoaPods CDN、GNU Guix substitute cache 与主要 Linux 静态仓库已覆盖，Homebrew bottles 已接入，并已有主要客户端协议 smoke 与小对象磁盘缓存；部分客户端/adapter 尚缺）。
- Web 控制台：约 90%（公开说明、源目录、登录、设置、统计、审计已完成；已覆盖语言/主题偏好、命令复制及管理控制台登录、配置加载与保存的 Chromium 浏览器端到端测试，密码更新和统计刷新等边缘流程仍待补齐）。
- 配置持久化与管理后台：约 85%。
- CLI 改源能力：约 65%（已覆盖计划中首批目标并具备回滚；尚未覆盖更多生态和跨平台细节）。
- SQLite 统计与月流量限制：约 86%（持久统计、封停、并发原子预留及缓存容量控制已完成；按用户配额和可选明细保留策略仍待完善）。
- 对齐 chsrc 支持源范围：约 84%，主要目标已登记且更多语言协议可代理，但 OS/软件仓库 adapter 仍有明显缺口。
- 整体按本计划口径估算：约 98%。

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

### 13. 对齐 chsrc 支持源范围

参考项目：`/home/flex/Code/Rust/chsrc`。

chsrc 的目标分三大类：

- 语言和开发生态：Ruby、Python/pip/Poetry/PDM/Rye/uv、JavaScript/npm/pnpm/Yarn/Bun/nvm、Perl、PHP/Composer、Lua、Go、Rust Cargo/rustup、Java/Maven、Clojure、Dart/Pub/Flutter、NuGet、Haskell、OCaml、R、Julia。
- 操作系统源：Ubuntu、Linux Mint、Debian、Fedora、openSUSE、Kali、MSYS2、Arch/ArchLinuxCN、Manjaro、Gentoo、RockyLinux、AlmaLinux、Alpine、Void、Solus、Trisquel、Linux Lite、ROS、Raspberry Pi OS、Armbian、OpenWrt、Termux、openKylin、openEuler、Anolis、deepin、FreeBSD、NetBSD、OpenBSD。
- 软件仓库：WinGet、Homebrew、CocoaPods、Docker、Flatpak、Nix、Guix、Emacs、TeX Live、Anaconda。

chsrc 的镜像站集合包括：

- 通用教育网镜像站：MirrorZ、TUNA、SJTUG、BFSU、USTC、ZJU、JLU、LZUOSS、PKU、BJTU、SUSTech、NJU、XJTU、HUST、ISCAS、HIT、SCAU、NJTech、NYIST、SDU、QLU、CQUPT、CQU、Neusoft。
- 商业镜像站：Ali、Tencent、Huawei/HuaweiCDN、Volcengine、Netease、Sohu、Api7、Fit2Cloud、DaoCloud。
- 专用镜像站：RubyChina、EmacsChina、npmmirror、goproxy.io、goproxy.cn、RsProxy.cn、FlutterCN。

MirrorProxy 需要把这些能力拆成三层，而不是直接把每个 chsrc recipe 都当成 HTTP 反向代理：

1. 服务端代理 adapter：
   - 适合有明确 registry 协议或下载协议的生态。
   - 当前已有：GitHub、Composer、OCI、npm、Go、Cargo、PyPI。
   - 优先新增：Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Clojars、Pub/Flutter、Homebrew bottles、Anaconda、TeX Live、ELPA、Nix channel/cache、Flatpak、OS 静态目录代理。
2. CLI 本机改源：
   - 适合需要修改本机配置文件或命令行配置的生态。
   - 例如 APT、YUM/DNF、pacman、apk、zypper、xbps、pkg、Homebrew、Docker daemon、pip/npm/cargo/go env、Poetry/PDM/uv 等。
3. 仅文档或配置模板：
   - 某些源需要用户系统版本、发行版代号、权限或已有配置上下文，服务端只能生成建议配置，不应盲写。

数据建模：

- 新增 `mirror_catalog`：镜像站元数据，字段包括 `code`、`name`、`kind`、`homepage`、`speed_test_url`、`enabled`。
- 新增 `source_targets`：目标生态元数据，字段包括 `code`、`category`、`aliases`、`supported_modes`、`default_scope`。
- 新增 `target_sources`：目标生态到镜像 URL 的映射，字段包括 `target_code`、`provider_code`、`repo_url`、`speed_url`、`capability`。
- 新增 `source_templates`：CLI 写配置或生成配置文本所需模板，字段包括 `target_code`、`os_family`、`scope`、`template`、`requires_sudo`。

验收标准：

- `mirrorproxy sources list` 能列出与 chsrc 分类相近的目标和镜像站。
- Web 能按语言、系统、软件仓库分类展示可用源。
- 对当前已实现 adapter 的目标，Web 和 CLI 能直接生成 MirrorProxy 代理地址。
- 对暂未实现 adapter 的目标，Web 和 CLI 能生成外部镜像站配置或提示未支持代理。
- chsrc 中的目标至少完成数据登记，不遗漏大类。

### 14. CLI 改源命令

目标：
- 在现有二进制上增加 CLI 子命令，支持类似 chsrc 的 `list/get/set/reset` 工作流。
- CLI 既能修改本机源地址，也能把源地址改为当前 MirrorProxy 服务地址。

建议命令：

```bash
mirrorproxy serve --config config.toml
mirrorproxy sources list
mirrorproxy sources list --category lang
mirrorproxy sources mirrors
mirrorproxy sources get npm
mirrorproxy sources set npm --mirror mirrorproxy --base-url http://127.0.0.1:3000
mirrorproxy sources set pip --mirror tuna --scope user
mirrorproxy sources reset npm --scope user
mirrorproxy config get
mirrorproxy config set public_base_url http://127.0.0.1:3000
```

实现要求：

- `serve` 保持当前服务启动行为。
- `sources list` 使用内置 catalog 或 SQLite catalog，不依赖启动 Web 服务。
- `set/reset` 需要分 target 实现，先覆盖 npm、pip、cargo、go、composer、docker、apt、yum/dnf、pacman。
- 默认写用户级配置；需要系统级写入时必须显式 `--scope system`，并清楚提示需要权限。
- 写配置前备份原文件，备份记录写入 SQLite 或本地状态文件，便于 rollback。
- 对 Windows、macOS、Linux 分平台处理，不支持的平台返回明确错误。

验收标准：

- `mirrorproxy sources set npm --mirror mirrorproxy` 后 `npm config get registry` 指向 MirrorProxy。
- `mirrorproxy sources set pip --mirror mirrorproxy` 后 `pip config list` 生效。
- `mirrorproxy sources set cargo --mirror mirrorproxy` 生成正确 sparse registry 配置。
- `mirrorproxy sources reset <target>` 能恢复 CLI 修改前的配置。
- CLI 不会静默覆盖用户手写配置；发生冲突时提示并要求 `--force`。

### 15. SQLite 配置、鉴权和管理 API

目标：
- 将运行时配置、管理鉴权、统计和配额落到 SQLite。
- Web 设置页通过受保护 API 读写配置，不再只依赖 TOML。

启动规则：

- 服务启动时打开 SQLite，默认路径为 `mirrorproxy.sqlite3`，可通过 `MIRRORPROXY_DB` 或配置文件指定。
- 首次启动如果没有管理员账号或管理密码，生成一个随机密码。
- 随机密码只在启动日志和本机启动输出中显示一次；数据库只保存密码哈希。
- 支持后续在 Web 或 CLI 中重置管理员密码。

鉴权方案：

- 管理 API 使用登录接口换取 session token。
- session token 存 SQLite，保存 `token_hash`、`created_at`、`expires_at`、`last_used_at`。
- Web 设置页、统计页、配置写入接口都必须鉴权。
- 公开接口保留：`/healthz`、`/version`、静态资源、必要的代理下载路径。
- `/api/config` 拆分为公开只读摘要和管理完整配置：`/api/public-config`、`/api/admin/config`。

建议表：

- `settings`：配置键值和版本号。
- `admin_users`：管理员账号、密码哈希、创建时间、更新时间。
- `admin_sessions`：登录会话。
- `proxy_targets`：启用的代理、上游地址、路由前缀。
- `traffic_daily`：按日统计请求数、响应字节、上游字节、错误数。
- `traffic_monthly`：按月统计总流量和配额状态。
- `request_events`：可选的短期请求明细，用于排障，默认设置保留天数。
- `config_audit_log`：配置变更审计。

验收标准：

- 首次启动会生成随机管理密码，并能用该密码登录 Web 设置页。
- 未登录访问管理 API 返回 401。
- 已登录可查看和修改 `public_base_url`、启用代理、上游地址、超时、限流、月流量上限。
- 配置更新后立即影响新请求；需要重启才生效的配置必须明确标记。
- SQLite migration 可重复执行，旧数据库可平滑升级。

### 16. Web 设置页和可视化配置

目标：
- Web 从说明页升级为完整控制台：概览、代理配置、镜像源目录、CLI 命令生成、流量统计、配额设置、登录状态。

页面结构：

- 登录页：输入启动时随机密码或管理员密码。
- 概览页：服务状态、启用代理、当月流量、月配额剩余、错误率。
- 代理设置页：每个 adapter 的启用开关、上游地址、路由前缀、超时、是否计入流量配额。
- 镜像源目录页：参考 chsrc 分类展示语言、系统、软件仓库源。
- CLI 改源页：选择目标、镜像站、作用域，生成或复制 CLI 命令。
- 统计页：按天/月展示请求数、流量、错误、Top target。
- 安全页：修改管理员密码、退出登录、查看最近配置变更。

交互要求：

- 编辑配置使用表单校验，URL 字段即时校验 scheme/host。
- 危险操作如关闭服务、重置配置、清空统计需要二次确认。
- 配额达到后，页面明确显示“代理已因月流量上限停止”。
- 公开说明页仍可免登录访问，但设置和统计必须登录。

验收标准：

- 用户无需编辑 TOML 即可完成常用配置。
- Web 配置保存后后端 SQLite 可见，并影响运行行为。
- 页面不会展示未实现的“可代理”能力；未实现项标为“仅支持生成配置”或“计划中”。

### 17. 流量统计和月流量限制

目标：
- 统计代理服务的真实流量，并支持“每月达到多少 GB 就停止服务”。

计量规则：

- 以代理响应给客户端的 body 字节数作为默认计费流量。
- HEAD、304、认证失败等无 body 响应按实际字节计 0 或小值，不估算文件大小。
- 流式响应需要在 body wrapper 中累计字节，不能为了统计把大文件读入内存。
- 统计维度至少包括：月份、日期、代理类型、HTTP 状态、是否命中配额拒绝。
- 月份边界使用配置时区，默认使用服务器本地时区或明确配置的 `quota.timezone`。

配置示例：

```toml
[quota]
enabled = true
monthly_gb = 500
timezone = "Asia/Taipei"
on_exceeded = "stop_proxy"
```

行为：

- 未达到配额：正常代理并累计流量。
- 达到配额：代理路径返回 `503 Service Unavailable` 或 `429 Too Many Requests`，错误 JSON 和 HTML 都明确说明月流量已用尽。
- 管理 API、健康检查、登录、静态控制台不受月流量封停影响。
- 管理员可以调整本月上限或手动解除封停；所有操作写审计日志。

验收标准：

- 配置月上限为很小值时，下载超过阈值后后续代理请求被拒绝。
- 新月份自动重新开启服务。
- 并发下载不会明显超卖配额；允许最后一个流式请求略微超过阈值，但后续请求必须停止。
- 统计数据重启后不丢失。

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

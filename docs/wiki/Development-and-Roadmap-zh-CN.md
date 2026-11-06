# 开发与路线图

本页面向贡献者。只想运行服务，请看[部署](Deployment-zh-CN)，无需安装开发工具链。

## 从源码构建

需要 Rust stable、Node.js 24 和 npm。服务端内嵌前端，因此先构建 Web，再编译 Rust：

```bash
cd web
npm ci
npm run build
cd ..
bash scripts/fetch-geoip.sh
cargo build --workspace --locked
```

Windows x64、ARM64 MSVC 目标通过 `.cargo/config.toml` 静态链接 C/C++ 运行库。
Windows 构建与测试应显式指定 `--target x86_64-pc-windows-msvc` 或
`--target aarch64-pc-windows-msvc`。服务端的 vendored OpenSSL 还需要
`scripts/setup-windows-build.ps1` 中的 Perl 构建工具。覆盖 Rust 编译参数时，
必须保留 `-C target-feature=+crt-static`。

CI 和发布流程执行 `python scripts/check-windows-runtime.py --architecture x64 path/to/program.exe`
（ARM64 使用 `arm64`），检查普通及延迟导入表并拒绝外部 CRT/OpenSSL DLL 依赖，
避免构建机预装的运行库掩盖问题。Windows 系统 DLL 仍是必需的。
修复计划从 1.7.3 起交付；现有 1.7.2 资产保持不变，其 WinGet 清单保留 VC++ 运行库依赖。
新产物除了依赖检查，还应在 Windows 上执行 `--version` 和客户端 `list` 验证。
回滚时使用完整的上一版发布包，并安装该版本声明的运行库依赖。

发布式 Linux 静态构建使用 `./build.sh`，还需要目标工具链、`musl-tools` 与联网获取固定校验和的 GeoIP 数据。
重复本地构建会核对输入指纹和产物摘要，仅复用一致的锁定依赖和前端；产物缺失或
被修改时自动重建。每次仍验证 GeoIP 校验和，并执行锁定 Cargo，将服务端与客户端
一起构建。原生构建输入和二进制未变时保留构建时间，否则使时间缓存失效。日志会
显示各阶段耗时。`./build.sh --force` 可强制重装依赖并重建前端；发布自动化仍可
显式设置 `BUILD_TIME`。

首次安装仍会看到 Umi 可选开发工具引入的三条废弃提示，不隐藏日志；未使用的
Max lint、DVA 和旧国际化插件已不再安装。React 19 的
Helmet/外部存储依赖和 Utoo 的 Less-loader 已定向锁定兼容版本。npm 11 安装脚本
策略仅允许已审查版本的 esbuild，拒绝 core-js/core-js-pure/es5-ext 的非必要
postinstall 提示。`npm audit` 仍是独立验证步骤；命中构建缓存不会重新执行在线
漏洞扫描。具体告警和性能验收边界见仓库的 `docs/testing/Local-Build-Cache.md`。
生产环境仍应按部署指南配置 HTTPS。

## 本地检查

浏览器测试首次运行前，在 `web` 下执行 `npx playwright install chromium`。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build && npm run test:e2e)
cargo build --locked -p mirrorproxy-server
./scripts/smoke-admin-api.sh target/debug/mirrorproxy-server
```

真实客户端协议验证：

```bash
bash scripts/smoke-clients.sh
```

它使用隔离的临时客户端目录，需要相应生态工具与公网访问，不修改开发者真实配置。
某个生态通过，不代表所有 OS 仓库和容器运行时都已验证。
依赖检查使用 `cargo audit --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0387`（仅限[已复核的 OIDC 公钥验签例外](https://github.com/inbjo/MirrorProxy/blob/main/SECURITY.md#scoped-dependency-advisory-exception--限定的依赖告警例外)）和 `web` 下的 `npm audit`。不带例外参数的 Rust 审计仍会报告这些未修复依赖，不能将其描述为已修复。引入私钥操作或修改 OIDC 依赖链前，必须重新评估。

## 进阶：仓库结构与交付

| 目录 | 内容 |
| --- | --- |
| `crates/server` | 服务、协议适配器、后台 API |
| `crates/client` | 本机改源客户端 |
| `crates/catalog` | 共用软件源目录和能力 |
| `web` | React 管理界面与浏览器测试 |

新增目标需同步目录、路由、配置、界面、客户端能力、文档和真实协议验证，不能只新增一个卡片。
发布流程见仓库 `AGENTS.md`、`.github/workflows` 和[发布维护者配置](Release-Maintenance-zh-CN)。

后续功能与缺陷通过 [Issues](https://github.com/inbjo/MirrorProxy/issues)、Milestones 和
[发布说明](https://github.com/inbjo/MirrorProxy/releases) 跟踪；计划中的功能要明确标注，不能当作当前版本能力。

[English](Development-and-Roadmap)

### Ant Design Pro 控制台

Web UI 使用 React 19、Umi 核心 4、Ant Design 6 和 ProComponents。在 `web` 目录
依次运行 `npm ci`、`npm test`、`npm run build` 和 `npm run test:e2e`。Umi 仍输出
到 `web/dist`，必须先构建前端再编译 Rust 服务端。`npm run dev` 使用 4173 端口；
连接真实本地 API 时设置 `MIRRORPROXY_DEV_API=http://127.0.0.1:3000`，保留原有
Cookie 和 CSRF 协议。生产环境仅需 Rust 服务，不增加 Node 服务。后台页面使用
`/admin/<module>` 路由，保留 `/admin`、`/login`、`/account` 和邀请链接兼容。
完整功能及验收清单见 `docs/testing/WebUI-Pro-Migration.md`。

前端安全门禁覆盖所有级别（`npm audit --audit-level=low`）。锁文件采用已修复的
间接依赖和 React Router 7，并适配 Umi 入口。移除未使用的旧 Node crypto 浏览器
实现；私有构建适配包被引用时会明确报错，生产构建也会拒绝包含该引用的分块。
浏览器原生 Web Crypto 和 WebAuthn 保持不变。Docker 在锁定依赖安装前复制该
适配包。

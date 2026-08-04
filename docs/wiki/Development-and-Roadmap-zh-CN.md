# 开发与路线图

工作区由 `crates/server`（`mirrorproxy-server`）、`crates/client`（`mirrorproxy`）、
`crates/catalog`（共享目录）和 `web`（React/Vite 管理后台）组成。任何新增目标都应同步
更新目录、服务路由/适配器、配置、门户、客户端能力、文档和 smoke 覆盖，避免“页面显示支持
但真实路径不可用”的漂移。

`./build.sh` 是 Linux 静态发布的标准路径，会先构建 Web，再下载经 SHA-256 固定的 GeoIP
数据并编译服务端与客户端。Release workflow 为客户端构建 Linux、macOS、Windows 资产，
为服务端构建 Linux 资产并附带校验和。

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build)
```

真实客户端 smoke 脚本使用隔离的临时包管理器目录，不会修改开发者真实配置：

```text
bash scripts/smoke-clients.sh
```

可选的 OS 客户端 smoke 与 Docker smoke 是更窄的环境验证，不应把语言生态的通过结果
表述成所有 OS 仓库均已验证。稳定标签会发布归档、DEB/RPM、Homebrew 与 WinGet manifest，
并部署经过签名的客户端 APT 仓库。

早期阶段性实施计划不再作为当前路线图，已从主分支移除，历史内容仍可通过 Git 记录查看。后续
功能、缺陷和版本安排通过项目的 [GitHub Issues](https://github.com/inbjo/MirrorProxy/issues)、
Milestones 与 [Release notes](https://github.com/inbjo/MirrorProxy/releases) 跟踪；文档只描述
当前已经提供的能力。

[English](Development-and-Roadmap)

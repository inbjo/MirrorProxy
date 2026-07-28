# 开发与路线图

工作区包含服务端、独立客户端、共享目录和 React/Vite Web 应用。先在 `web/` 构建
前端，再执行 Rust workspace 检查。`./build.sh` 是 Linux 静态发布的标准路径，构建
前会下载经过 SHA-256 固定的 GeoIP 数据。

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build)
```

真实客户端 smoke 脚本使用隔离的临时包管理器目录。新增适配器时必须同步路由、目录
元数据、配置、门户和 smoke 覆盖。当前路线图保存在主仓库的 `docs/plan.md` 和
`docs/next.md`。

[English](Development-and-Roadmap)

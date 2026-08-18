# Contributing to MirrorProxy / 参与贡献

Thank you for improving MirrorProxy. Small, focused changes with clear tests and
documentation are easiest to review. For a large feature or behavior change,
open an issue first so the scope and compatibility expectations can be agreed
before implementation.

感谢你改进 MirrorProxy。范围清晰、带有测试和文档的小型变更最容易审查。较大的功能或行为变更请先
创建 Issue，在实现前确认范围与兼容性要求。

Security vulnerabilities must be reported privately according to
[SECURITY.md](SECURITY.md), not through a public issue or pull request.

安全漏洞必须按照 [SECURITY.md](SECURITY.md) 私密报告，请勿提交公开 Issue 或 Pull Request。

## Repository layout / 仓库结构

- `crates/server`: the `mirrorproxy-server` service, administration API, and
  proxy adapters.
- `crates/client`: the cross-platform `mirrorproxy` source-management client.
- `crates/catalog`: shared source catalog and capability definitions.
- `web`: the React/Vite administration console embedded in the server binary.
- `docs/wiki`: canonical English and Simplified Chinese Wiki sources.
- `scripts`: build, packaging, installation, and smoke-test automation.

新增或修改软件源通常需要同步检查目录定义、服务端路由/适配器、配置、Web 控制台、客户端能力、
中英文文档和 smoke 测试，避免不同入口的能力不一致。

## Development setup / 开发环境

Install the stable Rust toolchain and Node.js 24. Build the web console before
building the server because its generated `web/dist` assets are embedded in the
server binary.

请安装 Rust stable 与 Node.js 24。服务端会嵌入 `web/dist`，因此首次构建服务端前需要先构建 Web。

```bash
cd web
npm ci
npm run build
cd ..
cargo build --workspace --locked
```

Do not commit generated `web/dist`, dependency directories, local databases,
credentials, or downloaded GeoIP databases.

请勿提交生成的 `web/dist`、依赖目录、本地数据库、凭据或下载的 GeoIP 数据库。

## Validation / 验证

Run the checks relevant to your change. Before requesting review for a Rust or
cross-stack change, run the full local quality gate:

请运行与变更有关的检查。Rust 或跨端变更在请求审查前应运行完整本地质量门禁：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build)
cargo build --locked -p mirrorproxy-server
./scripts/smoke-admin-api.sh target/debug/mirrorproxy-server
```

For proxy protocol changes, also run `bash scripts/smoke-clients.sh`. The script
uses isolated temporary package-manager homes, but it requires network access
and several ecosystem clients. Add or update focused tests whenever behavior
changes; documentation-only changes do not require the full runtime matrix.

代理协议发生变化时还应运行 `bash scripts/smoke-clients.sh`。该脚本使用隔离的临时包管理器目录，
但需要网络和多个生态客户端。行为变更必须补充或更新针对性测试；纯文档变更无需运行完整运行时矩阵。

## Pull requests / Pull Request 要求

- Keep each pull request focused on one problem and explain the user-visible
  behavior, motivation, and compatibility impact.
- Link the related issue when one exists. Include reproduction steps for fixes
  and screenshots for visible console changes.
- Keep English and Simplified Chinese documentation aligned when both versions
  describe the changed behavior.
- List the exact validation commands run and any checks that could not be run.
- Preserve backward compatibility unless the breaking change was discussed and
  documented explicitly.
- Never include secrets, private production data, or unrelated generated files.

- 每个 PR 只解决一个明确问题，并说明用户可见行为、动机和兼容性影响。
- 如有对应 Issue 请关联；缺陷修复需提供复现步骤，可见的控制台变更需附截图。
- 同一行为同时存在中英文文档时，两者必须同步更新。
- 列出实际执行的验证命令，并明确说明未能执行的检查。
- 除非破坏性变更已事先讨论并明确记录，否则应保持向后兼容。
- 禁止包含秘密、生产私有数据或无关的生成文件。

By contributing, you agree that your contribution is licensed under the
project's [MIT License](LICENSE).

提交贡献即表示你同意按本项目的 [MIT License](LICENSE) 授权该贡献。

# Security Policy / 安全策略

## Supported versions / 支持版本

Security fixes are provided for the latest released version of MirrorProxy.
Older releases should be upgraded before a report is evaluated or a fix is
applied.

MirrorProxy 仅为最新正式版本提供安全修复。报告问题或应用修复前，请先升级已停止维护的旧版本。

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older releases | No |

## Scoped dependency advisory exception / 限定的依赖告警例外

CI excludes only the two reviewed advisory IDs below. All other vulnerabilities
and yanked-version warnings remain outside these exceptions. An unfiltered
audit still reports the unpatched dependencies; an excepted audit must not be
described as a dependency fix or as exception-free. Review these exceptions
when upstream publishes compatible fixes or when OIDC/cryptographic usage changes.

CI 只排除下文经过复核的两个告警 ID，其他漏洞和撤回版本告警均不在例外范围内。
不带例外参数的审计仍会报告这些未修复依赖，不能将带例外的审计描述为依赖已修复或
无例外零告警。上游提供兼容修复版，或 OIDC/密码学用途发生变化时，必须重新评估。

### RSA private-key timing / RSA 私钥运算计时

Reviewed on 2026-10-21: [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)
remains in the dependency graph as `openidconnect 4.0.1 -> rsa 0.9.10`.
Upstream has no patched version. The advisory concerns RSA private-key timing
leaks; MirrorProxy uses this dependency only to verify OIDC provider signatures
with public keys. It does not use it to decrypt data or sign with RSA private
keys. This affected private-key path is therefore not applicable to the current
application.

The exception for this path is `RUSTSEC-2023-0071`. Reassess it before introducing
any RSA private-key operation through this dependency.

2026-10-21 复核：依赖链 `openidconnect 4.0.1 -> rsa 0.9.10` 仍包含上述告警，
上游尚无修复版。告警涉及 RSA 私钥运算的计时泄露；MirrorProxy 仅使用该依赖对 OIDC
提供方的签名做公钥验证，不使用它解密或执行 RSA 私钥签名，因此当前应用不涉及受影响路径。

此路径的例外为 `RUSTSEC-2023-0071`。通过此依赖引入任何 RSA 私钥运算前必须重新评估。

### OIDC EC public-key verification / OIDC 椭圆曲线公钥验签

Reviewed on 2026-10-24: [RUSTSEC-2026-0387](https://github.com/rustsec/advisory-db/blob/main/crates/crypto-bigint/RUSTSEC-2026-0387.md)
affects `crypto-bigint` operations on secret integers. The current chain is
`openidconnect 4.0.1 -> p256/p384 0.13 -> elliptic-curve 0.13.8 -> crypto-bigint 0.5.5`.
The patched `crypto-bigint >=0.7.6` is not compatible with that chain; the latest
published `openidconnect` release still requires the old curve implementations.

`oidc_exchange` in `crates/server/src/oauth.rs` calls the SDK's ID-token verifier.
For ES256/ES384, `openidconnect`'s `core::crypto::verify_ec_signature` constructs a
`p256`/`p384` `VerifyingKey` from the provider's public JWK `x` and `y`, then verifies
the message/signature. No EC private scalar, ECDH operation, signing key or
client secret is supplied to these integer operations. The sensitive-operand
path described by the advisory is not applicable to this verification-only use.
Other uses of EC private keys elsewhere (such as TLS/ACME) do not use this chain.

The precise CI exception is `RUSTSEC-2026-0387`; it does not patch the library or
permit secret-integer use. The OIDC regression test
`oidc_ec_public_keys_verify_signatures_and_reject_invalid_tokens` covers ES256
and ES384 verification, corrupt signatures, algorithm confusion, issuer,
audience, nonce and expiration checks. Mock-provider private keys are generated
ephemerally with `ring` only in the test, never persisted or passed to the OIDC
SDK. These tests protect the verification contract; the applicability assessment
also relies on the reviewed call chain and must be repeated for changes to it.

2026-10-24 复核：该告警涉及秘密整数的时序泄漏，当前依赖链如上。修复版
`crypto-bigint >=0.7.6` 与该依赖链不兼容，最新发布的 `openidconnect` 仍使用旧版曲线库。
MirrorProxy 的 `oidc_exchange` 只调用 ID Token 验证器；ES256/ES384 验签从身份提供方
公开 JWK 的 `x/y` 构造公钥并验证消息与签名，不向这些整数运算传入 EC 私钥、ECDH
操作、签名密钥或客户端密钥。因此，通告描述的秘密操作数路径不适用于当前用途。
TLS/ACME 等其他 EC 私钥用途不走这条依赖链。

精确例外为 `RUSTSEC-2026-0387`，不表示库已修复，也不允许将此依赖用于秘密整数。
回归测试覆盖两种曲线验签、损坏签名、算法混淆、issuer、audience、nonce 与过期检查；
测试身份提供方的临时私钥仅由 `ring` 在测试中生成，不持久化，也不传给 OIDC SDK。
测试用于保护验签契约，适用性判断还依赖人工核对的调用边界，调用链变化时必须重审。

Current CI command / 当前 CI 命令：

```sh
cargo audit --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0387
```

## Frontend build dependency status / 前端构建依赖状态

Reviewed on 2026-11-02: modern build-tool consumers use `glob 14.0.3` and
`minimatch 11.0.4`, addressing GHSA-3g3m-2fjr-6pw4 and GHSA-g3hv-75wg-m249
in those paths. Umi's `fork-ts-checker-webpack-plugin 8.0.0` still requires the
callable CommonJS API of `minimatch 3.1.5`; even upstream checker 9.1.0 retains
that dependency. Replacing it with minimatch 11 breaks issue and watch filtering.
The minimatch override therefore applies only to modern consumers. The legacy
path remains reported by unfiltered npm audit. With explicit user approval on
2026-11-02, the CI audit permits only GHSA-g3hv-75wg-m249 at moderate severity
for `node_modules/minimatch` version 3.1.5 with checker version 8.0.0, including
findings propagated from that cause. Different advisories, versions, paths,
or elevated severity still fail. This is an accepted build-tool risk, not a
patched dependency. Remove the exception when the checker provides a compatible
migration. The existing low-severity deepmerge exception is unchanged.

2026-11-02 复核：现代构建工具调用链已升级至 `glob 14.0.3`、`minimatch 11.0.4`，
修复这些路径中的上述两项告警。Umi 的类型检查插件仍依赖 `minimatch 3.1.5` 的 CommonJS
函数接口，上游最新版检查插件 9.1.0 也尚未迁移；直接替换为 minimatch 11 会破坏问题筛选与
文件监听。因此只覆盖现代调用链。旧路径仍会被原始 npm audit 报告。经用户于 2026-11-02 明确批准，CI 审计仅对
检查插件 8.0.0 所用 `node_modules/minimatch` 3.1.5 的 GHSA-g3hv-75wg-m249 中危告警
及其传播项设置例外。其他公告、版本、安装路径或更高严重等级仍会失败。这是接受构建工具风险，
并非依赖已修复；上游提供兼容迁移后须移除例外。原有 deepmerge 低危例外保持不变。

## Dependabot resolution status / Dependabot 依赖解析状态

Rechecked on 2026-11-02 after Dependabot update #1673348505 reported
`security_update_not_possible`: npm still publishes deepmerge 4.3.1 as latest,
and fork-ts-checker-webpack-plugin 9.1.0 still requires minimatch `^3.0.4`.
The installed checker 8.0.0 calls minimatch as a CommonJS function; minimatch 11
exports an object. The automatic updater therefore cannot resolve a compatible
security upgrade for these paths. Its failed update does not clear the alerts
or patch the dependencies. The existing narrowly scoped CI audit exceptions
above continue to apply. Review again when upstream dependencies change.

2026-11-02 复核：Dependabot 更新 #1673348505 报告
`security_update_not_possible`。npm 最新 deepmerge 仍为 4.3.1；最新版类型检查插件
9.1.0 仍依赖 minimatch `^3.0.4`。当前插件 8.0.0 使用 CommonJS 函数接口，
minimatch 11 改为对象导出，因此自动更新器无法解析兼容的安全升级。
此更新任务失败不表示告警已清除或依赖已修复；继续适用上文的限定审计例外，
上游依赖变化后应重新复核。

## Bundled SQLite WAL fix / 内嵌 SQLite WAL 修复

On 2026-11-14, the bundled SQLite dependency was updated to `libsqlite3-sys`
0.39.1 (SQLite 3.54.1), fixing
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459.html).
This advisory is not excepted. SQLx is temporarily pinned to upstream commit
`10462fa4414c6c7f1ed1d39bb31678e3b726878c` (PR #4502), because published SQLx
0.9.0 excludes the patched driver. Replace the Git pin with a stable release
once it permits `libsqlite3-sys` 0.39 or newer. Builds require Rust 1.94 or newer
and must retain locked dependency resolution. This changes no database schema;
back up the database using the existing backup command before upgrading.

2026-11-14：内嵌 SQLite 已升级到 `libsqlite3-sys` 0.39.1（SQLite 3.54.1），
修复 RUSTSEC-2026-0459，未为该告警添加例外。由于已发布的 SQLx 0.9.0
不允许使用修复版驱动，暂时固定到上游 PR #4502 的完整提交
`10462fa4414c6c7f1ed1d39bb31678e3b726878c`。稳定版支持 0.39 或更新的驱动后，
应将 Git 固定提交替换为稳定版。构建需要 Rust 1.94 或更新版本，并继续使用锁定依赖。
此更新不改变数据库结构；升级前请使用现有备份命令备份数据库。

## Reporting a vulnerability / 报告安全漏洞

Do not disclose suspected vulnerabilities in a public issue, discussion, or
pull request. Use GitHub's
[private vulnerability reporting](https://github.com/inbjo/MirrorProxy/security/advisories/new)
to send the maintainers a confidential report. If that form is unavailable,
email `2345@mail.com` with the subject `MirrorProxy security report`.

请勿在公开 Issue、Discussion 或 Pull Request 中披露疑似漏洞。请优先通过 GitHub
[私密漏洞报告](https://github.com/inbjo/MirrorProxy/security/advisories/new)联系维护者；若该入口不可用，
请发送邮件至 `2345@mail.com`，主题注明 `MirrorProxy security report`。

Include the affected version and configuration, impact, reproduction steps or
proof of concept, and any suggested mitigation. Remove credentials, access
tokens, private keys, personal data, and production logs that are not necessary
to reproduce the issue.

报告应包含受影响版本与配置、影响范围、复现步骤或 PoC，以及可选的缓解建议。请删除与复现无关的
凭据、访问令牌、私钥、个人信息和生产日志。

We aim to acknowledge a report within 7 days. We will coordinate validation,
remediation, release timing, and public disclosure with the reporter. Please do
not publish details until a fix is available or a disclosure date has been
agreed.

我们会尽量在 7 天内确认收到报告，并与报告者协调验证、修复、发布和公开披露时间。在修复版本可用
或双方约定披露日期之前，请勿公开漏洞细节。

## Scope / 范围

Reports about authentication or authorization bypasses, credential or secret
exposure, unsafe proxy routing, cache isolation, request smuggling, remote code
execution, and release or update integrity are especially useful. General bugs,
feature requests, and support questions belong in
[GitHub Issues](https://github.com/inbjo/MirrorProxy/issues).

我们尤其关注身份认证或授权绕过、凭据或秘密泄露、不安全的代理路由、缓存隔离、请求走私、远程代码
执行，以及发布或更新完整性问题。普通缺陷、功能建议和使用问题请提交到
[GitHub Issues](https://github.com/inbjo/MirrorProxy/issues)。

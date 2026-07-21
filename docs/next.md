# MirrorProxy 下一阶段实施计划

## 1. 阶段定位

下一阶段在现有单实例镜像代理、SQLite 管理后台、全局流量统计和月度配额基础上，增加账号体系、用户子域名、用户组和分级流量管理。

核心目标：

- 管理后台使用独立 `/admin` 入口，与普通用户门户、公开说明页完全分开。
- 管理员使用账号密码登录，并支持 Passkey 登录。
- 普通用户通过 OAuth2/OIDC、邮件验证码或 Magic Link 登录。
- 管理员可以关闭开放注册，通过邮件邀请用户注册。
- 每个普通用户获得独立的 Sqids 子域名。
- 包管理器使用 `accounting_only` 模式：不验证 Token，只通过用户子域名识别流量归属。
- 用户可以查看自己的流量和配额，并在怀疑子域名泄漏时重新生成子域名。
- 管理员可以配置注册策略、企业邮箱域名、OAuth/OIDC、SMTP、用户组和流量上限。
- 服务端可以通过单一全局 HTTP、HTTPS 或 SOCKS5 代理访问所有镜像上游。

本阶段不实现 LDAP，但在身份 Provider 模型中保留扩展位置。

## 2. 入口、身份和会话隔离

### 2.1 页面入口

页面按用途分成三套入口：

- `/`：公开使用说明、镜像源目录和服务状态。
- `/login`、`/account`：普通用户登录和个人控制台。
- `/admin`：独立的管理员登录和管理后台。

`/admin` 使用独立路由树、布局、登录页和会话，不在普通用户导航中暴露管理功能。前端可以继续共享 React 组件和构建产物，但身份状态、API 客户端和权限判断必须分开。

建议 API 路径：

- `/api/auth/*`：普通用户 OAuth/OIDC 和邮件登录。
- `/api/account/*`：普通用户资料、子域名和流量统计。
- `/admin/api/auth/*`：管理员密码和 Passkey 登录。
- `/admin/api/*`：用户、邀请、Provider、SMTP、用户组、配额和审计。

现有 `/api/admin/*` 在迁移期保留兼容，前端切换完成并经过一个版本后再移除。

### 2.2 身份模型分离

管理员和普通用户使用独立身份模型：

- `admin_users`：后台管理员账号，不自动获得用户子域名。
- `users`：镜像服务普通用户，不自动获得后台管理权限。
- 普通用户不能通过 OAuth、邮件邀请或用户组配置升级为管理员。
- 如管理员也需要使用个人镜像子域名，应另外创建普通用户账号。

这样可以避免 OAuth 账号绑定、企业域名自动注册或邮件邀请错误地扩大后台权限。

### 2.3 Cookie 和会话

- 管理员与普通用户使用不同 Cookie 名称和不同服务端 Session 表。
- 管理员 Session Cookie 必须设置 `HttpOnly`、`Secure`、`SameSite=Strict`。
- 普通用户 Session Cookie 至少设置 `HttpOnly`、`Secure`、`SameSite=Lax`。
- Cookie 不设置为 `.example.com`，只允许主域名使用。
- 用户 Sqids 子域名不能收到登录 Cookie。
- 登录、管理和个人控制台只允许在配置的主域名访问。
- 用户子域名访问 `/admin`、`/login`、`/account` 时不执行认证流程，应拒绝或跳转到主域名。
- Session 数据库只保存 Token 哈希，不保存原始 Token。
- 修改管理员密码、禁用账号或执行“退出所有设备”后，立即撤销对应 Session。

## 3. 管理员认证

### 3.1 账号密码登录

管理员使用用户名和密码登录 `/admin`。

要求：

- 保留当前首次启动管理员和随机初始密码逻辑。
- 支持多个管理员账号。
- 密码使用 Argon2id 哈希，绝不明文存储。
- 登录错误使用统一提示，不区分用户名不存在或密码错误。
- 按 IP 和用户名实施限速及短期锁定。
- 成功和失败登录均写安全审计，但不记录密码。
- 管理员可以修改自己的密码。
- 超级管理员可以创建、禁用其他管理员并重置其密码。
- 不能删除或禁用最后一个有效超级管理员。

管理员密码策略：

- 禁止空密码。
- 默认最少 12 个字符。
- 不强制复杂字符组合，但拒绝常见弱密码和与用户名相同的密码。
- 管理员手动设置密码时执行管理员密码策略，不复用普通用户登录逻辑。

保留一个本地 break-glass 超级管理员。SMTP、OAuth 或 Passkey 故障时，仍可使用密码或服务端 CLI 恢复管理权限。

### 3.2 Passkey 登录

使用成熟开源库 `webauthn-rs` 实现 WebAuthn/Passkey，不自行实现协议。

支持：

- Windows Hello。
- Touch ID、Face ID 和系统 Passkey。
- Android/Chrome Passkey。
- YubiKey 等安全密钥。
- 同一管理员登记多个 Passkey。
- 为 Passkey 设置名称，例如“办公室 YubiKey”“MacBook Touch ID”。
- 查看创建时间和最后使用时间。
- 删除遗失或不再使用的 Passkey。

默认策略：

1. 管理员首次仍使用用户名和密码登录。
2. 登录后台后可以登记一个或多个 Passkey。
3. 后续可以选择“账号密码登录”或“使用 Passkey 登录”。
4. 管理员可启用“除 break-glass 账号外必须使用 Passkey”的安全策略。
5. 启用强制策略前，至少要存在两个有效 Passkey，或确认 CLI 恢复方式可用。

Passkey 安全要求：

- RP ID 固定为配置的主域名，例如 `mirror.example.com`。
- Origin 固定为完整 HTTPS Origin，例如 `https://mirror.example.com`。
- 不接受通配符用户子域名作为 Origin。
- 注册和认证 Challenge 必须使用密码学安全随机数。
- `PasskeyRegistration` 和 `PasskeyAuthentication` 挑战状态保存在服务端，设置短有效期且只能使用一次。
- 完成请求必须与发起请求的管理员、Session、Challenge 和 Origin 匹配。
- Passkey 的用户 Handle 使用独立随机 UUID，不使用用户名或数据库自增 ID。
- 登记、删除 Passkey 前要求最近完成过管理员密码或 Passkey 验证。
- 新 Passkey 登记、Passkey 删除和强制策略变化写安全审计。

建议新增表：

- `admin_passkeys`
- `admin_webauthn_challenges`

管理员丢失全部 Passkey 时可以使用密码登录；密码也丢失时，通过本机 CLI 重置密码，不依赖邮件恢复。

## 4. 普通用户登录

普通用户不实现本地密码注册，支持以下登录方式：

- GitHub、GitLab、Gitee 等 OAuth2 平台。
- Google、Microsoft、Keycloak、Authentik、Okta 等标准 OIDC 平台。
- 通用 OAuth2 Provider。
- 通用 OIDC Discovery Provider。
- 邮箱验证码。
- 邮件 Magic Link。
- 管理员邮件邀请。

技术选型：

- OAuth2 使用开源 `oauth2-rs`。
- OIDC 使用开源 `openidconnect-rs`。
- OAuth2/OIDC 使用 Authorization Code Flow。
- 支持时一律启用 PKCE。
- 必须校验 `state`；OIDC 还必须校验 `nonce`、issuer、audience、签名和过期时间。
- OAuth/OIDC HTTP 客户端禁止自动跟随重定向，降低 SSRF 风险。
- SMTP 使用开源 `lettre` 异步发送。

内置 Provider 模板：

- GitHub。
- GitLab。
- Gitee。
- Google。
- Microsoft。
- Keycloak/Authentik 通用 OIDC。

管理员只需填写 Client ID、Client Secret 和必要的租户信息。通用模式允许配置授权端点、Token 端点、UserInfo 端点、scope 和字段映射。

### 4.1 身份绑定

- 外部身份以 `(provider_id, provider_subject)` 唯一识别。
- 昵称、用户名和可修改邮箱不能作为外部身份主键。
- 只有 Provider 明确返回已验证邮箱时，才允许按邮箱关联已有用户。
- 按邮箱自动关联必须由管理员显式开启。
- 未验证邮箱不得用于绕过邀请或企业邮箱域名限制。
- 不满足自动关联条件时，用户必须登录已有账号后手动绑定。
- 解绑最后一种可用登录方式前必须先绑定另一种登录方式。

## 5. 注册和企业准入策略

提供以下注册模式：

- `invite_only`：默认模式，只有收到邀请的邮箱可以注册。
- `domain_allowlist`：指定企业邮箱域名的已验证用户可以自动注册。
- `open`：任何完成邮箱验证的用户都可以注册。
- `disabled`：禁止创建新用户，只允许已有用户登录。

管理员可以配置：

- 允许注册的邮箱域名，例如 `example.com`。
- 是否拒绝临时邮箱服务。
- 哪些 OAuth/OIDC Provider 可以创建新用户。
- 新用户默认计费组和默认月流量上限。
- 是否允许通过已验证邮箱自动关联第三方身份。
- 是否允许用户自行绑定和解绑第三方身份。

在 `invite_only` 模式下：

1. 管理员输入邮箱、显示名称、计费组和个人配额。
2. 系统创建待接受邀请并发送邮件。
3. 用户通过邀请链接进入登录页。
4. 用户使用邮件验证码、Magic Link 或符合策略的 OAuth/OIDC 完成身份验证。
5. 验证邮箱必须与邀请邮箱一致。
6. 邀请成功接受后才创建或启用正式用户。

邀请支持撤销、重新发送、过期和审计。

## 6. SMTP 和邮件登录

管理后台增加邮件服务器设置：

- SMTP Host 和 Port。
- STARTTLS、SMTPS 或禁用加密。
- 用户名和密码。
- From Name 和 From Address。
- 站点名称和邮件模板。
- 邀请、验证码和 Magic Link 有效期。
- 发送测试邮件。

安全要求：

- SMTP 密码和 OAuth Client Secret 不返回给前端。
- 敏感配置使用 `MIRRORPROXY_MASTER_KEY` 加密后写入 SQLite。
- 日志和审计不得包含密码、Client Secret、验证码或完整登录链接。
- 未配置主密钥时，不允许在数据库中保存可恢复的敏感凭据，并在后台明确提示。

邮件验证码和 Magic Link 共用一次性凭证模型：

- 数据库只保存 Token 或验证码哈希。
- 默认 10 分钟过期。
- 成功使用后立即失效。
- 验证码限制错误尝试次数。
- 按 IP、邮箱和实例实施发送频率限制。
- 邮件通过 SQLite Outbox 和后台任务发送。
- 发送失败支持有限次数指数退避重试。
- 管理后台可以查看脱敏后的发送状态和失败原因。

建议新增表：

- `email_login_tokens`
- `email_invitations`
- `email_outbox`

## 7. 用户 Sqids 子域名

### 7.1 生成方式

不直接编码数据库自增 ID：

1. 为用户生成随机、不可预测的正整数 `public_number`。
2. 使用 `sqids-rust` 转换为适合域名的短标识。
3. 将结果持久化为唯一 `routing_id`。
4. 生成类似以下地址：

```text
k3m8q2d7x9ab.mirror.example.com
```

建议 Sqids 最短长度为 12。Sqids 不是加密算法，降低枚举风险主要依靠随机 `public_number`，不能把数据库自增 ID 直接交给 Sqids。

### 7.2 更换子域名

用户控制台提供“重新生成子域名”：

- 系统重新生成随机 Sqids，不允许输入任意文字。
- 旧子域名立即失效。
- 新请求只计入新子域名。
- 默认设置 24 小时冷却期，管理员可以调整或绕过。
- 用户和管理员的更换操作都写安全审计。
- 用户被禁用后，其子域名立即停止服务。

由于使用 `accounting_only`，任何知道该子域名的人都可以使用并消耗该用户流量。用户界面必须明确提示这一点，并提供醒目的更换入口。

### 7.3 域名和反向代理要求

- 配置 `*.mirror.example.com` 通配符 DNS。
- 配置覆盖通配符域名的可信 TLS 证书。
- 只接受匹配 `base_domain` 的 Host。
- 只信任来自已配置反向代理的 `Forwarded` 或 `X-Forwarded-Host`。
- 保留 `www`、`admin`、`api` 等系统名称，不能分配给用户。
- 未知、已更换或被禁用的子域名返回统一错误，不暴露用户状态。

## 8. 包管理器访问模式

### 8.1 `public`

- 主域名代理路径继续可用。
- 主域名流量只计入全局统计。
- 用户子域名流量同时计入用户、计费组和全局统计。

### 8.2 `subdomain_required`

- 主域名只提供网站、登录、个人控制台和 `/admin`。
- 主域名上的包代理路径被拒绝。
- 包管理器必须使用用户 Sqids 子域名。
- 不需要 Token、Basic Auth 或 Bearer Auth。
- 未知或失效子域名返回统一错误。

管理员可以在 `/admin` 切换访问模式。切换到 `subdomain_required` 前必须显示通配符 DNS、TLS、主域名和反向代理配置检查结果。

## 9. 用户组和流量配额

保留现有全局月流量限制，并增加：

- 用户个人月流量上限。
- 计费组共享月流量上限。
- 用户自定义月流量上限。
- 不限流量选项。

用户可以加入多个权限或标签组，但只能指定一个计费组，避免同一次下载重复扣减多个共享配额。

配额优先级：

```text
全局剩余配额
    ∩ 计费组剩余配额
    ∩ 用户个人剩余配额
```

任意一级达到上限，该用户后续代理请求都会被拒绝。全局、计费组和用户配额必须在同一个 SQLite 事务中完成原子预留，继续沿用现有流式计量和预留窗口机制。

用户控制台展示：

- 今日和本月流量。
- 本月请求数和错误数。
- 个人配额使用量和剩余量。
- 所属计费组及组配额剩余量。
- 按镜像类型统计的流量。
- 最近 30 天趋势。
- 当前子域名和最近更换时间。

默认不向普通用户展示完整请求 URL。

## 10. 管理后台功能

独立 `/admin` 后台包含：

- 管理员登录和 Passkey 登录。
- 管理员账号、密码、Passkey 和 Session 管理。
- 用户列表、搜索、禁用和删除。
- 用户身份绑定、流量、配额、用户组和子域名详情。
- 邮件邀请创建、撤销、重发和状态查询。
- 用户组、默认配额、共享配额和成员管理。
- OAuth2/OIDC Provider 配置和连通性检查。
- SMTP、发件人、模板和测试邮件。
- 注册模式和企业邮箱域名白名单。
- `public` 与 `subdomain_required` 访问模式。
- 登录、邀请、身份绑定、子域名轮换、配额和权限变更审计。

高风险操作要求重新验证管理员密码或 Passkey：

- 新增或删除管理员。
- 重置其他管理员密码。
- 删除 Passkey。
- 修改主域名、RP ID 或 Origin。
- 查看或替换敏感配置。
- 修改注册模式和企业邮箱域名。
- 清空统计或审计数据。

## 11. 数据库模型

建议新增或扩展：

- `admin_users`
- `admin_sessions`
- `admin_passkeys`
- `admin_webauthn_challenges`
- `users`
- `user_identities`
- `user_sessions`
- `auth_providers`
- `email_login_tokens`
- `email_invitations`
- `email_outbox`
- `groups`
- `group_members`
- `user_routing_ids`
- `user_traffic_daily`
- `user_traffic_monthly`
- `group_traffic_monthly`
- `user_quota_overrides`
- `security_audit_log`

关键约束：

- 外部身份 `(provider_id, provider_subject)` 唯一。
- 规范化用户邮箱唯一。
- 有效 `routing_id` 全局唯一。
- 同一用户只能有一个有效 `routing_id`。
- Passkey Credential ID 唯一。
- WebAuthn Challenge 有效期短、只能使用一次。
- Session、Token、验证码和邀请凭证只保存哈希。
- 用户删除优先采用软删除，保留必要汇总统计和安全审计。

## 12. 明确不做

本阶段不实现：

- LDAP 登录和 LDAP 同步。
- 包管理器 Token、Basic Auth 或 Bearer Auth。
- 普通用户本地密码注册和密码找回。
- 普通用户成为后台管理员的角色转换。
- 用户自定义任意文字子域名。
- 多组织 SaaS 隔离。
- 充值、付费套餐和按请求计费。
- 将 Sqids 当作安全凭据或加密方式。
- 根据普通用户 OAuth Access Token 代理第三方私有包。
- 强制只允许特定品牌硬件安全密钥的 Attestation 策略。

LDAP 只在 Provider 类型和数据库迁移设计中预留扩展位置，不添加依赖、配置页面或未完成入口。

## 13. 全局上游代理

### 13.1 范围

为 MirrorProxy 服务端增加一个可选的全局上游代理，应用于所有镜像 adapter 的出站请求，包括：

- GitHub、Release 重定向和最终下载地址。
- npm、PyPI、Cargo、Go、Composer、Maven 等包元数据和文件请求。
- Docker/OCI Registry、Bearer Token 获取、manifest 和 blob 请求。
- 操作系统仓库及其他已经接入统一 `reqwest::Client` 的上游。

本任务只支持一个全局代理，不实现以下能力：

- 不按 adapter 或上游域名选择不同代理。
- 不实现代理池、轮询、故障切换和自动测速。
- 不让客户端提交的请求决定上游代理。
- 不把 OTLP、SMTP、OAuth/OIDC 等控制面请求自动纳入镜像上游代理。

### 13.2 支持协议

- `http://proxy.example.com:8080`：HTTP 代理，HTTPS 上游使用 CONNECT。
- `https://proxy.example.com:8443`：使用 TLS 连接 HTTP 代理。
- `socks5://127.0.0.1:1080`：SOCKS5，本机解析上游 DNS。
- `socks5h://127.0.0.1:1080`：SOCKS5，由代理解析上游 DNS，推荐用于避免本地 DNS 泄漏。
- HTTP Basic 代理认证。
- SOCKS5 用户名密码认证。
- `no_proxy` 主机排除列表。

SOCKS5 通过 `reqwest` 的 `socks` feature 实现，不引入自定义 SOCKS 协议代码。

### 13.3 配置

建议 TOML：

```toml
[outbound_proxy]
enabled = false
url = ""
no_proxy = ["127.0.0.1", "localhost"]

# 可选代理认证。配置文件必须限制读取权限。
# username = "proxy-user"
# password = "replace-with-secret"
```

建议环境变量：

```text
MIRRORPROXY_OUTBOUND_PROXY_ENABLED
MIRRORPROXY_OUTBOUND_PROXY_URL
MIRRORPROXY_OUTBOUND_PROXY_USERNAME
MIRRORPROXY_OUTBOUND_PROXY_PASSWORD
MIRRORPROXY_OUTBOUND_PROXY_NO_PROXY
```

配置规则：

- `enabled = false` 时所有镜像上游直接连接。
- `enabled = true` 时 `url` 必填，并且只能使用明确支持的代理 scheme。
- 代理 URL 不允许包含用户名和密码，认证信息必须使用独立字段或环境变量。
- `no_proxy` 使用逗号分隔环境变量，并在 TOML 中使用字符串数组。
- 显式 MirrorProxy 配置优先于通用 `HTTP_PROXY`、`HTTPS_PROXY` 和 `ALL_PROXY`。
- 为保证部署行为确定，MirrorProxy 不依赖系统代理自动发现。
- 代理配置在启动时构建共享 `reqwest::Client`，修改后需要重启服务。
- 管理 API 和公开配置 API 不得返回代理用户名或密码。
- SQLite 运行时配置和配置审计不得保存代理密码。
- 日志只允许记录代理 scheme、脱敏主机和端口，不记录认证信息。

### 13.4 实现要求

- 在 `Config` 中增加 `OutboundProxyConfig`，提供默认值、环境变量覆盖和严格校验。
- 给服务端 `reqwest` 依赖启用 `socks` feature。
- 统一在 `build_router` 创建共享 `reqwest::Client` 时应用 `reqwest::Proxy::all`。
- 配置用户名密码时，通过 `Proxy` API 设置认证，不把凭据拼接到日志可见 URL。
- 为 Proxy 配置应用 `NoProxy` 排除列表。
- 所有生产 adapter 继续复用 `AppState.client`，禁止某个 adapter 私自创建绕过代理的新客户端。
- OCI Bearer Token、重定向后的 CDN 请求和 Maven fallback 必须继续使用同一个代理 Client。
- 无效 scheme、缺失 URL、空用户名、半套认证信息在启动时返回明确配置错误。
- 代理网络不可达时保持服务运行，但对应上游请求返回统一的 Bad Gateway，并记录脱敏错误。
- `/healthz` 只表示 MirrorProxy 进程和本地数据库健康，不因外部代理暂时不可达而整体失败。

### 13.5 验收标准

- 未启用代理时，现有上游请求行为保持不变。
- 配置 HTTP 代理后，HTTP 和 HTTPS 上游请求均经过该代理。
- 配置 `socks5://` 后，上游请求通过 SOCKS5，DNS 在本机解析。
- 配置 `socks5h://` 后，上游主机名交给 SOCKS5 代理解析。
- HTTP 和 SOCKS5 用户名密码正确时可以请求，错误时返回明确的代理连接或认证错误。
- `no_proxy` 中的目标保持直连，其他目标仍走全局代理。
- GitHub 重定向、OCI Token、blob、Maven primary/fallback 不会绕过代理。
- 代理密码不会出现在配置 API、SQLite、日志、指标和审计中。
- README、README_CN、`config.example.toml` 和 `compose.yaml` 包含一致的配置说明。

### 13.6 测试计划

- 配置单元测试：默认关闭、合法 scheme、非法 scheme、缺失 URL、认证字段完整性和环境变量覆盖。
- Client 构建测试：HTTP、HTTPS、SOCKS5、SOCKS5H 和 `no_proxy` 转换正确。
- HTTP 代理集成测试：本地 mock proxy 能观察到 HTTP 请求和 HTTPS CONNECT。
- SOCKS5 集成测试：本地 mock SOCKS5 服务能观察目标主机和端口，并验证本地/远端 DNS 差异。
- 认证测试：正确和错误的 HTTP/SOCKS5 凭据。
- 路由测试：普通 adapter、GitHub redirect、OCI Token、OCI blob 和 Maven fallback 均经过代理。
- 回归测试：`cargo test --workspace`、Clippy、格式检查和现有 client smoke 全部通过。

## 14. 实施轮次和 Commit

确认计划后分七个可独立验证的阶段实施，每阶段完成测试后单独提交。

### 14.1 全局上游代理

状态：已完成。已通过单元测试、HTTP/HTTPS/SOCKS5/SOCKS5H 集成测试、
全工作区测试、Clippy、Compose 配置检查，以及真实 HTTP 代理端到端验证。

建议 Commit：

```text
feat: support a global outbound proxy
```

内容：

- 全局 HTTP、HTTPS、SOCKS5 和 SOCKS5H 上游代理。
- 独立认证字段、环境变量和 `no_proxy`。
- 共享 `reqwest::Client` 接入及防绕过检查。
- 配置、集成、文档和 compose 测试。

### 14.2 管理入口和管理员认证

状态：已完成。已实现独立 `/admin` 页面和 `/admin/api/*` Cookie API、多管理员与
超级管理员保护、密码策略、登录限速和锁定、安全审计、Session 撤销及 CLI 应急重置；
旧 `/api/admin/*` Bearer API 暂时保留兼容。服务端、前端单元测试、生产构建和
Playwright 浏览器测试均已通过。

建议 Commit：

```text
feat: separate admin portal and harden administrator authentication
```

内容：

- 独立 `/admin` 路由和管理 API。
- 多管理员账号密码登录。
- 管理员 Session Cookie。
- 登录限速、锁定、审计和 CLI 恢复。
- 现有后台迁移兼容。

### 14.3 管理员 Passkey

状态：已完成。已使用 `webauthn-rs` 实现管理员 Passkey 登记、登录、命名和删除，
Challenge 服务端一次性存储、随机 UUID User Handle、精确 RP ID/HTTPS Origin 校验、
最近认证保护、强制 Passkey 策略、双凭据防锁死检查及 break-glass/CLI 恢复路径。
全工作区测试、Clippy、前端单元测试和生产构建均已通过；Playwright 已使用 Chromium
虚拟认证器验证真实的 `navigator.credentials.create()` 和 `get()` 流程。

建议 Commit：

```text
feat: add passkey authentication for administrators
```

内容：

- `webauthn-rs` 集成。
- Passkey 登记、登录、命名和删除。
- Challenge 服务端状态。
- RP ID、Origin 和 HTTPS 校验。
- Passkey 强制策略与防锁死保护。

### 14.4 用户身份和子域名

状态：已完成。已增加普通用户、外部身份、独立 Session、用户组和唯一计费组约束模型；
用户路由使用密码学随机正整数和 `sqids` 小写 DNS 字符表生成，默认最短 12 位。
已实现管理员用户管理、用户自助/管理员子域名轮换、冷却期、禁用与 Session 撤销、
统一失效响应、可信代理 Host 识别、控制入口隔离及 `public`/`subdomain_required`。
代理元数据会保留当前有效用户子域名。全工作区 203 项测试、Clippy、前端单元测试、
生产构建、Compose 校验和 8 项 Playwright 浏览器测试均已通过。

建议 Commit：

```text
feat: add user identity and sqids subdomain routing
```

内容：

- 普通用户、身份、Session、用户组模型。
- 随机数字和 Sqids 路由 ID。
- Host 归属识别。
- 子域名更换、禁用和审计。
- `public` 与 `subdomain_required`。

### 14.5 邮件邀请和无密码登录

建议 Commit：

```text
feat: add smtp invitations and passwordless email login
```

内容：

- SMTP 管理配置。
- 敏感配置加密。
- SQLite Outbox。
- 邀请、验证码和 Magic Link。
- 注册模式和企业邮箱白名单。

### 14.6 OAuth2 和 OIDC

建议 Commit：

```text
feat: add oauth2 and openid connect user login
```

内容：

- `oauth2-rs` 和 `openidconnect-rs`。
- 常用平台模板。
- 通用 OAuth2/OIDC Provider。
- PKCE、state、nonce 和身份绑定策略。

### 14.7 用户、用户组和分级配额

建议 Commit：

```text
feat: add user portal and hierarchical traffic quotas
```

内容：

- 用户个人控制台。
- 管理员用户和用户组页面。
- 用户、计费组和全局原子配额。
- 用户流量趋势和按镜像类型统计。
- 完整文档、迁移、前端 E2E 和客户端 smoke。

## 15. 验收和测试计划

### 15.1 管理员认证

- `/admin` 与普通用户门户的登录状态互不通用。
- 未登录不能访问任何 `/admin/api/*`。
- 密码正确、错误、锁定和 Session 撤销行为正确。
- 不能删除或禁用最后一个超级管理员。
- 用户 OAuth 或邮件 Session 不能访问管理 API。
- 管理员 Cookie 不会发送到用户子域名。

### 15.2 Passkey

- Passkey 注册和认证的开始、完成流程完整。
- Challenge 过期、重复使用、Session 不匹配时拒绝。
- RP ID、Origin 或签名不匹配时拒绝。
- 同一 Credential 不能重复注册。
- 管理员可以登记多个 Passkey。
- 删除 Passkey 前需要最近认证。
- 强制 Passkey 前执行防锁死检查。
- 丢失 Passkey 后仍可通过密码或 CLI 恢复。

### 15.3 OAuth/OIDC 和邮件

- OAuth2 回调正确校验 state 和 PKCE。
- OIDC 正确校验 nonce、issuer、audience、签名和过期时间。
- 未验证邮箱不能自动绑定或绕过邀请。
- 邀请邮箱与最终验证邮箱不一致时拒绝注册。
- Magic Link、验证码和邀请凭证只能使用一次。
- SMTP 失败不会错误标记为发送成功。
- OAuth/SMTP 密钥不会出现在 API、日志和审计中。

### 15.4 子域名和计量

- 用户子域名正确归属用户、计费组和全局流量。
- 子域名更换后旧值立即失效。
- 用户禁用后子域名立即停止服务。
- `subdomain_required` 下主域名不能访问包代理路径。
- 非法 Host 和伪造 `X-Forwarded-Host` 被拒绝。
- npm、Cargo、pip、Maven 等客户端可以直接使用用户子域名。
- 不要求 Token、Basic Auth 或 Bearer Auth。

### 15.5 配额和权限

- 并发下载不能绕过用户、计费组或全局配额。
- 用户只能查看自己的流量和配额。
- 管理员可以查看、调整和解除用户配额。
- 新月份自动重置周期状态。
- 用户被移动到其他计费组后，新请求使用新组配额。
- SQLite 升级后现有管理员、全局配置和统计不丢失。

## 16. 部署前置条件

启用用户子域名前必须满足：

- 主域名和 `base_domain` 配置一致。
- 主域名使用 HTTPS。
- `*.base_domain` 通配符 DNS 已生效。
- TLS 证书覆盖主域名和通配符子域名。
- 反向代理保留原始 Host。
- 可信代理列表配置正确。
- `/admin` 的 WebAuthn RP ID 和 Origin 检查通过。
- `MIRRORPROXY_MASTER_KEY` 已配置并持久保存。
- SQLite 数据目录和主密钥都已纳入备份方案。

如果以上条件不满足，管理后台不得允许启用 `subdomain_required` 或管理员 Passkey 强制策略。

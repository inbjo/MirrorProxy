# 支持的软件源

## DNS over HTTPS（DoH）

可选 `/dns-query` 端点支持 RFC 8484 二进制 GET（`dns=` 使用无填充 base64url）
和 POST（`Content-Type: application/dns-message`）。这是 DoH，不监听 DoT/853
端口，不提供 JSON DNS API。

打开**后台 → 高级设置 → DNS over HTTPS (DoH)**，启用代理、选择上游并保存，
配置立即生效并在重启后保留。选择顺序决定故障回退顺序，面板同时展示客户端地址。
首次初始化也可以在服务 TOML 中配置：

```toml
[doh]
enabled = true
upstreams = ["https://1.1.1.1/dns-query"]
```

需要顺序故障回退时设置
`upstreams = ["https://1.1.1.1/dns-query", "https://8.8.8.8/dns-query", "https://9.9.9.9/dns-query"]`。
另支持 `1.0.0.1`、`8.8.4.4`、`149.112.112.112`，须使用 HTTPS、443 端口和准确的
`/dns-query` 路径。不接受域名、私网 IP、URL 凭据、查询参数或重定向，客户端不能指定上游。

通过现有 HTTPS 监听或反向代理暴露 `https://xx.com/dns-query`，保留查询参数和 POST
请求体，再填入浏览器或路由器的自定义 DoH URL。支持单问题标准 DNS 查询、EDNS、DNSSEC
记录；请求和应答最大 65,535 字节。专用解析客户端启用 TLS 证书校验和 HTTP/2，直接
连接上游，不继承软件源出站代理或自定义 TLS 设置。每个上游尝试最多十秒，最多配置六个
地址；网络、HTTP 错误或无效应答触发回退，NXDOMAIN 等有效 DNS 错误原样返回。
不使用磁盘缓存，响应为 `Cache-Control: no-store`，不转发客户端凭据或上游 Cookie。

复用 IP 策略、限流、配额、用户子域名路由和流量统计，目标为 `doh`；限制目标的团队
需要允许该目标。不记录 DNS 域名或二进制内容。软件源目录及健康探测仍只描述软件源。

执行 `python scripts/smoke-doh.py https://xx.com` 验证 GET、POST、DNS 事务 ID、问题、
应答记录和畸形请求拒绝（仅需 Python 标准库）。验证公网连通性时分别切换各上游执行。
回退：后台关闭 DoH 并保存，端点立即恢复 404。新旧实例均默认关闭；已有数据库使用
持久化后台配置，不重新加载 TOML 值。无需数据库迁移，软件源 CLI 不修改客户端 DNS。

先打开实例首页，选择软件源，复制生成的命令；本机自动改源见[客户端](Client-zh-CN)，容器镜像见[容器指南](Container-Registries-and-Runtimes-zh-CN)。

> **服务端支持代理，不代表客户端能自动修改每一种配置。** MirrorProxy 是按需的只读代理/缓存，不是任意转发代理，也不提供私有制品上传。

当前实例支持列表以 `/api/sources` 为准；后台“高级设置”可开关适配器、修改上游组。多个上游地址以逗号分隔，按配置策略尝试。

源卡片和配置弹窗展示缓存能力及当前开关、上游认证规则和客户端。按条件缓存仅适用于符合规则的请求，启用不代表每次命中。Hugging Face、Docker/OCI 和 Homebrew OCI 绕过服务端磁盘缓存。仅配置指导的条目表示客户端直接配置目标源，不代表本站执行代理。

客户端 Authorization 默认不透传；管理员可为协议支持的请求显式开启，服务端配置的上游凭据优先。Cookie 不转发。HF 始终不转发客户端凭据，也不使用服务端上游凭据，不受全局透传开关影响；仅支持公开 HTTP 下载，需要禁用原生 Xet。

## 已覆盖类别

| 类别 | 代表目标 |
| --- | --- |
| 代码与发布 | GitHub、GitHub Raw、Git smart HTTP clone。 |
| AI 模型和数据集 | Hugging Face 公开 HTTP 下载，服务端不缓存。 |
| OCI | Docker Hub、GHCR、Quay、Kubernetes Registry、GCR、Microsoft MCR、Elastic Registry、GitLab、NVIDIA NVCR、Oracle、Homebrew OCI。 |
| 语言生态 | Composer、npm、Go、Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Julia、LuaRocks、Clojars、CocoaPods、Pub、Anaconda、PyPI、Cargo。 |
| 工具链 | Rustup、NVM、Homebrew、WinGet、TeX Live、ELPA、Nix、Guix、Flatpak。 |
| 操作系统仓库 | Debian、Ubuntu、Fedora、Arch、Alpine、openSUSE、Void、Gentoo、FreeBSD，以及 OpenWrt、Termux、MSYS2、ROS 和配置的额外目录源。 |

“支持代理”不等于所有原生客户端的配置格式都一致：服务端负责转发协议，独立客户端只会为其
目录中定义且本机可安全编辑的目标生成改源配置。FreeBSD pkg 等 OS 目标属于代理目录覆盖，
不应推断为客户端已实现完整系统级自动改源。

## Hugging Face 公开 HTTP 下载

在后台“高级设置”启用 `huggingface`。已有实例若持久化了 `enabled_proxies` 列表，需要主动
添加。`upstreams.huggingface` 默认为 `https://huggingface.co`；可信的替代 Hub 地址可包含
基础路径，上游组沿用现有选择策略。

安装官方 `huggingface_hub` 客户端，在 POSIX shell 执行：

```sh
export HF_ENDPOINT='https://mirror.example/huggingface'
export HF_HUB_DISABLE_XET=1
hf download gpt2 config.json
```

PowerShell：

```powershell
$env:HF_ENDPOINT='https://mirror.example/huggingface'
$env:HF_HUB_DISABLE_XET='1'
hf download gpt2 config.json
```

两个变量必须在 **Python 导入相关库之前** 设置。入口支持 `hf_hub_download`、
`snapshot_download` 以及 Transformers/Diffusers 经 Hub 下载文件的路径；覆盖仓库信息、
文件树分页、模型文件和数据集仓库文件。数据集引用的外部网站不属于本适配器范围。
`mirrorproxy get hf --base-url https://mirror.example` 可查看适合当前平台的示例；
CLI 不会持久化 HF 环境变量或 Token。

仅接受 GET/HEAD，保留提交号、哈希、大小、条件请求和 Range/If-Range 语义。
CDN 跳转由服务端跟随，客户端无需直接访问 HF CDN。暂不支持原生 Xet、Git/LFS clone、
私有或需申请权限的 gated 仓库、上传和推理 API。客户端凭据和服务端配置的上游凭据都不会
发送给 HF；客户端已有 HF 登录状态也不会获得私有仓库访问能力。

所有 HF 请求均绕过 MirrorProxy 磁盘缓存，包括小文件和元数据；SDK 自己的本地缓存仍然
有效。大文件直接流式转发，不完整缓冲到内存。`timeout.request_secs` 在本适配器中限制
连接和读取空闲时间，不限制健康下载的总时长。HF 路由沿用现有源访问策略、限流、配额和
用量统计，无需数据库迁移。

服务器需要直接访问 Hub/CDN。目标校验和 DNS 固定仍然生效：启用全局出站代理时，跳转到
DNS 域名目标会按现有安全策略拒绝。HF 下载主机使用明确白名单，若官方调整 CDN 基础设施，
可能需要更新；未知主机直接拒绝。

可用隔离的 SDK 环境对本地构建做真实上游验收：

```sh
uv run --no-project --with huggingface-hub==2.2.0 scripts/smoke-huggingface.py target/debug/mirrorproxy-server
```

脚本启动临时服务和数据库，校验真实模型 SHA-256、SDK 快照和数据集下载、HEAD 元数据、
Range/续传、分页，以及服务端没有缓存写入。SDK HTTP 钩子会拒绝绕过本地代理的请求。
该验收需要访问公共上游。如果测试环境仅能经代理出站，可在命令前加
`MIRRORPROXY_HF_TEST_RELAY=1`，通过临时可信上游中继和环境中的 HTTP 代理访问 HF，保留
正常 TLS 校验。输出会注明中继模式；它验证替代上游路径，不代表默认直连 HF 的网络链路
已经实测。生产重定向和 DNS 校验仍然生效。

回退时执行 `unset HF_ENDPOINT HF_HUB_DISABLE_XET`
（PowerShell：`Remove-Item Env:HF_ENDPOINT, Env:HF_HUB_DISABLE_XET -ErrorAction SilentlyContinue`），
并在后台关闭适配器即可停止服务。

## 进阶：上游与安全边界

- PyPI HTML 索引支持 `../../packages/...` 下载链接，并保留查询参数和哈希片段。
  `upstreams.pypi_files` 应设置为 `packages/` 之前的文件主机或基础目录；
  `/pypi/packages/...` 和已有 `/pypi/files/packages/...` 路由都会保留该上游路径段。
- 启用出站代理会影响镜像上游 HTTP 请求；ACME、DNS API 和 OAuth 等控制面请求不使用它。
- 企业 CA 可以通过 `upstream_tls.ca_certificates` 添加 PEM；`insecure_skip_verify=true` 会关闭
  所有镜像上游 TLS 校验，只能短期排障。
- Git 智能 HTTP 只允许只读 clone 所需的 upload-pack POST；receive-pack 写入仍被拒绝。
- 私有上游凭据、客户端 `Authorization` 是否向上游转发应按目标与最小权限原则配置。
- 上游重定向最多跟随 10 次。新目标必须仅解析到公网地址，并固定本次请求使用的解析结果。
  明确配置为上游的私有 IP 可在同源内跳转。启用出站代理时仅允许已配置的 IP 同源跳转，
  因为出站代理可能自行解析其他目标。升级后若某上游因此失败，请直接配置可信的最终 URL，
  不要关闭目标地址检查。跨域跳转会移除凭据。

[English](Proxy-Adapters)

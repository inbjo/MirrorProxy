# 代理适配器

`/api/sources` 是 Web 门户、客户端和文档均应遵循的权威目录；不要根据本文手工猜测 URL。
后台“高级设置”控制适配器开关与上游组。一个上游字段可填多个逗号分隔的 HTTP(S) 地址，
服务会按顺序尝试可用入口。

## 已覆盖类别

| 类别 | 代表目标 |
| --- | --- |
| 代码与发布 | GitHub、GitHub Raw、Git smart HTTP clone。 |
| OCI | Docker Hub、GHCR、Quay、Kubernetes Registry、Homebrew OCI。 |
| 语言生态 | Composer、npm、Go、Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Julia、LuaRocks、Clojars、CocoaPods、Pub、Anaconda、PyPI、Cargo。 |
| 工具链 | Rustup、NVM、Homebrew、WinGet、TeX Live、ELPA、Nix、Guix、Flatpak。 |
| 操作系统仓库 | Debian、Ubuntu、Fedora、Arch、Alpine、openSUSE、Void、Gentoo、FreeBSD，以及 OpenWrt、Termux、MSYS2、ROS 和配置的额外目录源。 |

“支持代理”不等于所有原生客户端的配置格式都一致：服务端负责转发协议，独立客户端只会为其
目录中定义且本机可安全编辑的目标生成改源配置。FreeBSD pkg 等 OS 目标属于代理目录覆盖，
不应推断为客户端已实现完整系统级自动改源。

## 上游与安全边界

- 启用出站代理会影响镜像上游 HTTP 请求；ACME、DNS API 和 OAuth 等控制面请求不使用它。
- 企业 CA 可以通过 `upstream_tls.ca_certificates` 添加 PEM；`insecure_skip_verify=true` 会关闭
  所有镜像上游 TLS 校验，只能短期排障。
- Git 智能 HTTP 只允许只读 clone 所需的 upload-pack POST；receive-pack 写入仍被拒绝。
- 私有上游凭据、客户端 `Authorization` 是否向上游转发应按目标与最小权限原则配置。

[English](Proxy-Adapters)

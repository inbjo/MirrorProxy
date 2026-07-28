# MirrorProxy Wiki

MirrorProxy 是一个自部署的 Rust 镜像代理平台，由三个可独立演进的部分组成：

- `mirrorproxy-server`：代理服务、SQLite 数据库和内嵌 React 管理后台。
- `mirrorproxy`：独立的 Windows、macOS、Linux 改源客户端，不需要安装服务端。
- `mirrorproxy-catalog`：服务端和客户端共用的目标、路径与改源能力目录。

它将 GitHub、OCI、语言包仓库、开发工具链和多个操作系统仓库统一到一个可控入口。后台提供
用户与配额、镜像健康检查、审计日志、GeoIP 地域报表、IP/CIDR 黑白名单和 ACME 证书管理。
所有 IP 定位使用本地 XDB 数据库，不会把查询 IP 发给第三方。

## 选择部署模式

| 场景 | 建议 |
| --- | --- |
| 已有 Caddy/Nginx/Traefik | 保持反向代理终止 TLS，MirrorProxy 监听内部端口。 |
| 单机、希望移除反向代理 | 启用 ACME `direct_https`，由 MirrorProxy 监听 80/443 并将 HTTP 重定向到 HTTPS。 |
| 普通域名证书 | 使用 HTTP-01；公网 80 必须到达 MirrorProxy。 |
| 通配符证书 | 使用 DNS-01；支持 Cloudflare、阿里云、DNSPod、Route53 和 Webhook。 |

首次使用从“快速开始”进入；生产环境请完整阅读“部署”和“ACME 自动证书”。

- [快速开始](Getting-Started-zh-CN)
- [部署](Deployment-zh-CN)
- [代理适配器](Proxy-Adapters-zh-CN)
- [客户端](Client-zh-CN)
- [客户端分发](Distribution-zh-CN)
- [后台管理](Administration-zh-CN)
- [GeoIP 与 IP 访问控制](GeoIP-and-IP-Access-Control-zh-CN)
- [ACME 自动证书](ACME-Certificates-zh-CN)
- [开发与路线图](Development-and-Roadmap-zh-CN)

[English](Home)

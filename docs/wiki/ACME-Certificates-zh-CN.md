# ACME 自动证书

MirrorProxy 可以通过 ACME 自动申请和续期证书，并把结果写入
`fullchain.pem` 与 `privkey.pem`。既可以交给反向代理加载，也可以由 MirrorProxy
直接监听 80/443 并提供 HTTPS。ACME 默认关闭。账户密钥与证书保存在配置指定的
本地目录；通过后台填写的 DNS API 密钥保存在本机 SQLite 中，并且只写不读，配置
接口只返回“已配置”状态，不返回密钥明文。

## 后台配置

超级管理员可以在“高级设置 → ACME 自动证书”中配置 HTTP-01、DNS-01、证书域名、
续期周期和 DNS 提供商凭据。后台配置写入独立的 `acme_settings` 表，不会改写
`config.toml`。保存后需要重启 MirrorProxy 才会替换运行中的 ACME Worker；保存动作
本身不会签发证书、替换现有证书文件或修改 Caddy。

如果进程设置了 `MIRRORPROXY_ACME_*` 或 acme.sh 兼容的 DNS 凭据环境变量，环境变量
保持最高优先级，后台表单会变为只读。未使用环境变量时，首次升级会从当前 TOML 配置
初始化后台设置表，避免丢失既有配置。

## 不使用反向代理直接提供 HTTPS

启用 `direct_https` 后，MirrorProxy 同时管理两个监听器：HTTP 监听器始终保留
`/.well-known/acme-challenge/*`，HTTPS 监听器使用 ACME 证书提供 TLS。开启重定向时，
其余 HTTP 请求在证书就绪后返回 308，并完整保留路径和查询参数。首次证书尚未签发时，
普通 HTTP 请求返回 503，避免把客户端重定向到尚不可用的 443 端口。

```toml
[acme]
enabled = true
email = "admin@example.com"
domains = ["mirror.example.com"]
challenge = "http-01"
storage_directory = "/var/lib/mirrorproxy/acme"
direct_https = true
http_listen_addr = "0.0.0.0:80"
https_listen_addr = "0.0.0.0:443"
redirect_http_to_https = true
```

已有有效证书会在启动时直接加载；首次签发成功后 443 自动开始服务。后续续签会热加载
新证书，已有连接和旧证书在加载失败时不受影响。两个监听地址不能相同。使用 DNS-01
时仍可开启 80 到 443 的重定向，但验证本身不依赖 80 端口。

不建议让服务进程长期以 root 身份运行。systemd 服务可以增加以下权限，使普通用户只
获得绑定 1024 以下端口的能力：

```ini
[Service]
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=true
```

Docker/Podman 镜像默认以非 root 用户运行，建议设置
`http_listen_addr = "0.0.0.0:3000"`、`https_listen_addr = "0.0.0.0:3443"`，再映射
`80:3000` 和 `443:3443`，无需给容器增加额外 capability。修改监听模式或地址后需要
重启服务；证书续期本身不需要重启。

## HTTP-01

适用于普通域名。域名的 A/AAAA 记录必须指向当前入口。反向代理模式下，公网 80
端口需要把 `/.well-known/acme-challenge/*` 原样转发到 MirrorProxy；原生 HTTPS
模式会自行处理该路径。HTTP-01 不能签发通配符证书。

```toml
[acme]
enabled = true
email = "admin@example.com"
domains = ["mirror.example.com"]
challenge = "http-01"
storage_directory = "/var/lib/mirrorproxy/acme"
```

Nginx 示例：

```nginx
location ^~ /.well-known/acme-challenge/ {
    proxy_pass http://127.0.0.1:3000;
}
```

如果入口是 Caddy，Caddy 本身已经能够自动管理普通域名证书；仅在希望统一由
MirrorProxy 输出证书文件时才启用本功能，并确保该 challenge 路由优先于 HTTP 到
HTTPS 的重定向。

## DNS-01

DNS-01 支持普通域名和通配符域名，不要求开放 80 端口。内置 Cloudflare、阿里云
DNS、腾讯云 DNSPod、AWS Route53 与通用 Webhook 提供商。

```toml
[acme]
enabled = true
email = "admin@example.com"
domains = ["example.com", "*.example.com"]
challenge = "dns-01"
storage_directory = "/var/lib/mirrorproxy/acme"

[acme.dns]
provider = "cloudflare"
cloudflare_zone_id = "zone-id"
propagation_delay_secs = 30
```

API Token 建议通过 `MIRRORPROXY_ACME_CLOUDFLARE_API_TOKEN` 注入，并只授予对应
Zone 的 DNS Edit 权限。也兼容 acme.sh 的 `CF_Zone_ID`、`CF_Token`、`CF_Key` 与
`CF_Email` 环境变量，但 Global API Key 权限更大，不建议用于新部署。

其他原生提供商要求显式配置托管主域名，MirrorProxy 不会猜测 `co.uk` 等公共后缀：

```toml
# 阿里云 DNS
[acme.dns]
provider = "aliyun"
aliyun_domain = "example.com"
# MIRRORPROXY_ACME_ALIYUN_ACCESS_KEY_ID / ..._ACCESS_KEY_SECRET
# 兼容 acme.sh：Ali_Key / Ali_Secret
```

```toml
# 腾讯云 DNSPod
[acme.dns]
provider = "tencent"
tencent_domain = "example.com"
# MIRRORPROXY_ACME_TENCENT_SECRET_ID / ..._SECRET_KEY
# 兼容 acme.sh：Tencent_SecretId / Tencent_SecretKey
```

```toml
# AWS Route53
[acme.dns]
provider = "route53"
route53_hosted_zone_id = "Z0123456789"
# AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY / AWS_SESSION_TOKEN
```

密钥应只授予所选 Zone 的解析记录管理权限。Route53 添加和删除 ACME 值时会保留同名
TXT 记录中的其他值。
同时兼容 acme.sh 的 provider 名称：`dns_cf`、`dns_ali`、`dns_tencent`（或
`dns_dp`）以及 `dns_aws`。

Webhook 提供商会向 `webhook_url` 发送 JSON POST：

```json
{
  "action": "present",
  "record_type": "TXT",
  "record_name": "_acme-challenge.example.com",
  "value": "challenge-value",
  "ttl": 120
}
```

验证完成后会再次发送 `action=cleanup`。两个请求都必须返回 2xx。可通过
`MIRRORPROXY_ACME_DNS_WEBHOOK_BEARER_TOKEN` 增加 Bearer 身份验证。

## 续期与使用

服务启动后会检查现有证书，在距离到期时间小于 `renew_before_days` 时自动续期，
并按 `check_interval_hours` 周期检查。超级管理员也可以在“高级设置”修改配置、查看
状态并手动触发签发。测试配置时应先把 `directory_url` 切换到 Let’s Encrypt staging，
避免触发生产环境速率限制。

反向代理应读取：

- `<storage_directory>/fullchain.pem`
- `<storage_directory>/privkey.pem`

反向代理模式下，MirrorProxy 负责安全、原子地更新文件；Nginx/Apache 等仍需由 systemd path unit、
容器编排或配置管理工具在文件变化后执行平滑重载。Caddy 部署通常应优先使用 Caddy
原生证书管理。原生 HTTPS 模式会在续签后自行热加载证书。

[English](ACME-Certificates)

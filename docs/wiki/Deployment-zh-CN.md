# 部署

服务端发布包包含 `mirrorproxy-server`、`config.example.toml` 和 IPv4/IPv6 ip2region
数据库。无论 Docker 还是二进制部署，都应将 SQLite、缓存、GeoIP 与 ACME 目录放在
持久化存储中；升级二进制/镜像前先验证数据库文件可读，并保持数据目录不变。

## 部署模式

### 反向代理终止 TLS

默认模式监听内部地址，例如 `127.0.0.1:3000`。由 Nginx、Caddy、Traefik、Apache 或
HAProxy 对外提供 TLS，并将原始 `Host`、请求路径与方法转发给 MirrorProxy。此模式适合
已经由入口统一管理证书的服务器。`trusted_proxies` 只填写真实反向代理节点；MirrorProxy
仅在 TCP 对端可信时读取 `X-Forwarded-For`，且从右向左解析代理链。

单层 Nginx 应使用 `$remote_addr` 覆盖该请求头，不能直接转发客户端自带值；多层代理只有
在每个中间节点都可信时才可保留追加链。管理后台也应通过 HTTPS 暴露。

### MirrorProxy 原生 HTTPS

不使用 Caddy/Nginx 时，可在后台“高级设置 → ACME 自动证书”或 TOML 中启用
`direct_https`。服务自行绑定 80 与 443，保留 HTTP-01 challenge，证书就绪后将其他 HTTP
请求 308 重定向到 HTTPS；续期会热加载，不需要重启。详见[ACME 自动证书](ACME-Certificates-zh-CN)。

二进制服务建议使用非 root 帐号，并通过 systemd 授予 `CAP_NET_BIND_SERVICE`；容器默认
非 root，建议内部使用 3000/3443 再映射到宿主机 80/443。

## Docker 运行要点

- 永久挂载 `/data`；镜像默认将数据库、缓存和可写 GeoIP 放在该目录。
- 通过环境变量覆盖配置时，环境变量优先于 TOML；特别是 `MIRRORPROXY_ACME_*` 会让后台
  ACME 表单变为只读。
- 固定镜像版本后再升级；升级后检查 `/healthz`、`/admin` 登录、一个公共代理目标和日志。
- 不要把 SQLite 放到网络文件系统；单实例使用本地磁盘最稳妥。

## 用户子域名

`subdomain_required` 是可选的按用户路由/计费模式，不是普通代理的必要条件。启用前必须
同时完成通配符 DNS、通配符 TLS、入口保留原始 Host，并在后台确认基础设施就绪；否则主域名
上的包代理路径会按策略拒绝。

## 镜像上游的企业 CA

MirrorProxy 的镜像上游客户端默认同时信任 WebPKI 公共根证书和操作系统根证书。
如果企业 HTTPS 上游由私有 CA 签发，可额外挂载一个或多个 PEM Bundle：

```toml
[upstream_tls]
ca_certificates = ["/etc/mirrorproxy/ca/company-root.pem"]
insecure_skip_verify = false
```

Docker 部署必须先把证书文件挂载到容器内，例如：

```yaml
volumes:
  - ./company-root.pem:/etc/mirrorproxy/ca/company-root.pem:ro
```

也可使用环境变量：

```text
MIRRORPROXY_UPSTREAM_TLS_CA_CERTIFICATES=/etc/mirrorproxy/ca/company-root.pem
MIRRORPROXY_UPSTREAM_TLS_INSECURE_SKIP_VERIFY=false
```

`insecure_skip_verify = true` 会关闭所有镜像上游 HTTPS 的证书校验，存在中间人攻击
风险，只能临时用于调试。该设置及附加 CA 不会应用到 ACME、DNS 服务商 API 或 OAuth
等控制面请求。

[English](Deployment)

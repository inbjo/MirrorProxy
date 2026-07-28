# 部署

服务端发布包包含 `mirrorproxy-server`、`config.example.toml` 和 IPv4/IPv6
ip2region 数据库。应从持久化工作目录运行，或者明确设置 `MIRRORPROXY_DB` 与两个
`MIRRORPROXY_GEOIP_*_PATH` 环境变量。

生产环境应隐藏服务监听端口，并由 Nginx、Caddy、Traefik、Apache 或 HAProxy 提供
TLS。`trusted_proxies` 只填写真实反向代理节点。MirrorProxy 仅在直接连接来源可信时
读取 `X-Forwarded-For`，并从右向左解析代理链。单层 Nginx 应使用 `$remote_addr`
覆盖该请求头；多层代理只有在所有中间节点都可信时才保留追加链。

用户通配符子域名需要通配符 DNS、TLS 证书和原始 Host 转发，全部验证后才能启用
`subdomain_required`。

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

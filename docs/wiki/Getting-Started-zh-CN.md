# 快速开始

本页用于在一台主机上启动一个可登录、可代理的实例。生产环境的 TLS、备份、反向代理或
原生 HTTPS 请继续阅读[部署](Deployment-zh-CN)和[ACME 自动证书](ACME-Certificates-zh-CN)。

## Docker Compose（推荐）

创建 `compose.yml`：

```yaml
services:
  mirrorproxy:
    image: kudang/mirrorproxy:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:3000:3000"
    environment:
      MIRRORPROXY_ADMIN_PASSWORD: "replace-with-a-long-unique-password"
    volumes:
      - mirrorproxy-data:/data
volumes:
  mirrorproxy-data:
```

启动并验证：

```bash
docker compose up -d
curl -f http://127.0.0.1:3000/healthz
docker compose logs mirrorproxy
```

访问 `http://127.0.0.1:3000/admin`，使用初始管理员登录。未设置
`MIRRORPROXY_ADMIN_PASSWORD` 时服务会只在首次启动日志中打印随机密码；应立即修改并保存。
务必保留 `/data` 卷，其中包含 SQLite、缓存、可写 GeoIP 数据库和后台保存的 ACME 机密。

## 二进制或源码

发布包内包含 `mirrorproxy-server`、配置模板和 GeoIP 数据。复制模板后，以持久化目录运行：

```bash
cp config.example.toml config.toml
./mirrorproxy-server --config ./config.toml serve
```

从源码运行前需拉取固定版本的 GeoIP 数据库：

```bash
bash scripts/fetch-geoip.sh
cargo run -p mirrorproxy-server -- --config config.example.toml serve
```

默认模板仅监听 `127.0.0.1:3000`，适合先放在 Caddy/Nginx 后面。不要在未设置认证、可信
代理和 TLS 策略前直接暴露管理端口。

## 下一步

1. 在后台“镜像检测”确认需要的代理目标可用；`/api/sources` 是实际目录来源。
2. 在“高级设置”按需关闭未使用的适配器、设置上游、限速、缓存和出站代理。
3. 配置可信反向代理或 ACME 原生 HTTPS，再公开域名。
4. 用独立客户端执行 `mirrorproxy list`，再以 `mirrorproxy set npm --base-url https://你的域名`
   等命令配置本机软件源。

[English](Getting-Started) · [完整部署文档](Deployment-zh-CN) · [客户端](Client-zh-CN)

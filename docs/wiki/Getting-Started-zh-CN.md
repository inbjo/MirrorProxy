# 快速开始

## Docker Compose

```yaml
services:
  mirrorproxy:
    image: kudang/mirrorproxy:latest
    restart: unless-stopped
    ports: ["3000:3000"]
    volumes: ["mirrorproxy-data:/data"]
volumes:
  mirrorproxy-data:
```

执行 `docker compose up -d`，检查 `http://127.0.0.1:3000/healthz`，然后访问
`/admin`。未指定初始密码时，从容器日志读取自动生成的密码。升级时必须保留 `/data`
数据卷，其中包含 SQLite、缓存和可写 GeoIP 数据库。

从源码运行时，先执行 `bash scripts/fetch-geoip.sh`，再执行
`cargo run -p mirrorproxy-server -- serve`。

[English](Getting-Started) · [完整部署文档](Deployment-zh-CN)

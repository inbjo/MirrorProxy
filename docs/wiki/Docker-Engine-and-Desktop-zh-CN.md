# Docker Engine 与桌面运行时

本页说明如何为 Docker Engine、Docker Desktop 和 OrbStack 配置 MirrorProxy。示例中的
`mirror.example.com` 应替换为实际部署域名。

## Linux Docker Engine

推荐使用客户端结构化合并现有 `/etc/docker/daemon.json`：

```bash
mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system --dry-run
sudo mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system
```

先审阅预览，再在维护窗口重启 Docker：

```bash
sudo systemctl restart docker
docker info | sed -n '/Registry Mirrors/,+5p'
docker pull busybox:1.36.1
```

客户端只修改 `registry-mirrors`，保留现有 JSON 字段并保存可回滚副本。若 Docker 无法启动，
先运行 `dockerd --validate --config-file /etc/docker/daemon.json` 并查看
`journalctl -u docker -n 100`。

## Docker Desktop

1. 打开 Settings → Docker Engine。
2. 在现有 JSON 中合并以下字段，不要删除其他键：

   ```json
   {
     "registry-mirrors": ["https://mirror.example.com"]
   }
   ```

3. 点击 Apply & Restart。
4. 在终端运行 `docker info` 和一次真实 `docker pull`。

有效 HTTPS 域名不需要加入 `insecure-registries`。如果 Docker Desktop 由企业策略管理，
应由管理员下发设置，而不是反复覆盖本地配置。

## OrbStack

在 OrbStack 设置中打开 Docker 配置，或审阅 `~/.orbstack/config/docker.json`，合并相同的
`registry-mirrors` 字段并按界面提示重启。完成后同样使用 `docker info` 验证。

## Compose 与 Dockerfile

Docker Hub 镜像可以保持原引用并由 mirror 加速。其他受支持 Registry 必须显式改写：

```yaml
services:
  api:
    image: mirror.example.com/ghcr.io/owner/api:1.0
  worker:
    image: mirror.example.com/mcr.microsoft.com/dotnet/runtime:8.0
```

```dockerfile
FROM mirror.example.com/docker.elastic.co/elasticsearch/elasticsearch:8.13.4
```

首页的“容器 Registry 配置台”可转换 Compose YAML 和 Dockerfile。提交转换结果前应审阅差异，
并在 CI 中固定明确的镜像标签或 digest。

## 回滚与排障

```bash
sudo mirrorproxy reset docker --scope system --dry-run
sudo mirrorproxy reset docker --scope system
sudo systemctl restart docker
```

- 重启 Docker 可能中断现有容器，应安排维护窗口并保留独立管理连接。
- `registry-mirrors` 只覆盖 Docker Hub；它不会自动代理 GHCR、GCR、Quay 或 MCR。
- `docker info` 显示配置只证明 Docker 已读取设置，仍需实际拉取 Manifest 与 Blob。
- Dockerfile 或 Compose 中写了完整第三方 Registry 时，必须显式使用 MirrorProxy 地址。

[English](Docker-Engine-and-Desktop) · [返回容器 Registry 总览](Container-Registries-and-Runtimes-zh-CN)

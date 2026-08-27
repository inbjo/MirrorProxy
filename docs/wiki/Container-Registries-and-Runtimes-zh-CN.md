# 容器 Registry 与运行时配置

MirrorProxy 通过一个 OCI Distribution 端点代理多个公开容器仓库。网页首页的“容器
Registry 配置台”读取 `/api/sources` 返回的真实能力列表，可校验单个镜像，也可转换
Compose YAML 和 Dockerfile。

按使用环境选择详细教程：

- [Docker Engine 与桌面运行时](Docker-Engine-and-Desktop-zh-CN)
- [containerd 与 K3s](Containerd-and-K3s-zh-CN)
- [NAS、面板与其他运行时](NAS-Panels-and-Other-Runtimes-zh-CN)

## 支持范围

| Registry | 原始镜像 | MirrorProxy 路径 |
| --- | --- | --- |
| Docker Hub | `nginx:latest` | `<本站域名>/nginx:latest` |
| GHCR | `ghcr.io/owner/image:tag` | `<本站域名>/ghcr.io/owner/image:tag` |
| Quay | `quay.io/owner/image:tag` | `<本站域名>/quay.io/owner/image:tag` |
| Kubernetes | `registry.k8s.io/pause:3.10` | `<本站域名>/registry.k8s.io/pause:3.10` |
| GCR | `gcr.io/project/image:tag` | `<本站域名>/gcr.io/project/image:tag` |
| Microsoft MCR | `mcr.microsoft.com/dotnet/runtime:8.0` | `<本站域名>/mcr.microsoft.com/dotnet/runtime:8.0` |
| Elastic | `docker.elastic.co/elasticsearch/elasticsearch:8.13.4` | `<本站域名>/docker.elastic.co/elasticsearch/elasticsearch:8.13.4` |
| GitLab | `registry.gitlab.com/group/project/image:tag` | `<本站域名>/registry.gitlab.com/group/project/image:tag` |
| NVIDIA NVCR | `nvcr.io/nvidia/cuda:tag` | `<本站域名>/nvcr.io/nvidia/cuda:tag` |
| Oracle | `container-registry.oracle.com/os/oraclelinux:tag` | `<本站域名>/container-registry.oracle.com/os/oraclelinux:tag` |

`k8s.gcr.io` 仅作为旧地址兼容输入，实际使用 `registry.k8s.io` 上游。

当前适配器面向公开镜像的 `GET`/`HEAD` 拉取。私有镜像、需要点击接受许可的镜像、推送、
删除以及签名写入不属于公开代理承诺。

GitLab 私有项目、NGC 组织/团队镜像，以及要求登录或先接受条款的 Oracle 镜像不会借用
MirrorProxy 服务端凭据；上游的 `401`/`403` 会作为权限边界保留。列入支持矩阵只表示能够
匿名拉取该 Registry 中的公开项目。

## Docker Engine：Docker Hub 全局加速

Docker 的 `registry-mirrors` 只作用于 Docker Hub。先预览，再写入系统配置：

```bash
mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system --dry-run

sudo mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system
```

客户端会结构化合并 `/etc/docker/daemon.json`，保留已有的 `data-root`、`dns`、运行时等
字段，并在 `/var/lib/mirrorproxy/sources/docker.json` 保存回滚记录。它不会自动重启 Docker；
维护窗口中自行执行：

```bash
sudo systemctl restart docker
docker info | sed -n '/Registry Mirrors/,+5p'
docker pull busybox:1.36.1
```

恢复原配置：

```bash
sudo mirrorproxy reset docker --scope system --dry-run
sudo mirrorproxy reset docker --scope system
sudo systemctl restart docker
```

重启 Docker 可能影响运行中的容器。生产主机应先检查配置差异，并准备独立的管理连接。

## Docker Desktop、OrbStack 与 NAS

Docker Desktop 的 Docker Engine 设置中合并以下字段，然后点击 Apply & restart：

```json
{
  "registry-mirrors": ["https://mirror.example.com"]
}
```

不要删除面板中已有的其他 JSON 字段。OrbStack、群晖、QNAP、Unraid、飞牛等平台若提供
“Registry mirror”输入框，可填写本站根地址；该配置仍然只加速 Docker Hub。不能编辑 daemon
配置的平台，使用显式镜像地址改写。

## Compose 与 Dockerfile

对 Docker Hub 之外的受支持 Registry，显式改写镜像地址：

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

建议使用网页配置台转换并审阅差异。转换器只修改 `image:` 与 `FROM` 中已知 Registry；未知
Registry 保持原样，避免生成后端实际不能代理的地址。

## containerd 与 K3s

Docker Hub 可使用运行时原生镜像端点。K3s 的 `/etc/rancher/k3s/registries.yaml` 示例：

```yaml
mirrors:
  docker.io:
    endpoint:
      - "https://mirror.example.com"
```

修改后在维护窗口重启 K3s，并以实际 Pod 拉取验证。对于其他 Registry，优先在 Helm values、
Deployment、DaemonSet 或离线清单中显式改写完整镜像地址；不要假定 Docker 的
`registry-mirrors` 会覆盖它们。

## Podman、Harbor、Nexus 与 Portainer

- Podman：使用显式 MirrorProxy 镜像地址最容易审计；修改 `registries.conf` 前先确认当前
  发行版使用的配置版本。
- Portainer：把本站作为自定义 Registry 使用，并在 Stack 中改写镜像地址。
- Harbor/Nexus：MirrorProxy 已是流式代理，不建议再把私有凭据交给公共实例。若将其作为
  上游，必须在 Harbor/Nexus 一侧实施认证、访问控制和缓存容量限制。
- Singularity/Apptainer：通过 `docker://<本站域名>/<原镜像>` 拉取公开镜像；GPU/HPC 镜像
  仍需遵守上游授权。

## 安全与排障

1. 先请求 `GET /api/sources`，确认目标仍在 `container_registries` 中。
2. `docker login` 的凭据不会自动变成对所有租户安全的共享上游凭据。
3. `401` 可能是正常 Bearer challenge；`403` 常表示上游许可或权限限制。
4. 大镜像验证应同时检查 manifest 与 blob，而不只检查 `/v2/` 返回 200。
5. 不要使用未固定版本、未校验内容的 `curl | sh` 或 `wget | bash` 安装方式。
6. 配置后仍直连上游时，确认改的是当前运行时真正读取的文件；Docker、containerd 与 K3s
   不共用配置。
7. `manifest unknown` 通常表示镜像路径或标签不存在；NAS 图形界面搜索失败也不等于命令行
   拉取失败。

[English](Container-Registries-and-Runtimes)

# NAS、面板与其他运行时

NAS 和服务器面板通常只是管理底层 Docker Engine，也可能只提供自定义 Registry，而不是
真正的 Docker Hub mirror。配置前先区分界面字段含义，并用命令行拉取验证。

## 平台配置索引

| 平台 | 建议入口 | 注意事项 |
| --- | --- | --- |
| 群晖 DSM 7.2+ | Container Manager 的 Registry 设置，或受支持的 daemon 配置 | 旧版套件名为 Docker；GUI 搜索失败时再用 SSH 拉取确认 |
| QNAP | Container Station 的自定义 Registry | 测试连接后仍应实际拉取；不同版本对 Registry 类型要求不同 |
| Unraid | Docker 设置或 Web 终端 | 修改现有 JSON 时只合并 `registry-mirrors`，升级后重新确认持久性 |
| 飞牛 fnOS、极空间、绿联 | 容器应用中的镜像源/自定义仓库 | 若只支持自定义仓库，镜像名必须显式带 MirrorProxy 域名 |
| 宝塔及类似面板 | Docker → 镜像仓库/加速设置 | 区分“Docker Hub mirror”和“自定义 Registry”字段 |
| 爱快等路由面板 | Docker/插件管理中的镜像源 | 保存可能触发 Docker 重启，应提前评估容器中断 |

若面板提供 Docker Engine JSON 编辑器，可在现有对象中合并：

```json
{
  "registry-mirrors": ["https://mirror.example.com"]
}
```

不要覆盖 `data-root`、DNS、日志驱动或自定义 runtime。有效 HTTPS 域名不需要
`insecure-registries`。无法设置 Docker Hub mirror 时，使用完整路径：

```bash
docker pull mirror.example.com/library/nginx:1.27
docker pull mirror.example.com/ghcr.io/owner/image:tag
```

某些 NAS 的 Registry 搜索接口与 Docker pull 链路不同。界面搜索不到镜像时，通过 SSH 执行
`docker pull` 并检查 daemon 日志，不能只凭搜索结果判断代理失效。

## Portainer

可以把 MirrorProxy 添加为自定义 Registry，在创建容器或 Stack 时使用完整镜像地址：

```yaml
services:
  app:
    image: mirror.example.com/ghcr.io/owner/app:1.0
```

Portainer 中“自定义 Registry”不会自动把现有 `docker.io/...`、`ghcr.io/...` 引用全部重写。
检查 Environment、Registry 和 Stack 是否引用同一 endpoint；排障时同时查看 Portainer 日志和
底层 Docker 日志。

## Podman

Podman 配置可能来自 `/etc/containers/registries.conf`、
`/etc/containers/registries.conf.d/` 或用户配置。不同发行版支持的格式不同，最容易审计的
方式是显式引用：

```bash
podman pull mirror.example.com/library/alpine:3.20
podman pull mirror.example.com/quay.io/prometheus/node-exporter:latest
```

需要全局映射时，先阅读当前系统的 `containers-registries.conf` 手册并备份配置。不要把安全的
HTTPS Registry 标记为 `insecure = true`，也不要让短名称搜索静默选择错误 Registry。

## Singularity 与 Apptainer

公开镜像可以使用 Docker transport 和完整 MirrorProxy 路径：

```bash
apptainer pull nginx.sif docker://mirror.example.com/library/nginx:1.27
apptainer pull pause.sif docker://mirror.example.com/registry.k8s.io/pause:3.10
```

GPU/HPC 镜像可能要求上游登录或接受许可；MirrorProxy 不绕过这些授权，也不承诺代理私有
镜像。

## Apple Container

如果当前 Apple Container 版本不支持 Docker 式全局 mirror，直接在 `pull`、`run` 和
Dockerfile `FROM` 中使用完整 MirrorProxy 镜像地址。其 CLI 与配置格式仍在演进，执行前应以
已安装版本的 `container help` 和 Apple 官方文档为准，不要套用 Docker Desktop 配置。

## Harbor 与 Nexus

MirrorProxy 本身已经执行公开镜像的流式代理和缓存。若企业仍需 Harbor/Nexus 提供内部权限、
审计或保留策略，可以把 MirrorProxy 作为公开拉取链路的一层，但应注意：

- 认证、访问控制和容量限制由企业仓库承担。
- 不要把私有上游凭据交给公共 MirrorProxy 实例。
- 双层缓存会增加存储、清理和故障定位成本。
- 推送仍应直接进入企业仓库；MirrorProxy OCI 适配器只承诺公开镜像 `GET`/`HEAD`。

## 通用验证

1. 先访问 `https://mirror.example.com/api/sources`，确认 Registry 在能力列表中。
2. 拉取一个固定标签的小镜像，而不是依赖 GUI 搜索。
3. 检查本地运行时日志与 MirrorProxy 请求日志。
4. 对生产平台安排维护窗口，记录原配置和回滚步骤。
5. 第三方 Registry 未走代理时，检查镜像引用是否包含 MirrorProxy 域名和原 Registry 路径。

[English](NAS-Panels-and-Other-Runtimes) · [返回容器 Registry 总览](Container-Registries-and-Runtimes-zh-CN)

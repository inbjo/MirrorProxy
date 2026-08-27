# containerd 与 K3s

containerd、K3s 和 Docker 使用不同配置。开始前先确认运行时及版本，并备份已有文件：

```bash
containerd --version
sudo cp -a /etc/containerd/config.toml /etc/containerd/config.toml.before-mirrorproxy
```

本页仅把 MirrorProxy 根地址配置为 Docker Hub mirror。其他受支持 Registry 应在 Helm
values、Pod 清单或离线镜像清单中显式改写为
`mirror.example.com/<原 Registry>/<仓库>:<标签>`。

## containerd 1.x

较旧的 CRI 配置通常位于 `/etc/containerd/config.toml`。不要直接用
`containerd config default` 覆盖生产配置；在现有对应节点中合并 Docker Hub endpoint：

```toml
[plugins."io.containerd.grpc.v1.cri".registry.mirrors."docker.io"]
  endpoint = ["https://mirror.example.com", "https://registry-1.docker.io"]
```

不同发行版和 Kubernetes 安装器可能生成不同的 `version` 与插件结构。修改前应检查当前文件，
不能在同一 TOML 中重复声明表。完成后重启并检查日志：

```bash
sudo systemctl restart containerd
sudo systemctl is-active containerd
sudo journalctl -u containerd -n 100 --no-pager
sudo crictl pull docker.io/library/busybox:1.36.1
```

也可用 `sudo nerdctl pull docker.io/library/busybox:1.36.1` 验证。测试命令必须使用运行中
Kubernetes 实际使用的 namespace、socket 和 CRI 配置。

## hosts.toml 模式

新版本 containerd 推荐使用 registry host 配置目录。先确认当前插件节点，并把
`config_path` 指向 `/etc/containerd/certs.d`。containerd 2.x 常见结构为：

```toml
[plugins."io.containerd.cri.v1.images".registry]
  config_path = "/etc/containerd/certs.d"
```

具体插件键以当前 `containerd config dump` 为准。为 Docker Hub 创建：

```text
/etc/containerd/certs.d/docker.io/hosts.toml
```

内容：

```toml
server = "https://registry-1.docker.io"

[host."https://mirror.example.com"]
  capabilities = ["pull", "resolve"]
  skip_verify = false
```

首次设置 `config_path` 后重启 containerd。后续仅更新 `hosts.toml` 是否热加载取决于运行版本，
应通过实际拉取和日志验证，不能只假定配置已经生效。

## K3s

K3s 默认使用内置 containerd，不读取 `/etc/docker/daemon.json`。在每个 server 和 agent 节点
创建或合并 `/etc/rancher/k3s/registries.yaml`：

```yaml
mirrors:
  docker.io:
    endpoint:
      - "https://mirror.example.com"
```

所有可能调度 Pod 的节点都必须配置。仅修改 server 不会解决 agent 上的镜像拉取问题。
在维护窗口分别重启：

```bash
# server 节点
sudo systemctl restart k3s

# agent 节点
sudo systemctl restart k3s-agent
```

验证系统镜像和工作负载镜像：

```bash
sudo k3s crictl pull docker.io/rancher/mirrored-pause:3.6
sudo k3s crictl pull docker.io/library/busybox:1.36.1
sudo journalctl -u k3s -n 100 --no-pager
```

`rancher/...` 仍是 Docker Hub 仓库路径，不应把 `rancher` 配成一个虚构的 Registry。
`k8s.gcr.io` 已是旧地址；新清单应使用 `registry.k8s.io`，再显式改写为
`mirror.example.com/registry.k8s.io/...`。

## 排障清单

| 现象 | 检查项 |
| --- | --- |
| `Failed to create pod sandbox` | 在发生调度的节点直接用 `crictl pull` 拉取 pause 镜像 |
| 仍访问官方源 | 确认配置文件、插件键、运行时 socket 与服务进程相匹配 |
| `no matching endpoint` | 检查 endpoint 拼写、HTTPS 证书和 TOML/YAML 语法 |
| 只有部分节点失败 | 对比每个 server/agent 的配置、服务状态和 DNS |
| 第三方 Registry 未走代理 | 显式改写镜像引用；Docker Hub mirror 不覆盖其他 Registry |

不要用 `skip_verify = true` 掩盖证书问题。应为 MirrorProxy 配置有效证书，或在受控企业环境中
正确安装私有 CA。

[English](Containerd-and-K3s) · [返回容器 Registry 总览](Container-Registries-and-Runtimes-zh-CN)

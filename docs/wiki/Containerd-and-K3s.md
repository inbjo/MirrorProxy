# containerd and K3s

containerd, K3s, and Docker use different configuration files. Identify the runtime and version,
then back up the current configuration:

```bash
containerd --version
sudo cp -a /etc/containerd/config.toml /etc/containerd/config.toml.before-mirrorproxy
```

This page configures the MirrorProxy root endpoint as a Docker Hub mirror only. Rewrite images from
other supported registries explicitly in Helm values, workload manifests, or offline image lists as
`mirror.example.com/<original-registry>/<repository>:<tag>`.

## containerd 1.x

Older CRI installations commonly use `/etc/containerd/config.toml`. Do not replace production
configuration with the output of `containerd config default`. Merge this Docker Hub endpoint into
the matching existing section:

```toml
[plugins."io.containerd.grpc.v1.cri".registry.mirrors."docker.io"]
  endpoint = ["https://mirror.example.com", "https://registry-1.docker.io"]
```

Distribution packages and Kubernetes installers may generate different `version` values and plugin
trees. Inspect the current file and avoid declaring the same TOML table twice. Restart and verify:

```bash
sudo systemctl restart containerd
sudo systemctl is-active containerd
sudo journalctl -u containerd -n 100 --no-pager
sudo crictl pull docker.io/library/busybox:1.36.1
```

`sudo nerdctl pull docker.io/library/busybox:1.36.1` is another useful check. Use the namespace,
socket, and CRI configuration of the containerd instance that Kubernetes actually runs.

## hosts.toml mode

Current containerd releases support registry host configuration directories. Confirm the active
plugin tree, then point `config_path` at `/etc/containerd/certs.d`. A common containerd 2.x shape is:

```toml
[plugins."io.containerd.cri.v1.images".registry]
  config_path = "/etc/containerd/certs.d"
```

Treat `containerd config dump` as authoritative for the running installation. Create:

```text
/etc/containerd/certs.d/docker.io/hosts.toml
```

with:

```toml
server = "https://registry-1.docker.io"

[host."https://mirror.example.com"]
  capabilities = ["pull", "resolve"]
  skip_verify = false
```

Restart containerd after setting `config_path`. Whether later `hosts.toml` changes reload without a
restart depends on the deployed version; prove the result with a real pull and logs.

## K3s

K3s normally uses its bundled containerd and does not read `/etc/docker/daemon.json`. Create or
merge `/etc/rancher/k3s/registries.yaml` on every server and agent node:

```yaml
mirrors:
  docker.io:
    endpoint:
      - "https://mirror.example.com"
```

Every node that may run a Pod needs the setting. Updating only the server does not fix image pulls
on agents. Restart each role during a maintenance window:

```bash
# Server node
sudo systemctl restart k3s

# Agent node
sudo systemctl restart k3s-agent
```

Verify system and workload images:

```bash
sudo k3s crictl pull docker.io/rancher/mirrored-pause:3.6
sudo k3s crictl pull docker.io/library/busybox:1.36.1
sudo journalctl -u k3s -n 100 --no-pager
```

`rancher/...` is still a Docker Hub repository path; do not invent a `rancher` registry entry.
`k8s.gcr.io` is legacy. New manifests should use `registry.k8s.io`, then explicitly rewrite the
reference as `mirror.example.com/registry.k8s.io/...`.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| `Failed to create pod sandbox` | Pull the pause image with `crictl` on the node where scheduling failed |
| Traffic still goes upstream | Match the file, plugin key, runtime socket, and running service |
| `no matching endpoint` | Check endpoint spelling, HTTPS certificates, and TOML/YAML syntax |
| Only some nodes fail | Compare configuration, service state, and DNS on every server and agent |
| A third-party registry bypasses the proxy | Rewrite the image explicitly; a Docker Hub mirror does not cover other registries |

Do not hide certificate failures with `skip_verify = true`. Give MirrorProxy a valid certificate,
or install the correct private CA in a controlled enterprise environment.

[简体中文](Containerd-and-K3s-zh-CN) · [Container registry overview](Container-Registries-and-Runtimes)

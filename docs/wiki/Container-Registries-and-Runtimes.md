# Container registries and runtimes

MirrorProxy exposes one OCI Distribution endpoint for multiple public container registries. The
Container Registry Workbench on the home page reads the live `/api/sources` capability catalog,
validates image references, and rewrites Compose YAML or Dockerfiles.

Choose the guide for your environment:

- [Docker Engine and desktop runtimes](Docker-Engine-and-Desktop)
- [containerd and K3s](Containerd-and-K3s)
- [NAS, panels, and other runtimes](NAS-Panels-and-Other-Runtimes)

## Supported registries

| Registry | Original image | MirrorProxy path |
| --- | --- | --- |
| Docker Hub | `nginx:latest` | `<mirror-host>/nginx:latest` |
| GHCR | `ghcr.io/owner/image:tag` | `<mirror-host>/ghcr.io/owner/image:tag` |
| Quay | `quay.io/owner/image:tag` | `<mirror-host>/quay.io/owner/image:tag` |
| Kubernetes | `registry.k8s.io/pause:3.10` | `<mirror-host>/registry.k8s.io/pause:3.10` |
| GCR | `gcr.io/project/image:tag` | `<mirror-host>/gcr.io/project/image:tag` |
| Microsoft MCR | `mcr.microsoft.com/dotnet/runtime:8.0` | `<mirror-host>/mcr.microsoft.com/dotnet/runtime:8.0` |
| Elastic | `docker.elastic.co/elasticsearch/elasticsearch:8.13.4` | `<mirror-host>/docker.elastic.co/elasticsearch/elasticsearch:8.13.4` |
| GitLab | `registry.gitlab.com/group/project/image:tag` | `<mirror-host>/registry.gitlab.com/group/project/image:tag` |
| NVIDIA NVCR | `nvcr.io/nvidia/cuda:tag` | `<mirror-host>/nvcr.io/nvidia/cuda:tag` |
| Oracle | `container-registry.oracle.com/os/oraclelinux:tag` | `<mirror-host>/container-registry.oracle.com/os/oraclelinux:tag` |

`k8s.gcr.io` is accepted only as a legacy input alias and is served from the current
`registry.k8s.io` upstream. The OCI adapter is a public, read-only `GET`/`HEAD` pull path;
private-image credentials, license acceptance, pushes, deletes, and signing writes are outside this
contract.

Private GitLab projects, NGC organization or team images, and Oracle images that require login or
license acceptance never borrow server-side MirrorProxy credentials. Upstream `401` and `403`
responses remain authorization boundaries. A registry in this table means that anonymous public
projects can be pulled, not that private content is available.

## Docker Engine: global Docker Hub acceleration

Docker's `registry-mirrors` setting applies only to Docker Hub. Preview and then apply the system
configuration:

```bash
mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system --dry-run
sudo mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system
```

The client structurally merges `/etc/docker/daemon.json`, preserves fields such as `data-root`,
`dns`, and runtimes, and stores a rollback record under
`/var/lib/mirrorproxy/sources/docker.json`. It does not restart Docker automatically:

```bash
sudo systemctl restart docker
docker info | sed -n '/Registry Mirrors/,+5p'
docker pull busybox:1.36.1
```

Restore the previous file with `sudo mirrorproxy reset docker --scope system`, then restart Docker
during a maintenance window.

## Desktop, Compose, Dockerfile, containerd, and K3s

Docker Desktop and compatible NAS panels can merge this into their Docker Engine settings:

```json
{ "registry-mirrors": ["https://mirror.example.com"] }
```

For every non-Docker-Hub registry, rewrite the image explicitly:

```yaml
services:
  api:
    image: mirror.example.com/ghcr.io/owner/api:1.0
```

```dockerfile
FROM mirror.example.com/mcr.microsoft.com/dotnet/runtime:8.0
```

K3s can use the Docker Hub endpoint in `/etc/rancher/k3s/registries.yaml`:

```yaml
mirrors:
  docker.io:
    endpoint:
      - "https://mirror.example.com"
```

For other registries, prefer explicit image rewrites in Helm values and workload manifests. Do not
assume Docker's `registry-mirrors` setting covers GHCR, Quay, GCR, or MCR.

## Podman and repository managers

- Prefer explicit MirrorProxy image references with Podman; validate the distribution's
  `registries.conf` version before editing system configuration.
- Portainer can use the site as a custom registry while stacks use rewritten image references.
- If Harbor or Nexus uses MirrorProxy as an upstream, enforce authentication and cache limits on
  the private repository manager. Do not place private upstream credentials on a public instance.
- Singularity and Apptainer can pull public images with
  `docker://<mirror-host>/<original-image>`; upstream GPU/HPC license requirements still apply.

## Verification and safety

1. Check `GET /api/sources` and its `container_registries` list first.
2. Treat `401` as a possible Bearer challenge; `403` commonly indicates upstream policy.
3. Verify both manifests and blobs rather than relying on a successful `/v2/` ping.
4. Avoid unversioned, unchecked `curl | sh` or `wget | bash` installers.
5. If traffic still goes directly upstream, confirm that you edited the file used by the active
   runtime. Docker, containerd, and K3s do not share configuration.
6. `manifest unknown` usually means the image path or tag does not exist. A failed search in a NAS
   GUI does not prove that command-line pulls are broken.

[简体中文](Container-Registries-and-Runtimes-zh-CN)

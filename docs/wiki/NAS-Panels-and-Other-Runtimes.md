# NAS, panels, and other runtimes

NAS products and server panels may manage an underlying Docker Engine, or they may expose only a
custom-registry form rather than a Docker Hub mirror. Identify what the field means and prove the
result with a command-line pull.

## Platform index

| Platform | Suggested entry point | Notes |
| --- | --- | --- |
| Synology DSM 7.2+ | Container Manager registry settings or supported daemon configuration | Older releases call the package Docker; use an SSH pull when GUI search fails |
| QNAP | Container Station custom registry | Test the connection and then pull an image; registry-type requirements vary by version |
| Unraid | Docker settings or web terminal | Merge only `registry-mirrors` into existing JSON and recheck persistence after upgrades |
| fnOS, ZSpace, UGREEN | Registry or custom repository in the container application | If only custom registries are supported, use a fully qualified MirrorProxy image |
| aaPanel and similar panels | Docker image-registry or acceleration settings | Distinguish a Docker Hub mirror from a custom registry field |
| Router panels such as iKuai | Image source in Docker or plugin management | Saving may restart Docker and interrupt running containers |

When a panel exposes the Docker Engine JSON, merge this into the existing object:

```json
{
  "registry-mirrors": ["https://mirror.example.com"]
}
```

Do not replace `data-root`, DNS, logging, or runtime settings. Valid HTTPS does not require
`insecure-registries`. If the platform cannot set a Docker Hub mirror, use a full reference:

```bash
docker pull mirror.example.com/library/nginx:1.27
docker pull mirror.example.com/ghcr.io/owner/image:tag
```

Some NAS search APIs differ from the Docker pull path. If GUI search finds nothing, run
`docker pull` over SSH and inspect daemon logs before declaring the proxy unavailable.

## Portainer

Add MirrorProxy as a custom registry and use full image references in containers and stacks:

```yaml
services:
  app:
    image: mirror.example.com/ghcr.io/owner/app:1.0
```

A Portainer custom registry does not automatically rewrite every existing `docker.io/...` or
`ghcr.io/...` reference. Check that the Environment, Registry, and Stack use the intended endpoint,
then inspect both Portainer and Docker logs when troubleshooting.

## Podman

Podman may read `/etc/containers/registries.conf`, files under
`/etc/containers/registries.conf.d/`, or user configuration. Formats vary across distributions, so
explicit references are the easiest option to audit:

```bash
podman pull mirror.example.com/library/alpine:3.20
podman pull mirror.example.com/quay.io/prometheus/node-exporter:latest
```

Before configuring global remapping, read the installed `containers-registries.conf` manual and
back up the file. Do not mark valid HTTPS as `insecure = true`, and do not let short-name search
silently select an unintended registry.

## Singularity and Apptainer

Pull public images through the Docker transport and a full MirrorProxy path:

```bash
apptainer pull nginx.sif docker://mirror.example.com/library/nginx:1.27
apptainer pull pause.sif docker://mirror.example.com/registry.k8s.io/pause:3.10
```

GPU and HPC images may require upstream authentication or license acceptance. MirrorProxy does not
bypass those controls and does not promise private-image proxying.

## Apple Container

If the installed Apple Container release does not support Docker-style global mirrors, use full
MirrorProxy references in `pull`, `run`, and Dockerfile `FROM` instructions. Its CLI and
configuration are evolving; check `container help` and current Apple documentation instead of
applying Docker Desktop settings.

## Harbor and Nexus

MirrorProxy already streams and caches public images. An organization may still place Harbor or
Nexus in the path for internal access control, audit, and retention, but:

- The private repository manager must enforce authentication, access control, and capacity limits.
- Do not give private upstream credentials to a public MirrorProxy instance.
- Two cache layers increase storage, cleanup, and incident-diagnosis costs.
- Push directly to the private repository manager. MirrorProxy's OCI adapter promises public
  `GET`/`HEAD` pulls only.

## General verification

1. Read `https://mirror.example.com/api/sources` and confirm the registry is advertised.
2. Pull a small image at a fixed tag instead of relying on GUI search.
3. Inspect both local runtime logs and MirrorProxy request logs.
4. Record the original configuration and rollback procedure before a production maintenance window.
5. If a third-party registry bypasses the proxy, check for both the MirrorProxy hostname and the
   original registry path in the image reference.

[简体中文](NAS-Panels-and-Other-Runtimes-zh-CN) · [Container registry overview](Container-Registries-and-Runtimes)

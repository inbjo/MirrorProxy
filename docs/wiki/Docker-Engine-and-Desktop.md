# Docker Engine and desktop runtimes

This page configures MirrorProxy for Docker Engine, Docker Desktop, and OrbStack. Replace
`mirror.example.com` with your deployment hostname.

## Linux Docker Engine

Use the client to merge `/etc/docker/daemon.json` structurally:

```bash
mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system --dry-run
sudo mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example.com \
  --scope system
```

Review the preview, then restart Docker during a maintenance window:

```bash
sudo systemctl restart docker
docker info | sed -n '/Registry Mirrors/,+5p'
docker pull busybox:1.36.1
```

The client changes only `registry-mirrors`, preserves existing JSON fields, and keeps rollback
state. If Docker fails to start, run
`dockerd --validate --config-file /etc/docker/daemon.json` and inspect
`journalctl -u docker -n 100`.

## Docker Desktop

1. Open Settings → Docker Engine.
2. Merge this field into the existing JSON without deleting other keys:

   ```json
   {
     "registry-mirrors": ["https://mirror.example.com"]
   }
   ```

3. Select Apply & Restart.
4. Run `docker info` and a real `docker pull` from a terminal.

A hostname with valid HTTPS does not belong in `insecure-registries`. If enterprise policy manages
Docker Desktop, have the administrator distribute the setting instead of overwriting it locally.

## OrbStack

Open the Docker configuration in OrbStack settings, or review
`~/.orbstack/config/docker.json`, merge the same `registry-mirrors` field, and follow the restart
prompt. Verify the result with `docker info`.

## Compose and Dockerfiles

Docker Hub images can keep their original references. Other supported registries require explicit
rewrites:

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

The Container Registry Workbench can rewrite Compose YAML and Dockerfiles. Review the diff before
committing it, and pin a deliberate tag or digest in CI.

## Rollback and troubleshooting

```bash
sudo mirrorproxy reset docker --scope system --dry-run
sudo mirrorproxy reset docker --scope system
sudo systemctl restart docker
```

- Restarting Docker may interrupt containers. Use a maintenance window and preserve an independent
  management connection.
- `registry-mirrors` covers Docker Hub only, not GHCR, GCR, Quay, or MCR.
- A `docker info` entry proves only that Docker loaded the setting; verify a manifest and blob pull.
- Fully qualified third-party images in Dockerfiles or Compose require explicit MirrorProxy paths.

[简体中文](Docker-Engine-and-Desktop-zh-CN) · [Container registry overview](Container-Registries-and-Runtimes)

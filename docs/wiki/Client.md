# Standalone Client

`mirrorproxy` is an independent binary: it neither connects to nor manages the
server database. It uses the shared catalog to write local source configuration
and preserves previous content for exact reset. Stable releases include Linux
x86_64/arm64, macOS Intel/Apple Silicon, and Windows x64 assets with checksums.

## Install

Linux/macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
mirrorproxy --version
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
mirrorproxy --version
```

The scripts verify the Release checksum. Set `MIRRORPROXY_VERSION` or
`MIRRORPROXY_INSTALL_DIR` to pin a version or change the destination.

## Common commands

```bash
# `sources` is optional, similar to chsrc.
mirrorproxy list
mirrorproxy list --category lang
mirrorproxy mirrors
mirrorproxy get npm --base-url https://mirror.example.com
mirrorproxy set npm --base-url https://mirror.example.com
mirrorproxy reset npm
```

`set` defaults to user scope and refuses to overwrite a non-empty configuration;
use `--force` only after review. `--dry-run` previews changes. The `mirrorproxy`
provider requires `--base-url` in `http(s)://host[:port]` form, without a package
repository path.

## Scope and safety

User scope covers npm, pip, Cargo, Go, Composer, and other supported targets.
System scope is directly supported on Linux and changes APT, DNF, pacman, apk,
zypper, xbps, Portage, and related system configurations; it needs normal
administrator rights and APT-like targets may require `--distribution`. Check
`get` and `--dry-run` before writing. The client never uploads credentials or
original configuration.

[简体中文](Client-zh-CN)

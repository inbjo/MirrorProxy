# Client Distribution

This page describes only the currently published client distribution. The
standalone Rust `mirrorproxy` binary does not need Python, Node.js, or the server
runtime. It is **not currently published** through pip, npm, APT, Homebrew,
Scoop, winget, or crates.io.

## Stable release assets

Each stable `v*` tag on [GitHub Releases](https://github.com/inbjo/MirrorProxy/releases) contains:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `mirrorproxy-client-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `mirrorproxy-client-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `mirrorproxy-client-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `mirrorproxy-client-x86_64-pc-windows-msvc.zip` |

Every asset has a matching `.sha256` file, and the release also includes an
aggregate `SHA256SUMS`. Verify the checksum before installation.

## Install scripts

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

The scripts download the latest stable release, verify SHA-256, and install to a
default directory. Version, destination, and download prefix can be explicit:

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- \
  --version v1.1.0 --install-dir "$HOME/.local/bin"
```

Equivalent environment variables are `MIRRORPROXY_VERSION`,
`MIRRORPROXY_INSTALL_DIR`, `MIRRORPROXY_DOWNLOAD_MIRROR`, and
`MIRRORPROXY_GITHUB_REPO`. The Windows script accepts `-Version`, `-InstallDir`,
`-Mirror`, and `-Repository`.

## Manual installation

Download the matching asset and `.sha256` from a Release, verify it, extract it,
and put `mirrorproxy` (`mirrorproxy.exe` on Windows) in a directory on `PATH`.
The archives contain only the client executable.

Run `mirrorproxy --version` and `mirrorproxy list` after installation. See
[Standalone Client](Client) for client operations.

[简体中文](Distribution-zh-CN)

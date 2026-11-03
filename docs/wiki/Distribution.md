# Client Distribution

This page describes distribution of the standalone `mirrorproxy` client. It
does not need Python, Node.js, or the server runtime. Stable releases are
distributed through GitHub Releases, a Homebrew tap, WinGet, and a signed APT
repository. See the deployment guide for the server.

> **These methods install the `mirrorproxy` client only.** For the server, see [Deployment](Deployment); for client commands, see [Client](Client).

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
  --version v1.5.0 --install-dir "$HOME/.local/bin"
```

Equivalent environment variables are `MIRRORPROXY_VERSION`,
`MIRRORPROXY_INSTALL_DIR`, `MIRRORPROXY_DOWNLOAD_MIRROR`, and
`MIRRORPROXY_GITHUB_REPO`. The Windows script accepts `-Version`, `-InstallDir`,
`-Mirror`, and `-Repository`.

## Homebrew (macOS/Linux)

Add the official tap once, then install and upgrade normally:

```bash
brew tap inbjo/tap
brew install mirrorproxy
brew upgrade mirrorproxy
```

`brew install inbjo/tap/mirrorproxy` combines the first two commands. A fresh
Homebrew installation can omit the tap only after the formula is accepted into
Homebrew Core.

## WinGet (Windows)

After `Inbjo.MirrorProxy` is accepted into the Microsoft WinGet Community
Repository:

```powershell
winget install --id Inbjo.MirrorProxy --exact
winget upgrade --id Inbjo.MirrorProxy --exact
```

Stable releases generate the multi-file WinGet manifest required for the first
submission. After that submission is accepted, the release workflow can open
version-update pull requests automatically.

## APT (Debian/Ubuntu)

Add the signing key and repository once:

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/apt/mirrorproxy-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/mirrorproxy-archive-keyring.gpg >/dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/mirrorproxy-archive-keyring.gpg] https://raw.githubusercontent.com/inbjo/MirrorProxy/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/mirrorproxy.list >/dev/null
sudo apt update
sudo apt install mirrorproxy
```

Normal `apt update` and `apt upgrade` operations then install future releases.
The repository contains both `amd64` and `arm64` client packages. The
`mirrorproxy` client and `mirrorproxy-server` service use distinct package names.

## Stable release assets

Each stable `v*` tag on [GitHub Releases](https://github.com/inbjo/MirrorProxy/releases) contains:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `mirrorproxy-client-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `mirrorproxy-client-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `mirrorproxy-client-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `mirrorproxy-client-x86_64-pc-windows-msvc.exe` / `.zip` |
| Windows ARM64 | `mirrorproxy-client-aarch64-pc-windows-msvc.exe` / `.zip` |
| Debian/Ubuntu x86_64 | `mirrorproxy_<version>_amd64.deb` |
| Debian/Ubuntu ARM64 | `mirrorproxy_<version>_arm64.deb` |
| RPM Linux x86_64 | `mirrorproxy-<version>-1.x86_64.rpm` |
| RPM Linux ARM64 | `mirrorproxy-<version>-1.aarch64.rpm` |

Every asset has a matching `.sha256` file, and the release also includes an
aggregate `SHA256SUMS`. Verify the checksum before installation.


Windows ARM64, standalone EXEs and Linux client RPMs are available from v1.7.2. Windows EXEs are portable command-line programs, without a graphical installer. ZIP archives remain available for the installer script, Scoop and WinGet. macOS uses TAR.GZ or Homebrew; no DMG is published.

Windows binaries require the matching Microsoft Visual C++ 2015+ Redistributable
(x64 or ARM64). WinGet declares and installs this dependency. For direct EXE/ZIP
downloads, ensure that runtime is installed before running the client.

Install a Linux RPM with `sudo dnf install ./mirrorproxy-<version>-1.x86_64.rpm` and remove it with `sudo dnf remove mirrorproxy`; choose the matching file for ARM64. Removing the package does not undo source configuration changes; use the client's rollback first.

## Server assets

Server builds target Windows and Linux, both x64/ARM64; no macOS server is built.
Linux provides TAR.GZ, DEB and RPM packages. Windows provides
`mirrorproxy-server-<target>.zip` containing `mirrorproxy-server.exe`,
`config.example.toml` and the `geoip` directory. Verify the checksum, extract the
complete archive and run from the extracted directory:

```powershell
.\mirrorproxy-server.exe --config config.example.toml serve
```

The Windows server runs in the foreground; the `install` command manages Linux
systemd only. Preserve configuration, database, cache and GeoIP data during an
upgrade or rollback, replacing only the necessary program files.

## Manual installation

Download the matching asset and `.sha256` from a Release, verify it, extract it,
and put `mirrorproxy` (`mirrorproxy.exe` on Windows) in a directory on `PATH`.
The archives contain only the client executable.

Run `mirrorproxy --version` and `mirrorproxy list` after installation. See
[Standalone Client](Client) for client operations.

[Maintainer publishing setup](Release-Maintenance) · [简体中文](Distribution-zh-CN)

### Proxy both the script and release downloads

Proxying the script URL does not proxy subsequent release downloads. Pass `--mirror` explicitly:

```bash
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- --mirror https://sina.dev
```

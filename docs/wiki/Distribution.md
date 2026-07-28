# Client distribution and package managers

`mirrorproxy` is a standalone Rust command-line program. Every stable `v*` tag
publishes prebuilt Linux, macOS and Windows clients with SHA-256 checksums. This
is the supported installation path today and needs no Python or Node.js runtime.

## Install now

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
mirrorproxy --version
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
mirrorproxy --version
```

The scripts download the latest stable asset and verify its published SHA-256.
Use `MIRRORPROXY_VERSION`, `MIRRORPROXY_INSTALL_DIR`, or
`MIRRORPROXY_DOWNLOAD_MIRROR` to control the version, destination, or download
mirror. Assets are also available from [GitHub Releases](https://github.com/inbjo/MirrorProxy/releases).

## Channel decision

| Channel | Feasible | Recommendation | Why |
| --- | --- | --- | --- |
| Homebrew | Yes | **First priority** | Natural macOS/Linux distribution for a native CLI; a separate tap is sufficient. |
| APT | Yes | Second phase | Needs `.deb` files, a signing key, an HTTPS repository, and a keyring package. |
| PyPI / pip | Wrapper only | Not primary | Wheels or a downloader wrapper are needed per Python/OS/CPU combination. |
| npm | Wrapper only | Not primary | Platform optional packages or a downloader wrapper add a Node.js runtime. |
| crates.io | Yes | Optional | Useful to Rust users after crate metadata and release policy are completed. |
| Scoop / winget | Yes | Windows second phase | Better native Windows CLI channels than npm. |

## Recommended rollout

1. Keep GitHub Releases and checksums as the single source of client artifacts.
2. Create `inbjo/homebrew-tap` and publish `brew install inbjo/tap/mirrorproxy`;
   update the formula version and macOS checksums on each release.
3. Build amd64 and arm64 `.deb` assets, then publish a signed APT repository
   with a dedicated `mirrorproxy-archive-keyring` package.
4. Add PyPI/npm **download wrappers** only if their ecosystems are required.
   A wrapper must select and verify the existing Release binary, not reimplement
   the client.

APT repositories should use a dedicated keyring and `Signed-By`, rather than a
global trusted key; see [Debian's third-party repository guidance](https://wiki.debian.org/DebianRepository/UseThirdParty).
Homebrew taps are ordinary Git repositories and support `brew install owner/repository/formula`; see
[Homebrew's tap guide](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap).

## Required authority

Publishing creates external artifacts, so workflows remain disabled until the
repository owner provides the appropriate access: a tap write token, APT hosting
and a restricted signing subkey, PyPI Trusted Publishing (preferred) or a
project token, and npm 2FA/granular publish access. Until then, Releases and the
install scripts remain the secure supported channel.

[Client](Client) · [Development](Development-and-Roadmap) · [简体中文](Distribution-zh-CN)

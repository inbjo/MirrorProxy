# Client Distribution

This page describes distribution of the standalone `mirrorproxy` client. It
does not need Python, Node.js, or the server runtime. Stable releases are
distributed through GitHub Releases, a Homebrew tap, WinGet, and a signed APT
repository. See the deployment guide for the server.

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
| Windows x64 | `mirrorproxy-client-x86_64-pc-windows-msvc.zip` |
| Debian/Ubuntu x86_64 | `mirrorproxy_<version>_amd64.deb` |
| Debian/Ubuntu ARM64 | `mirrorproxy_<version>_arm64.deb` |

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

## One-time publisher setup

1. Create the public `inbjo/homebrew-tap` repository with `main` as its default branch.
2. Create a fine-grained PAT with Contents read/write access only to that tap and
   save it as the `HOMEBREW_TAP_TOKEN` Actions secret in this repository.
3. Create a dedicated APT signing key. Save its ASCII-armored private key as
   `APT_GPG_PRIVATE_KEY` and, when applicable, its passphrase as
   `APT_GPG_PASSPHRASE`. Never commit the private key.

   ```bash
   gpg --quick-generate-key "MirrorProxy APT Repository <noreply@github.com>" ed25519 sign 3y
   gpg --list-secret-keys --keyid-format=long
   gpg --armor --export-secret-keys <fingerprint> >mirrorproxy-apt-private.asc
   ```

   After storing the complete exported file in the secret, back it up offline
   and remove it from the working directory. Losing the key prevents existing
   clients from authenticating future repository updates.
4. The Release workflow publishes the signed repository to the dedicated `apt`
   branch at `https://raw.githubusercontent.com/inbjo/MirrorProxy/apt`.
5. After the first WinGet manifest is accepted into `microsoft/winget-pkgs`, add
   a `WINGET_TOKEN` capable of creating the update PR and set the repository
   variable `WINGET_AUTO_SUBMIT` to `true`.

   For the first release, download `mirrorproxy-winget-manifests.zip` from the
   GitHub Release, run `winget validate <manifest-directory>`, and submit the
   `manifests/i/Inbjo/MirrorProxy/<version>` directory to
   `microsoft/winget-pkgs`. Do not enable automatic submission until it merges.

On a stable `v*` tag, the release workflow builds the client packages and
manifests, signs and deploys the APT repository, and updates the Homebrew tap.
WinGet submission runs only after it is explicitly enabled.

[简体中文](Distribution-zh-CN)

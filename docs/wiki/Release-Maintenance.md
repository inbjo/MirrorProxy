# Release maintainer setup

For release maintainers only. To install the client, see [Client installation](Distribution).

1. Create the public `inbjo/homebrew-tap` repository with `main` as its default branch.
2. Create a fine-grained PAT with Contents read/write access only to that tap and
   save it as the `HOMEBREW_TAP_TOKEN` Actions secret in this repository.
3. Create a dedicated APT signing key. Save its ASCII-armored private key as
   `APT_GPG_PRIVATE_KEY` and, when applicable, its passphrase as
   `APT_GPG_PASSPHRASE`. Never commit the private key.

   ```bash
   gpg --quick-generate-key "MirrorProxy APT Repository <noreply@github.com>" ed25519 sign 3y
   gpg --list-secret-keys --keyid-format=long
   gpg --armor --export-secret-keys YOUR_KEY_FINGERPRINT >mirrorproxy-apt-private.asc
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

## Native Docker builds and caches

Docker builds run `linux/amd64` on `ubuntu-24.04` and `linux/arm64` on
`ubuntu-24.04-arm`, without QEMU. Each native image passes the entrypoint version,
revision and local-upstream protocol smoke before its digest is made available
to the merge job. The merge job verifies both architecture entries and their
linked attestations, then signs the final index with GitHub OIDC. SBOM and
provenance remain enabled for published images.

Cargo registry, Git and target cache mounts are restored and extracted through
the pinned cache-dance action and `actions/cache`. Keys separate architectures
and include Dockerfile, Cargo lock/manifests and commit; fallback keys reuse
compatible prior builds. BuildKit layer caches also have per-architecture scopes.
PR and relevant `main` pushes build and test locally without publishing; stable
tags and explicit dispatches publish by digest and merge tags only after both
platforms pass. Digest-only dispatch still signs without rebuilding.

To validate a cache change, run two native builds, inspect restored/extracted
cache steps and Cargo rebuild timings, then run the image smoke on both runners.
Do not infer speedups from a successful YAML check or an unchanged cached layer.

### Other workflow caches

Release clients/servers and all Rust CI jobs cache dependency downloads,
compiled dependencies and installed Cargo tools with a pinned Node-24-compatible
Rust-cache action. Keys include compiler/toolchain, manifests/lockfile,
compiler environment, OS, runner architecture, target and build purpose.
Coverage and CodeQL analysis are isolated from normal compilation; CodeQL
caches dependencies only. Cargo commands and all tests/audits still execute.

CI and Release each build the frontend once. A content-keyed cache binds the
output to frontend sources/configuration, lockfile, exact Node version,
platform and build environment. File hashes and the complete output inventory
are checked before reuse. The current run's frontend artifact additionally
records its Git commit; every Rust consumer verifies the commit and hashes
before embedding it. Unit, toolchain and browser tests still run on cache hits.
Release shares one checksum-verified GeoIP input across server targets. Only
client/server distribution artifacts are selected for publication; internal
frontend/GeoIP artifacts are excluded from public release assets.

Playwright browser downloads, npm package downloads and the fixed-checksum
Windows Perl archive are cached. Browser system dependencies and Perl archive
hashes are checked every run. Real-client smoke homes/caches remain isolated so
cached package payloads cannot conceal proxy regressions. A cold run still
needs to populate these caches; no build-time guarantee is implied.

## Release download tables

The Release workflow runs `scripts/generate-release-body.py` against the built
artifacts. Both stable releases and Nightly show separate client/server tables
with operating-system and architecture columns, plus a `SHA256SUMS` link. Only
existing archives and native packages are linked; unavailable platforms show `—`.
Client tables include Windows, macOS and Linux; server tables include Windows and Linux only. Windows clients provide EXE/ZIP, Linux clients TAR.GZ/DEB/RPM, and macOS clients retain TAR.GZ/Homebrew. PowerShell, Scoop and WinGet support both x64 and ARM64; WinGet submits the complete generated manifests for this release.
Stable releases include `docs/releases/vX.Y.Z.md` below the tables.

When applying curated notes manually, preserve the tables by generating the
combined body from the downloaded release assets, then use that output with
`gh release edit --notes-file`. Replacing the body with the notes file alone
removes the download tables. Updating this generator does not modify existing
published releases.

WinGet manifests keep the common portable executable entry at the installer
root and retain release date, release-notes and Wiki metadata. Each architecture
declares its matching VC++ Runtime dependency. When repairing published
metadata, set `MIRRORPROXY_RELEASE_DATE` to the original Release's UTC date.
Run `gh workflow run winget-validation.yml -f version=X.Y.Z` to validate the
published package on Windows x64 and ARM64: install, command alias discovery,
version/catalog commands and uninstall. This workflow uses ephemeral runners.


[简体中文](Release-Maintenance-zh-CN)

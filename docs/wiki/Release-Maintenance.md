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

## Release download tables

The Release workflow runs `scripts/generate-release-body.py` against the built
artifacts. Both stable releases and Nightly show separate client/server tables
with operating-system and architecture columns, plus a `SHA256SUMS` link. Only
existing archives and native packages are linked; unavailable platforms show `—`.
Stable releases include `docs/releases/vX.Y.Z.md` below the tables.

When applying curated notes manually, preserve the tables by generating the
combined body from the downloaded release assets, then use that output with
`gh release edit --notes-file`. Replacing the body with the notes file alone
removes the download tables. Updating this generator does not modify existing
published releases.


[简体中文](Release-Maintenance-zh-CN)

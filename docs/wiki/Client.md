# Standalone Client

> **This configures your computer; it does not deploy a server.** Have an existing server HTTPS root URL such as `https://mirror.example.com`, without `/npm` or other repository paths.

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

### Check and install updates

```bash
mirrorproxy update --check
mirrorproxy update
```

After a successful interactive source command, the client checks at most once
every 24 hours and prints a notice when a newer stable release is available.
Run `mirrorproxy update` to install it. Check failures do not affect the
original command. Scripts, MCP, and `--version` do not trigger checks.
Version checks work for any installation. Binary replacement supports the
standalone installer's default directories: `/usr/local/bin` and `~/.local/bin`
on Unix, and `%LOCALAPPDATA%\Programs\MirrorProxy\bin` on Windows. System
directories may require administrator privileges. For APT, Homebrew, WinGet or
Scoop installations, use the corresponding package manager. Reinstall older
standalone clients once to get the update command.

```bash
# `sources` is optional, similar to chsrc.
mirrorproxy list
mirrorproxy list --category lang
mirrorproxy mirrors
# After installing with --mirror or -Mirror
mirrorproxy set pip
mirrorproxy set go
mirrorproxy set rust     # Cargo; cargo is also accepted
mirrorproxy set npm
mirrorproxy reset npm
```

The POSIX installer's `--mirror` and the PowerShell installer's `-Mirror` save the
instance. For package-manager installations, run `mirrorproxy set go https://mirror.example.com`
or save the instance with `mirrorproxy use https://mirror.example.com --user`.
The short commands for pip, Go, Cargo, npm, Bun, PDM, and Homebrew merge source
settings into existing configuration. Other targets retain their own overwrite
protections. Rustup download settings have a separate configuration example.

For Hugging Face, `mirrorproxy get hf --base-url https://mirror.example.com`
prints the platform-specific environment settings without writing shell profiles
or tokens. See [Hugging Face downloads](Proxy-Adapters#hugging-face-public-http-downloads)
for the required `HF_HUB_DISABLE_XET=1` setting and supported scope.

## Advanced: scope and safety

For the unified 31-distribution native migration, version matrix and Linux
verification, see [Linux system sources](System-Sources). Run `mirrorproxy systems`
to inspect the supported profiles. Existing target-specific commands remain available.

User scope covers npm, pip, Cargo, Go, Composer, and other supported targets.
System scope is directly supported on Linux and changes APT, DNF, pacman, apk,
zypper, xbps, Portage, and related system configurations. System source changes
require administrator privileges. The client never uploads credentials or original
configuration.

Manual setup in the source catalog configures defaults across projects.
For example, npm uses `npm config set registry https://mirror.example.com/npm/ --location=global`.
Tools with per-user global configuration use their user files or environment.
Poetry configures download sources per project in `pyproject.toml`.

## Debian and Ubuntu system sources

After installing the current CLI from a MirrorProxy site, run `sudo mirrorproxy set debian`
to switch recognized sources and refresh APT indexes. Use `sudo mirrorproxy set ubuntu`
for Ubuntu or `sudo mirrorproxy set system` for automatic detection. If no default instance
is configured, run `sudo mirrorproxy set debian https://mirror.example.com`.

### Legacy APT flags

APT detects `ID` and `VERSION_CODENAME` from `/etc/os-release` when
`--distribution` is omitted. Explicit `--distribution debian/bookworm` or
`ubuntu/noble` still works for the existing generated-file mode. Derivatives
require an explicit distribution; they are not guessed from their parent OS.

With the explicit `set apt --replace-existing --scope system` flags, the client
uses the original base-repository-only behavior. It rewrites recognized official base URIs in `sources.list` and
`sources.list.d/*.list` / `*.sources`, including DEB822 URI continuations.
It preserves suites (including updates/backports), components, architectures,
`Signed-By`, comments, disabled entries, and third-party repositories. It never
changes release suites or installs/upgrades packages; `--distribution` cannot be
combined with `--replace-existing`. Custom mirror hosts, Debian security,
Ubuntu security/ports and old-release URIs retain their existing addresses and
signature checks with these legacy flags.

`--dry-run` validates and previews actual file changes without writing files or
rollback state. Existing non-empty generated configurations still need `--force`
for preview and apply. Sensitive configuration lines are redacted in previews.
Replacement validates all files before writing, saves a multi-file rollback at
`/var/lib/mirrorproxy/sources/apt.json`, and preserves file permissions on Unix.
Writes are atomic per file, not across the entire set. If interrupted, run
`reset apt --scope system` to restore the saved files. Reset validates every
path and file before restoring anything; later user edits require `--force`.
Keep this client version available until reset: older clients cannot read the
new replacement rollback format. Existing single-file rollback records remain
compatible. Symlinked files and directories are refused.

Verify `apt-cache policy <package>` and the actual download URI, in addition to
`apt-get update`: an unchanged security or third-party repository may still win
package selection. After reset, refresh APT metadata again. For isolated
testing, `--config-root <directory>` reads both OS identification and APT files
under that root and stores rollback state there.

[简体中文](Client-zh-CN)

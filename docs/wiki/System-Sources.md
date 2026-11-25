# Linux system sources

The standalone client has native configuration adapters for the following 31
distribution profiles. The command and package-manager workflows follow
[chsrc](https://github.com/RubyMetric/chsrc) and the mirror catalog draws on
[LinuxMirrors](https://github.com/SuperManito/LinuxMirrors). MirrorProxy implements
the migrations in Rust.
`mirrorproxy systems` lists the same catalog used by the client and server.

These ranges describe **configuration compatibility**, including archived releases.
They do not extend the OS vendor's security support or guarantee that an upstream
still publishes usable metadata for every version and architecture. Fixture tests
cover all profiles; real package downloads must also be verified on Linux.

## Distribution and version matrix

| Client profile | Distribution | Configuration versions | Adapter |
| --- | --- | --- | --- |
| `debian` | Debian | 8–14, testing/sid | APT |
| `ubuntu` | Ubuntu | 14–26 | APT |
| `trisquel` | Trisquel | Existing APT feeds | APT |
| `linuxlite` | Linux Lite | Existing APT feeds | APT |
| `kali` | Kali Linux | Rolling | APT |
| `linuxmint` | Linux Mint / LMDE | Mint 19–22; LMDE 6–7 | APT |
| `deepin` | Deepin | Existing release layouts, including 25/beige | APT |
| `zorin` | Zorin OS | Existing Ubuntu-based layouts | APT |
| `armbian` | Armbian | Existing Debian/Ubuntu layouts | APT |
| `proxmox` | Proxmox VE | Existing Debian/PVE/Ceph layouts | APT |
| `raspios` | Raspberry Pi OS | Existing Debian/Raspbian layouts | APT |
| `rhel` | Red Hat Enterprise Linux | 7–10; compatible customer mirror required | RPM |
| `fedora` | Fedora | 30–44 | RPM |
| `centos` | CentOS Linux | 7–8 | RPM |
| `centos-stream` | CentOS Stream | 8–10 | RPM |
| `rocky` | Rocky Linux | 8–10 | RPM |
| `alma` | AlmaLinux | 8–10 | RPM |
| `oraclelinux` | Oracle Linux | 8–10 | RPM |
| `openeuler` | openEuler | 20–26 | RPM |
| `opencloudos` | OpenCloudOS | 8.6–9; Stream 23 | RPM |
| `openkylin` | openKylin | Existing release layouts | APT |
| `anolis` | Anolis OS | 8 / 23 | RPM |
| `opensuse` | openSUSE | Leap 15–16; Tumbleweed | RPM/Zypper |
| `archlinux` | Arch Linux / Arch Linux ARM | Rolling | pacman |
| `manjaro` | Manjaro | Stable/testing/unstable | pacman |
| `endeavouros` | EndeavourOS | Arch and EndeavourOS repository lists | pacman |
| `alpine` | Alpine Linux | v3 / edge | apk |
| `gentoo` | Gentoo | Existing Portage configurations | Distfile mirrors |
| `nixos` | NixOS | 19–26 | Nix module and existing root channels |
| `void` | Void Linux | Rolling, including existing musl paths | XBPS |
| `openwrt` | OpenWrt | Releases that use opkg | opkg |

## Switch system sources

The Linux installer from a MirrorProxy site remembers that instance for system
source changes. The server must have the `os` adapter enabled.

```bash
sudo mirrorproxy set debian     # Checks the named distribution
sudo mirrorproxy set system     # Detects the distribution automatically
```

Run either command. APT, DNF/YUM, zypper, pacman, apk, XBPS, and opkg refresh package indexes
after switching and restore the previous configuration if the refresh fails.
Gentoo's distfile mirror takes effect after writing. The NixOS module still requires an import
and administrator activation. On OpenWrt, run `mirrorproxy set openwrt` as root. If installed by a
package manager, run `sudo mirrorproxy set debian https://mirror.example.com`
directly, or save an instance with `sudo mirrorproxy use https://mirror.example.com`.
Use `sudo mirrorproxy reset system --scope system` to undo the migration.
The explicit configuration flags remain available.

Detection reads `os-release` without evaluating shell code. Armbian, Raspberry Pi
and Proxmox markers identify systems which otherwise report their parent OS.
OS identification symlinks must resolve within the selected root. The client
preserves existing release suites, branch names, `$releasever`, `$basearch`,
`$repo`, `$arch`, component lists, keyrings and signature settings. It does not
guess an x86 repository for an ARM machine or change OS releases.

APT handles both `.list` and DEB822 `.sources`, including URI continuations,
updates, security, ports and multiple base/vendor repositories. RPM migration
changes recognized base URLs and disables mirror discovery only in the sections
it can replace. Fedora's known base/updates/modular metalinks map to equivalent
repository trees. Unknown third-party sections remain intact. Pacman retains the
Manjaro branch and handles EndeavourOS's separate mirrorlist. Void uses `/etc`
overrides for vendor files under `/usr/share`, leaving vendor files untouched.

Only declared official repository roots and known mirror-provider layouts are
rewritten. Unknown custom mirrors remain unchanged. Missing or unrecognized
configurations are refused instead of inventing distribution configuration.
Dedicated pacman mirrorlists also recognize the native `$repo`/`$arch` URL
templates regardless of mirror hostname.
For repairs, restore the distribution's native repository definitions first.
Newly generated Gentoo settings, NixOS modules and Void overrides are exceptions
described below and are removed again by reset when originally absent.

## Archived and special repositories

- Debian Jessie/Stretch/Buster suites select separate base/security archive
  roots. Older Ubuntu suites select `ubuntu-old`. Existing suites and signature
  policy remain unchanged. Archived metadata can expire; the client never adds
  `trusted=yes`, disables signature/expiry checks, or installs replacement keys.
- CentOS 7/8 use Vault, pinning generic `$releasever` paths to 7.9.2009 / 8.5.2111.
  Fedora 30–42 select the archive tree; 43–44 retain the active tree. This placement
  is a catalog snapshot and should be reviewed as repositories move. Archived
  releases no longer receive normal vendor security updates.
- RHEL has no default public upstream. Configure a compatible, authorized mirror
  under `upstreams.additional_os.rhel` with the same `/content/...` hierarchy.
  The proxy does not forward client entitlement credentials or configure an
  upstream client certificate. A subscription-only Red Hat CDN cannot be used
  directly through this adapter. RHEL is never converted into CentOS.
- Proxmox enterprise/subscription sources remain unchanged. Public PVE/Ceph
  sources can be migrated; the client does not alter subscriptions, Perl modules,
  container templates or enterprise repository policy.
- Gentoo changes only `GENTOO_MIRRORS` in `make.conf`, retaining other settings,
  sync methods and trust controls. Run the normal sync/fetch commands afterward.
  Multiline shell assignments require manual review. Rsync/Git sync is not
  tunneled through the HTTP proxy.
- NixOS receives `/etc/nixos/mirrorproxy.nix`. Add `./mirrorproxy.nix` to your
  configuration's `imports`, then run `sudo nixos-rebuild switch`. Existing trusted
  public keys remain unchanged; the official cache stays available. Existing
  root `.nix-channels` HTTP URLs can also be migrated. The client does not edit
  flake inputs or evaluate Nix. To roll back, remove the import, run `reset system`,
  then rebuild; refresh channels if they were used.
- On openSUSE, a repository service such as RIS may regenerate `.repo` files.
  Manage that service with the native Zypper tools before migration if needed;
  the client does not uninstall service packages or edit their service policy.

## Safety, server upgrade and rollback

`--dry-run` validates actual payloads and displays changed lines without writing
files or rollback state. Sensitive lines are redacted. Changed files and their
original contents are recorded in `/var/lib/mirrorproxy/sources/system.json`
before any configuration is replaced. Paths are restricted to the selected
profile; symlinked configuration files/directories are refused. Unix permissions
are retained. Every restore path and file is checked before restoration starts;
post-migration user changes require an explicit `--force` to overwrite.

Writes are atomic per file, not as one filesystem transaction. After an
interruption, `reset system --scope system` can restore partially written files.
Keep this client version until reset; older clients cannot read the new
multi-file manifest. Refresh the native package index after applying or restoring
sources. No database migration or server deployment is performed by the client.

At startup and when saving runtime settings, the server fills in missing public
system repositories from the catalog, including Debian and Ubuntu security,
ports, distribution archives, and EPEL. Existing overrides take precedence.
Set an override in the administration console or use `upstreams.additional_os`
for a new instance. RHEL still requires an explicitly configured authorized
upstream. The APT, DNF, and pacman cards let you select a distribution and copy its in-place migration command.

For isolated fixtures, add `--config-root <directory>`; identification, native
configuration and rollback state are all read/written below that root. The native
default system scope is Linux; other platforms require this explicit test root.

## Supplemental repositories and Ubuntu flavours

Ubuntu Kylin, Kubuntu, Lubuntu, Xubuntu, Ubuntu MATE/Budgie/Studio/Unity and
Edubuntu IDs require an Ubuntu/Debian `ID_LIKE` declaration. Unknown derivatives
are not inferred from `ID_LIKE` alone. Mint before 19 and LMDE before 6 are no
longer advertised as supported; existing rollback records remain restorable.
The vendor `mint-info` package's `/etc/linuxmint/info` identifies environments
retaining their parent OS ID; the vendor `RELEASE` takes precedence over the
Ubuntu version.

Known mirror layouts include the LinuxMirrors mainland, education, overseas and
cloud-internal lists. Exact domain and repository-path boundaries are required;
Aliyun uses `mirrors.aliyun.com`, not its developer homepage. Recognition does
not assert that each mirror carries every repository. Server settings determine
the final upstream.

```bash
sudo mirrorproxy set epel
sudo mirrorproxy reset epel --scope system
sudo mirrorproxy set docker-ce
sudo mirrorproxy reset docker-ce --scope system
sudo mirrorproxy set gentoo-sync
sudo mirrorproxy reset gentoo-sync --scope system
```

All three support `--dry-run` and share the protected system rollback slot. Reset
the previous operation before starting another. EPEL migrates existing EPEL,
Next and Archive repositories; bootstrap `epel-release` through the vendor's
official procedure first. EPEL 7 uses Archive without bypassing trust or expiry.
EPEL Next uses upstream `pub/epel/next`; version availability follows upstream.
AlmaLinux Vault uses `/os/alma-vault/` and retains its pinned minor version.

Docker CE independently migrates existing APT/RPM installation sources to
`/os/docker-ce/linux/<distribution>/...`. For a first install, provision Docker's
official key first (APT uses `/etc/apt/keyrings/docker.asc`), then `set docker-ce`
generates a signature-required source. Existing sources are migrated. Use the native package manager to
install `docker-ce`, `docker-ce-cli` and `containerd.io`. The client does not
install/restart daemons. Docker Hub acceleration remains `set docker`.

`set gentoo-sync` configures Portage's HTTP `webrsync` snapshots and enables
snapshot signature verification, retaining the existing repository location and
OpenPGP key settings. Run `emerge --sync` afterward. It requires an existing
official repository definition and valid key. A single-file `repos.conf` or
multiple Gentoo definitions require manual organization first. Ordinary
`set system` continues to change only `GENTOO_MIRRORS`; rsync is not tunneled.

## Verify on Linux

After migrating, verify the effective configuration and a real package download:

| Adapter | Example verification |
| --- | --- |
| APT | `apt-get update`; `apt-cache policy hello`; `apt-get --print-uris download hello`; `apt-get download hello` |
| RPM | `dnf repolist -v`; `dnf --refresh makecache`; download a package with DNF |
| Zypper | `zypper lr -u`; `zypper refresh`; download a package |
| pacman | `pacman -Syyw --noconfirm <package>` in a disposable environment |
| apk | `apk update`; `apk fetch busybox` |
| XBPS | `xbps-query -L`; `xbps-install -S`; fetch/install a small package in a container |
| Gentoo | Normal sync followed by `emerge --fetchonly <package>` |
| NixOS | Import/rebuild, inspect substituters, fetch a signed store path |

Check the selected URI and the ecosystem's signature/digest validation, not only
an HTTP status or generated text. A preserved third-party repository may still
provide a preferred candidate. Compare original file checksums after reset.

The disposable container smoke is run by the operator on Linux:

```bash
cargo build --locked --release -p mirrorproxy-cli --target x86_64-unknown-linux-musl
MIRRORPROXY_OS_SMOKE_BASE=https://mirror.example.com \
MIRRORPROXY_OS_SMOKE_CLIENT_BINARY="$PWD/target/x86_64-unknown-linux-musl/release/mirrorproxy" \
MIRRORPROXY_OS_SMOKE_TARGETS="debian ubuntu fedora archlinux alpine opensuse void gentoo" \
bash scripts/smoke-os-clients.sh
```

This exercises representative package-manager families, not every version in
the matrix. Additional OS/version images need their own live validation before
production use. Container execution and package downloads require network access.
The musl build requires the Rust target and musl toolchain; select the matching
architecture on ARM hosts. Optional smoke targets include `rocky`, `alma` and
`oraclelinux`. Override a family image with variables such as
`MIRRORPROXY_OS_SMOKE_DEBIAN_IMAGE=debian:trixie-slim` or
`MIRRORPROXY_OS_SMOKE_FEDORA_IMAGE=fedora:43` to test another version.

`scripts/smoke-os-matrix.sh` runs all 31 profiles independently, writing per-target
logs and `results.tsv`. Missing authentic images are `BLOCKED`, never substituted
with a parent distribution. Supply `MIRRORPROXY_OS_SMOKE_<UPPERCASE_CODE>_IMAGE`
for Mint, Zorin, Armbian, Proxmox, Raspberry Pi OS, RHEL, EndeavourOS and NixOS;
CentOS Stream uses `MIRRORPROXY_OS_SMOKE_CENTOS_STREAM_IMAGE`. RHEL additionally
requires an entitled customer upstream. `nixos/nix` alone is not a NixOS image.
Unavailable upstreams, expired signatures and failed pulls remain non-passing.

`scripts/smoke-system-extras.sh` checks EPEL RPM signatures, Docker CE APT
downloads and Gentoo signed snapshot synchronization. Use the static musl client;
set `MIRRORPROXY_OS_SMOKE_REGISTRY=sina.dev` for image pulls and
`MIRRORPROXY_OS_SMOKE_BASE` for the candidate server.

Mirror-recognition data references LinuxMirrors `76fbf173`; see its
[MIT notice](https://github.com/inbjo/MirrorProxy/blob/main/docs/licenses/LinuxMirrors-MIT.txt).
See the [2026-10-22 acceptance record](https://github.com/inbjo/MirrorProxy/blob/main/docs/testing/Linux-Source-Acceptance-2026-10-22.md)
for per-profile results and remaining environment requirements.

[简体中文](System-Sources-zh-CN)

#!/usr/bin/env bash
set -Eeuo pipefail

# Runs destructive package-manager checks only inside disposable OS containers.
# Images, the installer, release assets, repository metadata, and packages all
# flow through the selected MirrorProxy deployment.

base="${MIRRORPROXY_OS_SMOKE_BASE:-https://sina.dev}"
base="${base%/}"
registry="${MIRRORPROXY_OS_SMOKE_REGISTRY:-${base#*://}}"
targets="${MIRRORPROXY_OS_SMOKE_TARGETS:-debian ubuntu fedora archlinux alpine opensuse void gentoo}"
candidate="${MIRRORPROXY_OS_SMOKE_CLIENT_BINARY:-}"

command -v docker >/dev/null 2>&1 || { echo "docker is required" >&2; exit 2; }
[[ "${base}" =~ ^https?://[^/]+$ ]] || {
  echo "MIRRORPROXY_OS_SMOKE_BASE must be an origin URL without a path" >&2
  exit 2
}
if [[ -n "${candidate}" ]]; then
  candidate="$(realpath "${candidate}")"
  [[ -x "${candidate}" ]] || { echo "candidate client is not executable: ${candidate}" >&2; exit 2; }
fi

# Expanded intentionally by the shell running inside each container.
# shellcheck disable=SC2016
install_client='curl -fsSL "${MIRROR_BASE}/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh" | sh -s -- --mirror "${MIRROR_BASE}"
mirrorproxy --version
if [ -x /opt/mirrorproxy-candidate ]; then
  install -m 0755 /opt/mirrorproxy-candidate /usr/local/bin/mirrorproxy
fi'

run_case() {
  local target=$1
  local image=$2
  local body=$3
  local -a args=(run --rm -e "MIRROR_BASE=${base}")
  if [[ -n "${candidate}" ]]; then
    args+=(-v "${candidate}:/opt/mirrorproxy-candidate:ro")
  fi
  printf '\n==> %s (%s)\n' "${target}" "${registry}/${image}"
  docker "${args[@]}" "${registry}/${image}" sh -euxc "${body}"
  printf 'PASS %s\n' "${target}"
}

for target in ${targets}; do
  case "${target}" in
    debian)
      run_case debian debian:bookworm-slim "apt-get update
apt-get install -y --no-install-recommends ca-certificates curl
${install_client}
rm -f /etc/apt/sources.list /etc/apt/sources.list.d/debian.sources
mirrorproxy set apt --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system --distribution debian/bookworm
rm -rf /var/lib/apt/lists/*
apt-get update
cd /tmp
apt-get download hello
test -s hello_*.deb
mirrorproxy reset apt --scope system
test ! -e /etc/apt/sources.list.d/mirrorproxy.list"
      ;;
    ubuntu)
      run_case ubuntu ubuntu:24.04 "apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl
${install_client}
rm -f /etc/apt/sources.list /etc/apt/sources.list.d/ubuntu.sources
mirrorproxy set apt --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system --distribution ubuntu/noble
rm -rf /var/lib/apt/lists/*
apt-get update
cd /tmp
apt-get download hello
test -s hello_*.deb
mirrorproxy reset apt --scope system
test ! -e /etc/apt/sources.list.d/mirrorproxy.list"
      ;;
    fedora)
      run_case fedora fedora:42 "${install_client}
rm -f /etc/yum.repos.d/*.repo
mirrorproxy set dnf --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system
dnf --refresh makecache
dnf download --destdir=/tmp setup
test -s /tmp/setup-*.rpm
mirrorproxy reset dnf --scope system
test ! -e /etc/yum.repos.d/mirrorproxy.repo"
      ;;
    archlinux)
      run_case archlinux archlinux:latest "${install_client}
sha256sum /etc/pacman.d/mirrorlist >/tmp/mirrorlist.before
mirrorproxy set pacman --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system --force
grep -F \"Server = \${MIRROR_BASE}/os/archlinux/\" /etc/pacman.d/mirrorlist
pacman -Syyw --noconfirm --cachedir /tmp filesystem
test -s /tmp/filesystem-*.pkg.tar.zst
mirrorproxy reset pacman --scope system --force
sha256sum -c /tmp/mirrorlist.before"
      ;;
    alpine)
      run_case alpine alpine:3.21 "apk add --no-cache ca-certificates curl
${install_client}
: >/etc/apk/repositories
mirrorproxy set alpine --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system --distribution v3.21 --force
apk update
apk fetch --output /tmp busybox
test -s /tmp/busybox-*.apk
mirrorproxy reset alpine --scope system --force"
      ;;
    opensuse)
      run_case opensuse opensuse/leap:15.6 "zypper --non-interactive install --no-recommends tar gzip
${install_client}
rm -f /etc/zypp/repos.d/*.repo
mirrorproxy set zypper --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system --distribution distribution/leap/15.6
zypper --non-interactive refresh
zypper --non-interactive install --download-only --no-recommends tree
find /var/cache/zypp/packages -type f -name 'tree-*.rpm' -print -quit | grep .
mirrorproxy reset zypper --scope system
test ! -e /etc/zypp/repos.d/mirrorproxy.repo"
      ;;
    void)
      run_case void voidlinux/voidlinux:latest "mv /usr/share/xbps.d /usr/share/xbps.d.bootstrap
mkdir -p /usr/share/xbps.d
xbps-install -Suy -R \"\${MIRROR_BASE}/os/void/current\" xbps
xbps-install -y -R \"\${MIRROR_BASE}/os/void/current\" ca-certificates curl
${install_client}
rm -rf /usr/share/xbps.d
mkdir -p /usr/share/xbps.d
mirrorproxy set xbps --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system
xbps-install -S
xbps-install -y tree
command -v tree
mirrorproxy reset xbps --scope system
test ! -e /etc/xbps.d/00-mirrorproxy.conf"
      ;;
    gentoo)
      run_case gentoo gentoo/stage3:latest "${install_client}
emerge-webrsync
mirrorproxy set gentoo --mirror mirrorproxy --base-url \"\${MIRROR_BASE}\" --scope system --force
emerge --fetchonly --quiet-build app-misc/hello
test -s /var/cache/distfiles/hello-*.tar.gz
mirrorproxy reset gentoo --scope system --force"
      ;;
    *)
      echo "unknown OS smoke target: ${target}" >&2
      exit 2
      ;;
  esac
done

printf '\nOS native-client smoke passed: %s\n' "${targets}"

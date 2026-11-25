#!/usr/bin/env bash
set -Eeuo pipefail

# Runs destructive package-manager checks only inside disposable OS containers.
# Migrate original distribution configuration and verify package payloads through
# the selected deployment. Initial bootstrap installs may use official upstreams.

base="${MIRRORPROXY_OS_SMOKE_BASE:-https://sina.dev}"
base="${base%/}"
registry="${MIRRORPROXY_OS_SMOKE_REGISTRY:-${base#*://}}"
targets="${MIRRORPROXY_OS_SMOKE_TARGETS:-debian ubuntu fedora archlinux alpine opensuse void gentoo}"
candidate="${MIRRORPROXY_OS_SMOKE_CLIENT_BINARY:-}"
container_dir="$(mktemp -d /tmp/mirrorproxy-os-container.XXXXXX)"
cleanup_container() {
  local container_id=""
  if [[ -s "${container_dir}/container.id" ]]; then
    read -r container_id <"${container_dir}/container.id" || true
    if [[ "${container_id}" =~ ^[a-f0-9]{64}$ ]]; then
      docker rm -f "${container_id}" >/dev/null 2>&1 || true
    fi
    rm -f "${container_dir}/container.id"
  fi
  rmdir "${container_dir}" 2>/dev/null || true
}
trap cleanup_container EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

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
install_client='if [ -x /opt/mirrorproxy-candidate ]; then
  install -m 0755 /opt/mirrorproxy-candidate /usr/local/bin/mirrorproxy
else
  curl -fsSL "${MIRROR_BASE}/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh" | sh -s -- --mirror "${MIRROR_BASE}"
fi
mirrorproxy --version
mirrorproxy systems'

run_case() {
  local target=$1
  local image=$2
  local body=$3
  local -a args=(run --rm --init --cidfile "${container_dir}/container.id" --user 0 -e "MIRROR_BASE=${base}")
  if [[ -n "${candidate}" ]]; then
    args+=(-v "${candidate}:/opt/mirrorproxy-candidate:ro")
  fi
  printf '\n==> %s (%s)\n' "${target}" "${registry}/${image}"
  docker "${args[@]}" "${registry}/${image}" sh -euxc "${body}"
  rm -f "${container_dir}/container.id"
  printf 'PASS %s\n' "${target}"
}

for target in ${targets}; do
  case "${target}" in
    rhel|nixos|endeavouros)
      image_variable="MIRRORPROXY_OS_SMOKE_${target^^}_IMAGE"
      [[ -n "${!image_variable:-}" ]] || { echo "${target} requires ${image_variable} with an authentic system image" >&2; exit 2; }
      ;;
  esac
  case "${target}" in
    kali|trisquel|linuxlite|linuxmint|deepin|zorin|armbian|proxmox|raspios|openkylin)
      image_variable="MIRRORPROXY_OS_SMOKE_${target^^}_IMAGE"
      case "${target}" in
        kali) default_image=kalilinux/kali-rolling:latest ;;
        linuxmint) default_image=linuxmintd/mint22-amd64:latest ;;
        deepin) default_image=linuxdeepin/deepin:beige ;;
        openkylin) default_image=openkylin/openkylin:2.0 ;;
        *) default_image="" ;;
      esac
      apt_image="${!image_variable:-${default_image}}"
      [[ -n "${apt_image}" ]] || { echo "${target} requires ${image_variable} with an authentic distribution image" >&2; exit 2; }
      bootstrap=""
      if [[ "${target}" == linuxmint ]]; then
        bootstrap='apt-get install -y mint-info-cinnamon'
      fi
      run_case "${target}" "${apt_image}" "${bootstrap}
${install_client}
find /etc/apt -type f -exec sha256sum {} \\; > /tmp/apt.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing --dry-run
sha256sum -c /tmp/apt.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
rm -rf /var/lib/apt/lists/*
apt-get update -o APT::Update::Error-Mode=any
cd /tmp
apt-get --print-uris download bash | grep -F \"\${MIRROR_BASE}/os/\"
apt-get download bash
test -s bash_*.deb
mirrorproxy reset system --scope system
sha256sum -c /tmp/apt.before"
      ;;
    debian)
      run_case debian "${MIRRORPROXY_OS_SMOKE_DEBIAN_IMAGE:-debian:bookworm-slim}" "apt-get update
apt-get install -y --no-install-recommends ca-certificates curl
${install_client}
find /etc/apt -type f -exec sha256sum {} \\; > /tmp/apt.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing --dry-run
sha256sum -c /tmp/apt.before
test ! -e /var/lib/mirrorproxy/sources/system.json
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
rm -rf /var/lib/apt/lists/*
apt-get update
cd /tmp
apt-get --print-uris download hello | grep -F \"\${MIRROR_BASE}/os/debian/\"
apt-get download hello
test -s hello_*.deb
mirrorproxy reset system --scope system
sha256sum -c /tmp/apt.before
test ! -e /var/lib/mirrorproxy/sources/system.json"
      ;;
    ubuntu)
      run_case ubuntu "${MIRRORPROXY_OS_SMOKE_UBUNTU_IMAGE:-ubuntu:24.04}" "apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl
${install_client}
find /etc/apt -type f -exec sha256sum {} \\; > /tmp/apt.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing --dry-run
sha256sum -c /tmp/apt.before
test ! -e /var/lib/mirrorproxy/sources/system.json
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
rm -rf /var/lib/apt/lists/*
apt-get update
cd /tmp
apt-get --print-uris download hello | grep -F \"\${MIRROR_BASE}/os/ubuntu/\"
apt-get download hello
test -s hello_*.deb
mirrorproxy reset system --scope system
sha256sum -c /tmp/apt.before
test ! -e /var/lib/mirrorproxy/sources/system.json"
      ;;
    centos)
      run_case centos "${MIRRORPROXY_OS_SMOKE_CENTOS_IMAGE:-centos:7}" "${install_client}
sha256sum /etc/yum.repos.d/*.repo > /tmp/repos.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
yum install -y yum-utils
yum makecache
yumdownloader --urls setup | grep -F \"\${MIRROR_BASE}/os/centos-vault/\"
yumdownloader --destdir=/tmp setup
rpm --import /etc/pki/rpm-gpg/RPM-GPG-KEY-CentOS-7
rpmkeys --checksig /tmp/setup-*.rpm
mirrorproxy reset system --scope system
sha256sum -c /tmp/repos.before"
      ;;
    fedora|rocky|alma|oraclelinux|centos-stream|openeuler|opencloudos|anolis|rhel)
      case "${target}" in
        fedora) rpm_image="${MIRRORPROXY_OS_SMOKE_FEDORA_IMAGE:-fedora:44}" ;;
        rocky) rpm_image="${MIRRORPROXY_OS_SMOKE_ROCKY_IMAGE:-rockylinux:9}" ;;
        alma) rpm_image="${MIRRORPROXY_OS_SMOKE_ALMA_IMAGE:-almalinux:9}" ;;
        oraclelinux) rpm_image="${MIRRORPROXY_OS_SMOKE_ORACLELINUX_IMAGE:-oraclelinux:9}" ;;
        centos-stream) rpm_image="${MIRRORPROXY_OS_SMOKE_CENTOS_STREAM_IMAGE:-quay.io/centos/centos:stream9}" ;;
        openeuler) rpm_image="${MIRRORPROXY_OS_SMOKE_OPENEULER_IMAGE:-openeuler/openeuler:24.03-lts}" ;;
        opencloudos) rpm_image="${MIRRORPROXY_OS_SMOKE_OPENCLOUDOS_IMAGE:-opencloudos/opencloudos:9.0}" ;;
        anolis) rpm_image="${MIRRORPROXY_OS_SMOKE_ANOLIS_IMAGE:-openanolis/anolisos:8.8}" ;;
        rhel) rpm_image="${MIRRORPROXY_OS_SMOKE_RHEL_IMAGE:?RHEL requires an entitled distribution image and customer upstream}" ;;
      esac
      run_case "${target}" "${rpm_image}" "if [ ! -x /opt/mirrorproxy-candidate ]; then
  dnf install -y ca-certificates
  command -v curl >/dev/null 2>&1 || dnf install -y curl
fi
${install_client}
sha256sum /etc/yum.repos.d/*.repo > /tmp/repos.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
if ! dnf download --help >/dev/null 2>&1; then dnf install -y dnf-plugins-core; fi
dnf --refresh makecache
dnf download --url setup | grep -F \"\${MIRROR_BASE}/os/\"
dnf download --destdir=/tmp setup
test -s /tmp/setup-*.rpm
if [ \"${target}\" = centos-stream ]; then
  rpm --import /etc/pki/rpm-gpg/RPM-GPG-KEY-centosofficial
else
  rpm --import /etc/pki/rpm-gpg/RPM-GPG-KEY-*
fi
rpmkeys --checksig /tmp/setup-*.rpm
mirrorproxy reset system --scope system
sha256sum -c /tmp/repos.before"
      ;;
    archlinux|manjaro|endeavouros)
      case "${target}" in
        archlinux) pacman_image="${MIRRORPROXY_OS_SMOKE_ARCHLINUX_IMAGE:-archlinux:latest}" ;;
        manjaro) pacman_image="${MIRRORPROXY_OS_SMOKE_MANJARO_IMAGE:-manjarolinux/base:latest}" ;;
        endeavouros) pacman_image="${MIRRORPROXY_OS_SMOKE_ENDEAVOUROS_IMAGE:?EndeavourOS requires an authentic distribution image}" ;;
      esac
      run_case "${target}" "${pacman_image}" "${install_client}
sha256sum /etc/pacman.d/mirrorlist >/tmp/mirrorlist.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
grep -F \"\${MIRROR_BASE}/os/\" /etc/pacman.d/mirrorlist
pacman -Syyw --noconfirm --cachedir /tmp filesystem
test -s /tmp/filesystem-*.pkg.tar.zst
mirrorproxy reset system --scope system
sha256sum -c /tmp/mirrorlist.before"
      ;;
    alpine)
      run_case alpine "${MIRRORPROXY_OS_SMOKE_ALPINE_IMAGE:-alpine:3.21}" "apk add --no-cache ca-certificates curl
${install_client}
sha256sum /etc/apk/repositories > /tmp/repositories.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
apk update
apk fetch --output /tmp busybox
test -s /tmp/busybox-*.apk
apk verify /tmp/busybox-*.apk
mirrorproxy reset system --scope system
sha256sum -c /tmp/repositories.before"
      ;;
    opensuse)
      run_case opensuse "${MIRRORPROXY_OS_SMOKE_OPENSUSE_IMAGE:-opensuse/leap:15.6}" "zypper --non-interactive install --no-recommends tar gzip
${install_client}
find /etc/zypp/repos.d -type f -exec sha256sum {} \\; > /tmp/repos.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
zypper --non-interactive refresh
zypper --non-interactive install --download-only --no-recommends tree
find /var/cache/zypp/packages -type f -name 'tree-*.rpm' -print -quit | grep .
mirrorproxy reset system --scope system
sha256sum -c /tmp/repos.before"
      ;;
    void)
      run_case void "${MIRRORPROXY_OS_SMOKE_VOID_IMAGE:-voidlinux/voidlinux:latest}" "if [ ! -x /opt/mirrorproxy-candidate ]; then
  xbps-install -Suy xbps
  xbps-install -y ca-certificates curl
fi
${install_client}
find /etc/xbps.d -type f -exec sha256sum {} \\; > /tmp/repos.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
xbps-install -Suy xbps
xbps-install -y tree
command -v tree
mirrorproxy reset system --scope system
test ! -s /tmp/repos.before || sha256sum -c /tmp/repos.before
test ! -e /etc/xbps.d/00-repository-main.conf"
      ;;
    gentoo)
      run_case gentoo "${MIRRORPROXY_OS_SMOKE_GENTOO_IMAGE:-gentoo/stage3:latest}" "${install_client}
sha256sum /etc/portage/make.conf > /tmp/make-conf.before
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
grep -F \"GENTOO_MIRRORS=\\\"\${MIRROR_BASE}/os/gentoo\\\"\" /etc/portage/make.conf
wget -q \"\${MIRROR_BASE}/os/gentoo/distfiles/32/hello-2.12.3.tar.gz\" -O /tmp/hello-2.12.3.tar.gz
echo \"0d5f60154382fee10b114a1c34e785d8b1f492073ae2d3a6f7b147687b366aa0  /tmp/hello-2.12.3.tar.gz\" | sha256sum -c -
mirrorproxy reset system --scope system
sha256sum -c /tmp/make-conf.before"
      ;;
    nixos)
      run_case nixos "${MIRRORPROXY_OS_SMOKE_NIXOS_IMAGE:?NixOS requires a NixOS image; nixos/nix alone is not NixOS}" "${install_client}
mirrorproxy set ${target} --base-url \"\${MIRROR_BASE}\" --scope system --replace-existing
nix-instantiate --eval --strict -E 'let c = import /etc/nixos/mirrorproxy.nix { options.nix.settings = {}; }; in c.config.nix.settings.substituters'
store_path=\$(nix-instantiate --eval --strict -E '(import <nixpkgs> {}).hello.outPath' | tr -d '\"')
nix-store --option substituters \"\${MIRROR_BASE}/os/nix-store/\" --option fallback false --realise \"\${store_path}\"
mirrorproxy reset system --scope system
test ! -e /etc/nixos/mirrorproxy.nix"
      ;;
    openwrt)
      openwrt_image="${MIRRORPROXY_OS_SMOKE_OPENWRT_IMAGE:-}"
      [[ -n "${openwrt_image}" ]] || { echo "openwrt requires an authentic opkg OpenWrt image"; exit 2; }
      run_case openwrt "${openwrt_image}" "${install_client}
sha256sum /etc/opkg/distfeeds.conf > /tmp/feeds.before
mirrorproxy set openwrt "\${MIRROR_BASE}"
grep -F "\${MIRROR_BASE}/os/openwrt/" /etc/opkg/distfeeds.conf
opkg list | grep -E '^libc - '
mirrorproxy reset openwrt --scope system
sha256sum -c /tmp/feeds.before"
      ;;
    *)
      echo "unknown OS smoke target: ${target}" >&2
      exit 2
      ;;
  esac
done

printf '\nOS native-client smoke passed: %s\n' "${targets}"

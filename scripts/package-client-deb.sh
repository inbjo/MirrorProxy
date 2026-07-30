#!/usr/bin/env bash
set -euo pipefail

binary="${1:?usage: package-client-deb.sh BINARY VERSION ARCH OUTPUT_DIR}"
version="${2:?version required}"
arch="${3:?architecture required}"
output_dir="${4:?output directory required}"

case "$arch" in
  x86_64 | amd64) deb_arch="amd64" ;;
  aarch64 | arm64) deb_arch="arm64" ;;
  *) echo "unsupported Debian architecture: $arch" >&2; exit 2 ;;
esac

package_root="$(mktemp -d)"
cleanup() {
  if [[ -n "$package_root" && "$package_root" == /tmp/* ]]; then
    rm -rf -- "$package_root"
  fi
}
trap cleanup EXIT

install -Dm755 "$binary" "$package_root/usr/bin/mirrorproxy"
mkdir -p "$package_root/DEBIAN" "$output_dir"

installed_size="$(du -sk "$package_root/usr" | awk '{ print $1 }')"
cat >"$package_root/DEBIAN/control" <<EOF
Package: mirrorproxy
Version: $version
Section: utils
Priority: optional
Architecture: $deb_arch
Installed-Size: $installed_size
Maintainer: MirrorProxy contributors <noreply@github.com>
Homepage: https://github.com/inbjo/MirrorProxy
Description: Standalone MirrorProxy source manager
 Configure package managers and development tools to use MirrorProxy or
 supported public mirrors, with safe preview, write, and rollback operations.
EOF

dpkg-deb --build --root-owner-group \
  "$package_root" "$output_dir/mirrorproxy_${version}_${deb_arch}.deb"

#!/usr/bin/env bash
set -euo pipefail

binary="${1:?usage: package-client-rpm.sh BINARY VERSION ARCH OUTPUT_DIR}"
version="${2:?version required}"
arch="${3:?architecture required}"
output_dir="${4:?output directory required}"

case "$arch" in
  x86_64 | amd64) rpm_arch="x86_64" ;;
  aarch64 | arm64) rpm_arch="aarch64" ;;
  *) echo "unsupported RPM architecture: $arch" >&2; exit 2 ;;
esac

package_root="$(mktemp -d)"
cleanup() {
  if [[ -n "$package_root" && "$package_root" == /tmp/* ]]; then
    rm -rf -- "$package_root"
  fi
}
trap cleanup EXIT

mkdir -p "$output_dir" "$package_root"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS,rpmdb,tmp}
install -m755 "$binary" "$package_root/SOURCES/mirrorproxy"
cat >"$package_root/SPECS/mirrorproxy.spec" <<EOF
Name: mirrorproxy
Version: $version
Release: 1%{?dist}
Summary: Standalone MirrorProxy source manager
License: MIT
URL: https://github.com/inbjo/MirrorProxy
Source0: mirrorproxy
%define debug_package %{nil}
%global __strip /bin/true
%description
Configure package managers and development tools to use MirrorProxy or
supported public mirrors, with safe preview, write, and rollback operations.
%prep
%build
%install
install -Dm755 %{SOURCE0} %{buildroot}/usr/bin/mirrorproxy
%files
%attr(0755,root,root) /usr/bin/mirrorproxy
EOF
rpmbuild --define "_topdir $package_root" --define "_dbpath $package_root/rpmdb" \
  --define "_tmppath $package_root/tmp" --target "${rpm_arch}-linux" -bb "$package_root/SPECS/mirrorproxy.spec"
find "$package_root/RPMS" -name '*.rpm' -exec cp {} "$output_dir/" \;

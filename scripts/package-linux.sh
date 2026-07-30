#!/usr/bin/env bash
set -euo pipefail

binary="${1:?usage: package-linux.sh BINARY VERSION ARCH OUTPUT_DIR}"
version="${2:?version required}"
arch="${3:?architecture required}"
output_dir="${4:?output directory required}"
mkdir -p "$output_dir"

package_root="$(mktemp -d)"
cleanup() {
  if [[ -n "$package_root" && "$package_root" == /tmp/* ]]; then
    rm -rf -- "$package_root"
  fi
}
trap cleanup EXIT

deb_arch="$arch"
rpm_arch="$arch"
if [[ "$arch" == "x86_64" ]]; then deb_arch="amd64"; fi
if [[ "$arch" == "aarch64" ]]; then deb_arch="arm64"; fi

install -Dm755 "$binary" "$package_root/deb/usr/bin/mirrorproxy-server"
install -Dm644 config.example.toml "$package_root/deb/etc/mirrorproxy/config.toml"
mkdir -p "$package_root/deb/DEBIAN"
cat >"$package_root/deb/DEBIAN/control" <<EOF
Package: mirrorproxy-server
Version: $version
Section: net
Priority: optional
Architecture: $deb_arch
Maintainer: MirrorProxy contributors
Description: Self-hosted package mirror proxy and administration console
EOF
dpkg-deb --build --root-owner-group "$package_root/deb" "$output_dir/mirrorproxy-server_${version}_${deb_arch}.deb"

mkdir -p "$package_root/rpmroot/usr/bin" "$package_root/rpmroot/etc/mirrorproxy" "$package_root/rpmbuild"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
install -m755 "$binary" "$package_root/rpmroot/usr/bin/mirrorproxy-server"
install -m644 config.example.toml "$package_root/rpmroot/etc/mirrorproxy/config.toml"
tar -C "$package_root/rpmroot" -czf "$package_root/rpmbuild/SOURCES/mirrorproxy-server.tar.gz" .
cat >"$package_root/rpmbuild/SPECS/mirrorproxy-server.spec" <<EOF
Name: mirrorproxy-server
Version: $version
Release: 1%{?dist}
Summary: Self-hosted package mirror proxy
License: MIT
BuildArch: $rpm_arch
Source0: mirrorproxy-server.tar.gz
%description
MirrorProxy server and embedded administration console.
%prep
%setup -q -c -T
tar -xzf %{SOURCE0}
%install
mkdir -p %{buildroot}
cp -a . %{buildroot}/
%files
%attr(0755,root,root) /usr/bin/mirrorproxy-server
%config(noreplace) /etc/mirrorproxy/config.toml
EOF
rpmbuild --define "_topdir $package_root/rpmbuild" -bb "$package_root/rpmbuild/SPECS/mirrorproxy-server.spec"
find "$package_root/rpmbuild/RPMS" -name '*.rpm' -exec cp {} "$output_dir/" \;

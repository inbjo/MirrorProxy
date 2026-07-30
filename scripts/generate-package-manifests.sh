#!/usr/bin/env bash
set -euo pipefail

version="${1:?version required}"
artifacts="${2:?artifact directory required}"
output="${3:?output directory required}"
repository="${GITHUB_REPOSITORY:-inbjo/MirrorProxy}"
base_url="https://github.com/${repository}/releases/download/v${version}"
mkdir -p "$output"

checksum() {
  local filename="$1"
  local checksum_file
  checksum_file="$(find "$artifacts" -name "${filename}.sha256" -print -quit)"
  if [[ -z "$checksum_file" ]]; then echo "missing checksum for $filename" >&2; exit 1; fi
  awk 'NR == 1 { print $1 }' "$checksum_file"
}

linux_x64="mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz"
mac_x64="mirrorproxy-client-x86_64-apple-darwin.tar.gz"
mac_arm64="mirrorproxy-client-aarch64-apple-darwin.tar.gz"
windows_x64="mirrorproxy-client-x86_64-pc-windows-msvc.zip"

cat >"$output/mirrorproxy.rb" <<EOF
class Mirrorproxy < Formula
  desc "Standalone source manager for MirrorProxy"
  homepage "https://github.com/${repository}"
  version "${version}"
  license "MIT"
  on_macos do
    if Hardware::CPU.arm?
      url "${base_url}/${mac_arm64}"
      sha256 "$(checksum "$mac_arm64")"
    else
      url "${base_url}/${mac_x64}"
      sha256 "$(checksum "$mac_x64")"
    end
  end
  on_linux do
    url "${base_url}/${linux_x64}"
    sha256 "$(checksum "$linux_x64")"
  end
  def install
    bin.install "mirrorproxy"
  end
  test do
    system "#{bin}/mirrorproxy", "--version"
  end
end
EOF

cat >"$output/mirrorproxy.json" <<EOF
{
  "version": "${version}",
  "description": "Standalone source manager for MirrorProxy",
  "homepage": "https://github.com/${repository}",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "${base_url}/${windows_x64}",
      "hash": "$(checksum "$windows_x64")"
    }
  },
  "bin": "mirrorproxy.exe",
  "checkver": { "github": "https://github.com/${repository}" },
  "autoupdate": { "architecture": { "64bit": { "url": "https://github.com/${repository}/releases/download/v\$version/${windows_x64}" } } }
}
EOF

winget_dir="$output/winget/manifests/i/Inbjo/MirrorProxy/$version"
mkdir -p "$winget_dir"
cat >"$winget_dir/Inbjo.MirrorProxy.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.1.10.0.schema.json
PackageIdentifier: Inbjo.MirrorProxy
PackageVersion: ${version}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.10.0
EOF

cat >"$winget_dir/Inbjo.MirrorProxy.installer.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.10.0.schema.json
PackageIdentifier: Inbjo.MirrorProxy
PackageVersion: ${version}
InstallerType: zip
NestedInstallerType: portable
Commands:
- mirrorproxy
Installers:
- Architecture: x64
  NestedInstallerFiles:
  - RelativeFilePath: mirrorproxy.exe
    PortableCommandAlias: mirrorproxy
  InstallerUrl: ${base_url}/${windows_x64}
  InstallerSha256: $(checksum "$windows_x64" | tr '[:lower:]' '[:upper:]')
ManifestType: installer
ManifestVersion: 1.10.0
EOF

cat >"$winget_dir/Inbjo.MirrorProxy.locale.en-US.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.1.10.0.schema.json
PackageIdentifier: Inbjo.MirrorProxy
PackageVersion: ${version}
PackageLocale: en-US
Publisher: MirrorProxy contributors
PublisherUrl: https://github.com/${repository}
PublisherSupportUrl: https://github.com/${repository}/issues
PackageName: MirrorProxy
PackageUrl: https://github.com/${repository}
License: MIT
LicenseUrl: https://github.com/${repository}/blob/main/LICENSE
ShortDescription: Standalone source manager for MirrorProxy
Description: Configure package managers and development tools to use MirrorProxy or supported public mirrors, with safe preview, write, and rollback operations.
Moniker: mirrorproxy
Tags:
- cli
- mirror
- package-manager
- source-manager
ManifestType: defaultLocale
ManifestVersion: 1.10.0
EOF

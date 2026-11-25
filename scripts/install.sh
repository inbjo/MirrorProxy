#!/usr/bin/env sh
set -eu

repo="${MIRRORPROXY_GITHUB_REPO:-inbjo/MirrorProxy}"
version="${MIRRORPROXY_VERSION:-latest}"
install_dir="${MIRRORPROXY_INSTALL_DIR:-/usr/local/bin}"
mirror="${MIRRORPROXY_DOWNLOAD_MIRROR:-}"
setup=false
setup_base_url=""
install_dir_explicit=false
[ -z "${MIRRORPROXY_INSTALL_DIR:-}" ] || install_dir_explicit=true

usage() {
  cat <<'EOF'
Install the latest stable MirrorProxy client release.

Usage: install.sh [options]
  --mirror URL       Prefix GitHub downloads and save the default MirrorProxy origin
  --version VERSION  Install a release tag instead of the latest stable release
  --install-dir DIR  Directory (default: /usr/local/bin; --setup: ~/.local/bin)
  --setup            Start the Skill + MCP setup wizard after installing the client
  --base-url URL     Instance to bind in the setup wizard (required with --setup)
  -h, --help         Show this help

Environment equivalents:
  MIRRORPROXY_DOWNLOAD_MIRROR, MIRRORPROXY_VERSION,
  MIRRORPROXY_INSTALL_DIR, MIRRORPROXY_GITHUB_REPO
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --mirror)
      [ "$#" -ge 2 ] || { echo "--mirror requires a URL" >&2; exit 2; }
      mirror=$2
      shift 2
      ;;
    --version)
      [ "$#" -ge 2 ] || { echo "--version requires a release tag" >&2; exit 2; }
      version=$2
      shift 2
      ;;
    --install-dir)
      [ "$#" -ge 2 ] || { echo "--install-dir requires a directory" >&2; exit 2; }
      install_dir=$2
      install_dir_explicit=true
      shift 2
      ;;
    --setup)
      setup=true
      shift
      ;;
    --base-url)
      [ "$#" -ge 2 ] || { echo "--base-url requires a URL" >&2; exit 2; }
      setup_base_url=$2
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ "$setup" = true ]; then
  [ -n "$setup_base_url" ] || { echo "--setup requires --base-url" >&2; exit 2; }
  if [ "$install_dir_explicit" = false ]; then
    [ -n "${HOME:-}" ] || { echo "HOME is required for user installation" >&2; exit 2; }
    install_dir="${HOME}/.local/bin"
  fi
elif [ -n "$setup_base_url" ]; then
  echo "--base-url requires --setup" >&2
  exit 2
fi

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=darwin ;;
  *) echo "unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) arch=x86_64 ;;
  arm64|aarch64) arch=aarch64 ;;
  *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

case "${os}-${arch}" in
  linux-x86_64) target=x86_64-unknown-linux-musl ;;
  linux-aarch64) target=aarch64-unknown-linux-gnu ;;
  darwin-x86_64) target=x86_64-apple-darwin ;;
  darwin-aarch64) target=aarch64-apple-darwin ;;
esac

archive="mirrorproxy-client-${target}.tar.gz"
if [ "$version" = latest ]; then
  release_url="https://github.com/${repo}/releases/latest/download"
else
  release_url="https://github.com/${repo}/releases/download/${version}"
fi

mirror=${mirror%/}
download_url() {
  if [ -n "$mirror" ]; then
    printf '%s/%s' "$mirror" "$1"
  else
    printf '%s' "$1"
  fi
}

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo "tar is required" >&2; exit 1; }
command -v gzip >/dev/null 2>&1 || { echo "gzip is required" >&2; exit 1; }
command -v install >/dev/null 2>&1 || { echo "install is required" >&2; exit 1; }
tmp_dir=$(mktemp -d 2>/dev/null || mktemp -d -t mirrorproxy-install)
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT HUP INT TERM

archive_path="${tmp_dir}/${archive}"
checksum_path="${archive_path}.sha256"
echo "Downloading MirrorProxy ${version} for ${target}..."
echo "From: $(download_url "${release_url}/${archive}")"
if ! curl -fL --retry 3 --connect-timeout 15 "$(download_url "${release_url}/${archive}")" -o "$archive_path"; then
  echo "Client archive download failed. See the curl error above for network, TLS or HTTP details." >&2
  echo "If GitHub is unreachable, retry with --mirror https://YOUR-INSTANCE. Fetching this script through a proxy does not proxy subsequent downloads." >&2
  echo "For an HTTP 404, check the release tag and asset name; you can select a tag with --version <tag>." >&2
  exit 1
fi
curl -fL --retry 3 --connect-timeout 15 "$(download_url "${release_url}/${archive}.sha256")" -o "$checksum_path"

expected=$(awk 'NR == 1 { print $1 }' "$checksum_path")
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$archive_path" | awk '{ print $1 }')
elif command -v shasum >/dev/null 2>&1; then
  actual=$(shasum -a 256 "$archive_path" | awk '{ print $1 }')
else
  echo "sha256sum or shasum is required" >&2
  exit 1
fi
[ "$expected" = "$actual" ] || { echo "checksum verification failed" >&2; exit 1; }

tar -xzf "$archive_path" -C "$tmp_dir"
[ -f "${tmp_dir}/mirrorproxy" ] || { echo "archive does not contain mirrorproxy" >&2; exit 1; }

# Refuse old stable clients before touching an existing installation.
if [ "$setup" = true ] && ! "${tmp_dir}/mirrorproxy" setup --help >/dev/null 2>&1; then
  echo "This client release does not support setup. Publish a stable release containing setup, or select one with --version. Existing client was not replaced." >&2
  exit 1
fi

install_binary() {
  install -d "$install_dir"
  install -m 0755 "${tmp_dir}/mirrorproxy" "${install_dir}/mirrorproxy"
}

# A fresh user may have neither ~/.local nor ~/.local/bin yet. Inspect the
# nearest existing ancestor before deciding whether privilege is needed.
writable_parent=$install_dir
while [ ! -e "$writable_parent" ]; do
  parent=$(dirname "$writable_parent")
  [ "$parent" != "$writable_parent" ] || break
  writable_parent=$parent
done
if [ -d "$writable_parent" ] && [ -w "$writable_parent" ]; then
  install_binary
elif command -v sudo >/dev/null 2>&1; then
  sudo install -d "$install_dir"
  sudo install -m 0755 "${tmp_dir}/mirrorproxy" "${install_dir}/mirrorproxy"
else
  echo "${install_dir} is not writable; rerun as root or set --install-dir" >&2
  exit 1
fi

echo "MirrorProxy installed to ${install_dir}/mirrorproxy"
if [ -n "$mirror" ] && "${install_dir}/mirrorproxy" use --help >/dev/null 2>&1; then
  if [ "$os" = linux ] && [ "$install_dir" = /usr/local/bin ] && [ "$(id -u)" -eq 0 ]; then
    "${install_dir}/mirrorproxy" use "$mirror" || echo "Default source URL was not saved." >&2
  elif [ "$os" = linux ] && [ "$install_dir" = /usr/local/bin ] && command -v sudo >/dev/null 2>&1; then
    sudo "${install_dir}/mirrorproxy" use "$mirror" || echo "Default source URL was not saved." >&2
  else
    "${install_dir}/mirrorproxy" use "$mirror" --user || echo "Default source URL was not saved." >&2
  fi
fi
case ":${PATH}:" in
  *":${install_dir}:"*) ;;
  *) echo "Add ${install_dir} to PATH before running mirrorproxy." ;;
esac

if [ "$setup" = true ]; then
  # The Rust wizard opens the controlling terminal: stdin can be a script pipe.
  "${install_dir}/mirrorproxy" setup --base-url "$setup_base_url"
fi

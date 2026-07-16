#!/usr/bin/env bash
set -uo pipefail

base="${MIRRORPROXY_PUBLIC_BASE:-https://sina.dev}"
base="${base%/}"
pass=0
fail=0
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 2; }
command -v jq >/dev/null 2>&1 || { echo "jq is required" >&2; exit 2; }

# target|adapter|method|representative public protocol path
checks=(
  'julia|julia|get|/julia/registries'
  'poetry|pypi|get|/pypi/simple/idna/'
  'uv|pypi|get|/pypi/simple/idna/'
  'pdm|pypi|get|/pypi/simple/idna/'
  'bun|npm|get|/npm/is-number'
  'nvm|nvm|get|/nvm/index.json'
  'ocaml|opam|get|/opam/repo'
  'lua|luarocks|get|/luarocks/manifest'
  'rustup|rustup|get|/rustup/dist/channel-rust-stable.toml'
  'cocoapods|cocoapods|get|/cocoapods/all_pods_versions_2_0_0.txt'
  'apt|os|get|/os/debian/dists/stable/Release'
  'dnf|os|get|/os/fedora/releases/42/Everything/x86_64/os/repodata/repomd.xml'
  'pacman|os|head|/os/archlinux/core/os/x86_64/core.db'
  'kali|os|get|/os/kali/dists/kali-rolling/Release'
  'rocky|os|get|/os/rocky/9/BaseOS/x86_64/os/repodata/repomd.xml'
  'alma|os|get|/os/alma/9/BaseOS/x86_64/os/repodata/repomd.xml'
  'manjaro|os|head|/os/manjaro/stable/core/x86_64/core.db'
  'msys2|os|head|/os/msys2/mingw/x86_64/mingw64.db'
  'raspios|os|get|/os/raspios/dists/bookworm/Release'
  'armbian|os|get|/os/armbian/dists/bookworm/Release'
  'openeuler|os|get|/os/openeuler/openEuler-24.03-LTS/OS/x86_64/repodata/repomd.xml'
  'anolis|os|get|/os/anolis/8/BaseOS/x86_64/os/repodata/repomd.xml'
  'deepin|os|get|/os/deepin/dists/beige/InRelease'
  'linuxmint|os|get|/os/linuxmint/dists/faye/Release'
  'solus|os|head|/os/solus/polaris/eopkg-index.xml.xz'
  'trisquel|os|get|/os/trisquel/dists/aramo/Release'
  'linuxlite|os|get|/os/linuxlite/dists/emerald/Release'
  'ros|os|head|/os/ros/dists'
  'netbsd|os|get|/os/netbsd/pub/NetBSD/README'
  'openbsd|os|head|/os/openbsd/pub/OpenBSD'
  'alpine|os|get|/os/alpine/MIRRORS.txt'
  'openwrt|os|head|/os/openwrt/releases'
  'xbps|os|head|/os/void/current/x86_64-repodata'
  'zypper|os|head|/os/opensuse/distribution'
  'gentoo|os|head|/os/gentoo/releases'
  'termux|os|get|/os/termux/dists/stable/InRelease'
  'flatpak|flatpak|head|/flatpak/summary'
  'nix|nix|get|/nix/nix-cache-info'
  'guix|guix|get|/guix/nix-cache-info'
  'elpa|elpa|get|/elpa/archive-contents'
  'texlive|texlive|head|/texlive/tlpkg/texlive.tlpdb'
  'winget|winget|head|/winget/cache/source.msix'
  'anaconda|anaconda|get|/anaconda/main/noarch/repodata.json'
  'npm|npm|get|/npm/is-number'
  'pip|pypi|get|/pypi/simple/idna/'
  'cargo|crates|get|/crates-index/by/te/bytes'
  'go|go|get|/goproxy/github.com/gorilla/mux/@v/v1.8.1.info'
  'composer|composer|get|/composer/packages.json'
  'maven|maven|get|/maven/org/apache/commons/commons-lang3/3.14.0/commons-lang3-3.14.0.pom'
  'rubygems|rubygems|get|/rubygems/specs.4.8.gz'
  'nuget|nuget|get|/nuget/v3/index.json'
  'cpan|cpan|get|/cpan/modules/02packages.details.txt.gz'
  'cran|cran|get|/cran/src/contrib/PACKAGES.gz'
  'hackage|hackage|head|/hackage/packages/index.tar.gz'
  'clojars|clojars|get|/clojars/ring/ring-core/1.12.2/ring-core-1.12.2.pom'
  'pub|pub|get|/pub/api/packages/http'
  'docker|oci|get|/v2/'
  'homebrew|homebrew|get|/homebrew/curl/tags/list'
  'github|github|get|/https://raw.githubusercontent.com/octocat/Hello-World/master/README'
)

curl --fail --silent --show-error --location --max-time 60 \
  "${base}/api/sources" >"${work}/sources.json"
curl --fail --silent --show-error --location --max-time 60 \
  "${base}/api/public-config" >"${work}/public-config.json"

printf '%s\n' "${checks[@]}" | cut -d'|' -f1 | sort -u >"${work}/tested-targets"
jq -r '.sources[] | select(.provider_code == "mirrorproxy" and .capability == "proxy") | .target_code' \
  "${work}/sources.json" | sort -u >"${work}/live-targets"
if ! diff -u "${work}/live-targets" "${work}/tested-targets"; then
  echo "public smoke coverage does not match the live MirrorProxy source catalog" >&2
  exit 1
fi

printf '%s\n' "${checks[@]}" | cut -d'|' -f2 | sort -u >"${work}/tested-adapters"
jq -r '.enabled_proxies[]' "${work}/public-config.json" | sort -u >"${work}/live-adapters"
if ! diff -u "${work}/live-adapters" "${work}/tested-adapters"; then
  echo "public smoke coverage does not match the live enabled proxy adapters" >&2
  exit 1
fi

for item in "${checks[@]}"; do
  IFS='|' read -r target adapter method path <<<"${item}"
  args=(--silent --show-error --location --retry 1 --retry-delay 1 --max-time 90
    --output /dev/null --write-out '%{http_code}')
  [[ "${method}" == head ]] && args+=(--head)
  result="$(curl "${args[@]}" "${base}${path}" 2>&1)"
  if [[ "${result}" =~ ^2[0-9][0-9]$ ]]; then
    printf 'PASS\t%-12s\t%-9s\t%s\t%s\n' "${target}" "${adapter}" "${result}" "${path}"
    pass=$((pass + 1))
  else
    printf 'FAIL\t%-12s\t%-9s\t%s\t%s\n' "${target}" "${adapter}" "${result}" "${path}"
    fail=$((fail + 1))
  fi
done

printf 'SUMMARY\tpass=%s\tfail=%s\ttotal=%s\n' "${pass}" "${fail}" "$((pass + fail))"
test "${fail}" -eq 0

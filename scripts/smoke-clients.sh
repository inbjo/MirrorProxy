#!/usr/bin/env bash
set -Eeuo pipefail

# Runs real public-client protocol checks against one temporary MirrorProxy
# instance. It deliberately uses isolated homes/caches and never changes the
# caller's package-manager configuration.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
port="${MIRRORPROXY_SMOKE_PORT:-39102}"
base="http://127.0.0.1:${port}"
startup_timeout="${MIRRORPROXY_SMOKE_STARTUP_TIMEOUT:-300}"

if [[ ! "${startup_timeout}" =~ ^[1-9][0-9]*$ ]]; then
  printf 'MIRRORPROXY_SMOKE_STARTUP_TIMEOUT must be a positive integer, got %s\n' "${startup_timeout}" >&2
  exit 2
fi

work="$(mktemp -d)"
config="${work}/mirrorproxy.toml"
server_log="${work}/server.log"
pid=""

cleanup() {
  [[ -n "${pid}" ]] && kill "${pid}" 2>/dev/null || true
  rm -rf "${work}"
}
trap cleanup EXIT

print_server_log() {
  if [[ -s "${server_log}" ]]; then
    printf '%s\n' '--- MirrorProxy server log ---' >&2
    cat "${server_log}" >&2
    printf '%s\n' '--- end MirrorProxy server log ---' >&2
  else
    printf 'MirrorProxy server log is empty: %s\n' "${server_log}" >&2
  fi
}

report_failure() {
  local exit_code=$?
  local line="${BASH_LINENO[0]}"
  local command="${BASH_COMMAND}"

  printf 'client smoke failed at line %s (exit %s): %s\n' \
    "${line}" "${exit_code}" "${command}" >&2
  print_server_log
  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    printf '::error title=Client smoke failed::line %s, exit %s: %s\n' \
      "${line}" "${exit_code}" "${command}"
  fi
  exit "${exit_code}"
}

trap report_failure ERR

wait_for_server() {
  local deadline=$((SECONDS + startup_timeout))
  local exit_code

  while (( SECONDS < deadline )); do
    if curl --fail --silent "${base}/healthz" >/dev/null 2>&1; then
      return 0
    fi

    if ! kill -0 "${pid}" 2>/dev/null; then
      if wait "${pid}"; then
        exit_code=0
      else
        exit_code=$?
      fi
      printf 'MirrorProxy exited with code %s before becoming healthy at %s\n' "${exit_code}" "${base}" >&2
      print_server_log
      return 1
    fi

    sleep 0.25
  done

  printf 'MirrorProxy did not become healthy at %s within %s seconds\n' "${base}" "${startup_timeout}" >&2
  print_server_log
  return 1
}

cat >"${config}" <<EOF
listen_addr = "127.0.0.1:${port}"
database_path = "${work}/mirrorproxy.sqlite3"
public_base_url = "${base}"
enabled_proxies = ["github", "composer", "oci", "npm", "nvm", "opam", "go", "crates", "pypi", "cpan", "rubygems", "maven", "nuget", "cran", "hackage", "julia", "luarocks", "cocoapods", "pub", "anaconda", "texlive", "elpa", "nix", "guix", "flatpak", "homebrew", "os"]

[upstreams]
github = "https://github.com"
github_raw = "https://raw.githubusercontent.com"
packagist = "https://repo.packagist.org"
docker_hub = "https://registry-1.docker.io"
ghcr = "https://ghcr.io"
quay = "https://quay.io"
kubernetes = "https://registry.k8s.io"
npm = "https://registry.npmjs.org"
go_proxy = "https://proxy.golang.org"
crates_index = "https://index.crates.io"
crates_api = "https://crates.io"
pypi_simple = "https://pypi.org/simple"
pypi_files = "https://files.pythonhosted.org"
cpan = "https://cpan.metacpan.org"
rubygems = "https://rubygems.org"
maven = "https://repo.maven.apache.org/maven2"
nuget = "https://api.nuget.org"
cran = "https://cloud.r-project.org"
hackage = "https://hackage.haskell.org"
luarocks = "https://luarocks.org"
EOF

cd "${root}"
cargo run --quiet --package mirrorproxy-server --bin mirrorproxy-server -- \
  --config "${config}" >"${server_log}" 2>&1 &
pid=$!
wait_for_server

git clone --quiet --depth 1 "${base}/https://github.com/octocat/Hello-World.git" "${work}/git-clone"
test -f "${work}/git-clone/README"
npm install --ignore-scripts --no-save --prefix "${work}/npm" --registry "${base}/npm/" is-number@7.0.0 >/dev/null
mkdir "${work}/yarn"
(
  cd "${work}/yarn"
  yarn add --ignore-scripts --registry "${base}/npm/" is-number@7.0.0 >/dev/null
)
mkdir "${work}/pnpm"
(
  cd "${work}/pnpm"
  pnpm add --ignore-scripts --registry "${base}/npm/" is-number@7.0.0 >/dev/null
)
GOPROXY="${base}/goproxy,direct" GOMODCACHE="${work}/gomodcache" go list -m github.com/gorilla/mux@v1.8.1 >/dev/null
mkdir -p "${work}/cargo/.cargo" "${work}/cargo/src"
touch "${work}/cargo/src/lib.rs"
cat >"${work}/cargo/Cargo.toml" <<EOF
[package]
name = "mirrorproxy-client-smoke"
version = "0.1.0"
edition = "2021"
[dependencies]
bytes = "1.10.1"
EOF
cat >"${work}/cargo/.cargo/config.toml" <<EOF
[source.crates-io]
replace-with = "mirrorproxy"
[source.mirrorproxy]
registry = "sparse+${base}/crates-index/"
EOF
(
  cd "${work}/cargo"
  CARGO_HOME="${work}/cargo-home" cargo fetch >/dev/null
)
PIP_CACHE_DIR="${work}/pip-cache" pip download --no-deps --dest "${work}/pip" --index-url "${base}/pypi/simple/" idna==3.10 >/dev/null
command -v cpanm >/dev/null
cpanm --mirror "${base}/cpan/" --mirror-only --notest --local-lib-contained "${work}/cpan" Try::Tiny >/dev/null
gem install rake --version 13.2.1 --no-document --clear-sources --source "${base}/rubygems/" --install-dir "${work}/gems" >/dev/null
mkdir -p "${work}/maven"
cat >"${work}/maven/settings.xml" <<EOF
<settings><mirrors><mirror><id>mirrorproxy</id><url>${base}/maven/</url><mirrorOf>central</mirrorOf></mirror></mirrors></settings>
EOF
mvn -s "${work}/maven/settings.xml" -Dmaven.repo.local="${work}/m2" dependency:get -Dartifact=org.apache.commons:commons-lang3:3.14.0 -q
dotnet new classlib --output "${work}/nuget" --no-restore >/dev/null
cat >"${work}/nuget/NuGet.Config" <<EOF
<configuration>
  <packageSources>
    <clear />
    <add key="mirrorproxy" value="${base}/nuget/v3/index.json" protocolVersion="3" allowInsecureConnections="true" />
  </packageSources>
</configuration>
EOF
NUGET_PACKAGES="${work}/nuget-packages" dotnet add "${work}/nuget/nuget.csproj" package Newtonsoft.Json --version 13.0.3 --source "${base}/nuget/v3/index.json" --no-restore >/dev/null
NUGET_PACKAGES="${work}/nuget-packages" dotnet restore "${work}/nuget/nuget.csproj" --configfile "${work}/nuget/NuGet.Config" >/dev/null
mkdir -p "${work}/r-library"
R_LIBS_USER="${work}/r-library" Rscript -e 'install.packages("digest", repos = commandArgs(TRUE)[1], lib = Sys.getenv("R_LIBS_USER"), quiet = TRUE)' "${base}/cran/"
mkdir -p "${work}/hackage"
cat >"${work}/hackage/config" <<EOF
remote-repo-cache: ${work}/hackage/packages
repository mirrorproxy
  url: ${base}/hackage/
  secure: False
EOF
CABAL_CONFIG="${work}/hackage/config" cabal update mirrorproxy >/dev/null
CABAL_CONFIG="${work}/hackage/config" cabal get --destdir="${work}/hackage/source" base-orphans >/dev/null
luarocks --server "${base}/luarocks/" --tree "${work}/luarocks" install luafilesystem 1.8.0-1 >/dev/null
mkdir "${work}/composer"
(
  cd "${work}/composer"
  COMPOSER_HOME="${work}/composer-home" composer init --no-interaction --name mirrorproxy/client-smoke >/dev/null
  COMPOSER_HOME="${work}/composer-home" composer config secure-http false
  COMPOSER_HOME="${work}/composer-home" composer config repositories.packagist composer "${base}/composer/"
  COMPOSER_HOME="${work}/composer-home" composer require monolog/monolog:^3 --no-interaction --no-progress >/dev/null
)

if [[ "${MIRRORPROXY_SMOKE_DOCKER:-0}" == "1" ]]; then
  docker pull "127.0.0.1:${port}/library/busybox:1.36.1" >/dev/null
fi

# Some adapters do not have a lightweight client available on every CI image.
# Exercise their real public protocol entry points through the running proxy so
# route wiring, upstream selection, streaming, and path validation stay covered.
if [[ "${MIRRORPROXY_SMOKE_EXTENDED:-0}" == "1" ]]; then
  curl --fail --silent --show-error --output /dev/null "${base}/nvm/index.json"
  curl --fail --silent --show-error --output /dev/null "${base}/opam/repo"
  curl --fail --silent --show-error --output /dev/null "${base}/rustup/dist/channel-rust-stable.toml"
  curl --fail --silent --show-error --output /dev/null "${base}/julia/registries"
  curl --fail --silent --show-error --output /dev/null "${base}/cocoapods/all_pods_versions_2_0_0.txt"
  curl --fail --silent --show-error --output /dev/null "${base}/pub/api/packages/http"
  curl --fail --silent --show-error --output /dev/null "${base}/anaconda/main/noarch/repodata.json"
  curl --fail --silent --show-error --output /dev/null "${base}/texlive/tlpkg/texlive.tlpdb"
  curl --fail --silent --show-error --output /dev/null "${base}/elpa/archive-contents"
  curl --fail --silent --show-error --output /dev/null "${base}/nix/nix-cache-info"
  curl --fail --silent --show-error --output /dev/null "${base}/guix/nix-cache-info"
  curl --fail --silent --show-error --output /dev/null "${base}/flatpak/summary"
  curl --fail --silent --show-error --output /dev/null "${base}/homebrew/curl/tags/list"
  curl --fail --silent --show-error --output /dev/null "${base}/os/debian/dists/stable/Release"
fi

printf 'client smoke passed: git npm yarn pnpm go cargo pip cpanm rubygems maven nuget cran cabal luarocks composer%s\n' \
  "$([[ "${MIRRORPROXY_SMOKE_DOCKER:-0}" == "1" ]] && printf ' docker' || true)"

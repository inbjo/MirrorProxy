#!/usr/bin/env bash
set -euo pipefail

# Runs real public-client protocol checks against one temporary MirrorProxy
# instance. It deliberately uses isolated homes/caches and never changes the
# caller's package-manager configuration.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
port="${MIRRORPROXY_SMOKE_PORT:-39102}"
base="http://127.0.0.1:${port}"
work="$(mktemp -d)"
config="${work}/mirrorproxy.toml"
pid=""

cleanup() {
  [[ -n "${pid}" ]] && kill "${pid}" 2>/dev/null || true
  rm -rf "${work}"
}
trap cleanup EXIT

cat >"${config}" <<EOF
listen_addr = "127.0.0.1:${port}"
database_path = "${work}/mirrorproxy.sqlite3"
public_base_url = "${base}"
enabled_proxies = ["github", "composer", "oci", "npm", "go", "crates", "pypi", "cpan", "rubygems", "maven", "nuget"]

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
EOF

cd "${root}"
cargo run --quiet -- --config "${config}" >"${work}/server.log" 2>&1 &
pid=$!
for _ in {1..40}; do
  curl --fail --silent "${base}/healthz" >/dev/null && break
  sleep 0.25
done
curl --fail --silent "${base}/healthz" >/dev/null

git ls-remote "${base}/https://github.com/rust-lang/cargo.git" HEAD >/dev/null
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
mkdir -p "${work}/cargo/.cargo"
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
CARGO_HOME="${work}/cargo-home" cargo fetch --manifest-path "${work}/cargo/Cargo.toml" >/dev/null
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
NUGET_PACKAGES="${work}/nuget-packages" dotnet add "${work}/nuget/nuget.csproj" package Newtonsoft.Json --version 13.0.3 --source "${base}/nuget/v3/index.json" --no-restore >/dev/null
NUGET_PACKAGES="${work}/nuget-packages" dotnet restore "${work}/nuget/nuget.csproj" --source "${base}/nuget/v3/index.json" >/dev/null
mkdir "${work}/composer"
(
  cd "${work}/composer"
  COMPOSER_HOME="${work}/composer-home" composer init --no-interaction --name mirrorproxy/client-smoke >/dev/null
  COMPOSER_HOME="${work}/composer-home" composer config repositories.packagist composer "${base}/composer/"
  COMPOSER_HOME="${work}/composer-home" composer require monolog/monolog:^3 --no-interaction --no-progress >/dev/null
)

if [[ "${MIRRORPROXY_SMOKE_DOCKER:-0}" == "1" ]]; then
  docker pull "127.0.0.1:${port}/library/busybox:1.36.1" >/dev/null
fi

printf 'client smoke passed: git npm yarn pnpm go cargo pip cpanm rubygems maven nuget composer%s\n' \
  "$([[ "${MIRRORPROXY_SMOKE_DOCKER:-0}" == "1" ]] && printf ' docker' || true)"

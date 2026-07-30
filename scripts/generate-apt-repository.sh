#!/usr/bin/env bash
set -euo pipefail

artifacts="${1:?usage: generate-apt-repository.sh ARTIFACT_DIR OUTPUT_DIR [SUITE]}"
output="${2:?output directory required}"
suite="${3:-stable}"

for command in dpkg-scanpackages apt-ftparchive gzip gpg; do
  command -v "$command" >/dev/null || {
    echo "required command is unavailable: $command" >&2
    exit 1
  }
done

mapfile -t packages < <(find "$artifacts" -name 'mirrorproxy_*_*.deb' -type f | sort)
if [[ "${#packages[@]}" -eq 0 ]]; then
  echo "no mirrorproxy client DEB packages found in $artifacts" >&2
  exit 1
fi

rm -rf -- "$output"
mkdir -p "$output/pool/main/m/mirrorproxy"
for package in "${packages[@]}"; do
  cp "$package" "$output/pool/main/m/mirrorproxy/"
done

architectures=()
declare -A seen_architectures=()
while IFS= read -r package; do
  arch="$(dpkg-deb -f "$package" Architecture)"
  if [[ -z "${seen_architectures[$arch]+configured}" ]]; then
    architectures+=("$arch")
    seen_architectures["$arch"]=1
  fi
done < <(find "$output/pool" -name '*.deb' -type f | sort)

for arch in "${architectures[@]}"; do
  index_dir="$output/dists/$suite/main/binary-$arch"
  mkdir -p "$index_dir"
  (
    cd "$output"
    dpkg-scanpackages --arch "$arch" pool /dev/null >"dists/$suite/main/binary-$arch/Packages"
  )
  gzip -9n -c "$index_dir/Packages" >"$index_dir/Packages.gz"
done

release="$output/dists/$suite/Release"
apt-ftparchive \
  -o APT::FTPArchive::Release::Origin=MirrorProxy \
  -o APT::FTPArchive::Release::Label=MirrorProxy \
  -o APT::FTPArchive::Release::Suite="$suite" \
  -o APT::FTPArchive::Release::Codename="$suite" \
  -o APT::FTPArchive::Release::Architectures="${architectures[*]}" \
  -o APT::FTPArchive::Release::Components=main \
  -o APT::FTPArchive::Release::Description="MirrorProxy client packages" \
  release "$output/dists/$suite" >"$release"

if [[ -z "${APT_GPG_PRIVATE_KEY:-}" ]]; then
  echo "APT_GPG_PRIVATE_KEY is required to sign the APT repository" >&2
  exit 1
fi

gpg_home="$(mktemp -d)"
chmod 700 "$gpg_home"
cleanup() {
  if [[ -n "$gpg_home" && "$gpg_home" == /tmp/* ]]; then
    rm -rf -- "$gpg_home"
  fi
}
trap cleanup EXIT
export GNUPGHOME="$gpg_home"

printf '%s' "$APT_GPG_PRIVATE_KEY" | gpg --batch --import >/dev/null 2>&1
fingerprint="$(gpg --batch --with-colons --list-secret-keys | awk -F: '$1 == "fpr" { print $10; exit }')"
if [[ -z "$fingerprint" ]]; then
  echo "APT_GPG_PRIVATE_KEY does not contain a secret signing key" >&2
  exit 1
fi

sign_args=(--batch --yes --pinentry-mode loopback --local-user "$fingerprint")
if [[ -n "${APT_GPG_PASSPHRASE:-}" ]]; then
  sign_args+=(--passphrase "$APT_GPG_PASSPHRASE")
fi
gpg "${sign_args[@]}" --armor --detach-sign \
  --output "$output/dists/$suite/Release.gpg" "$release"
gpg "${sign_args[@]}" --armor --clearsign \
  --output "$output/dists/$suite/InRelease" "$release"
gpg --batch --yes --armor --export "$fingerprint" >"$output/mirrorproxy-archive-keyring.asc"
gpg --batch --yes --export "$fingerprint" >"$output/mirrorproxy-archive-keyring.gpg"

cat >"$output/index.html" <<EOF
<!doctype html>
<meta charset="utf-8">
<title>MirrorProxy APT repository</title>
<h1>MirrorProxy APT repository</h1>
<p>Suite: <code>$suite</code>; architectures: <code>${architectures[*]}</code>.</p>
EOF

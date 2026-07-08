#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${TARGET:-x86_64-unknown-linux-musl}"
BIN_NAME="mirrorproxy"

cd "$ROOT_DIR/web"
npm ci
npm run build

cd "$ROOT_DIR"
rustup target add "$TARGET"
cargo build --release --target "$TARGET"

BIN_PATH="$ROOT_DIR/target/$TARGET/release/$BIN_NAME"
echo "Built: $BIN_PATH"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$BIN_PATH"
fi


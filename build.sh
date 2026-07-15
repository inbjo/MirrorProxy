#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${TARGET:-x86_64-unknown-linux-musl}"
SERVER_PACKAGE="mirrorproxy-server"
SERVER_BIN="mirrorproxy-server"
CLIENT_PACKAGE="mirrorproxy-cli"
CLIENT_BIN="mirrorproxy"

cd "$ROOT_DIR/web"
npm ci
npm run build

cd "$ROOT_DIR"
if [[ "$TARGET" == "x86_64-unknown-linux-musl" ]] && ! command -v musl-gcc >/dev/null 2>&1; then
  echo "missing musl-gcc; install musl-tools before building $TARGET" >&2
  exit 1
fi
rustup target add "$TARGET"
cargo build --release --target "$TARGET" --package "$SERVER_PACKAGE" --bin "$SERVER_BIN"
cargo build --release --target "$TARGET" --package "$CLIENT_PACKAGE" --bin "$CLIENT_BIN"

SERVER_BIN_PATH="$ROOT_DIR/target/$TARGET/release/$SERVER_BIN"
CLIENT_BIN_PATH="$ROOT_DIR/target/$TARGET/release/$CLIENT_BIN"
echo "Built server: $SERVER_BIN_PATH"
echo "Built client: $CLIENT_BIN_PATH"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$SERVER_BIN_PATH" "$CLIENT_BIN_PATH"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$SERVER_BIN_PATH" "$CLIENT_BIN_PATH"
else
  echo "warning: no SHA-256 command was found" >&2
fi

#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

cargo build --release --locked

VERSION=$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)
mkdir -p dist
ARCHIVE="dist/vimpp-${VERSION}-linux-x86_64.tar.gz"
tar -czf "$ARCHIVE" \
    -C "$ROOT_DIR/target/release" vimpp \
    -C "$ROOT_DIR" README.md LICENSE
printf 'Created %s\n' "$ARCHIVE"

#!/usr/bin/env bash
set -euo pipefail

BIN_NAME=zenith
BUILD_DIR=target/release
LOCAL_DIR="$HOME/.local/bin"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: cargo not found. Install Rust toolchain first." >&2
  exit 1
fi

echo "Building release..."
cargo build --release

if [ "${1:-}" = "--system" ]; then
  echo "Installing ${BIN_NAME} to /usr/local/bin (requires sudo)"
  sudo install -m 0755 "$BUILD_DIR/$BIN_NAME" /usr/local/bin/
  echo "Installed to /usr/local/bin"
  exit 0
fi

mkdir -p "$LOCAL_DIR"
install -m 0755 "$BUILD_DIR/$BIN_NAME" "$LOCAL_DIR/"

echo "Installed $BIN_NAME to $LOCAL_DIR"
echo "Make sure $LOCAL_DIR is in your PATH. For example, add to ~/.profile or ~/.bashrc:"
echo "  export PATH=\"$LOCAL_DIR:\$PATH\""

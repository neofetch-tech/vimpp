#!/usr/bin/env sh
set -eu

REPO="neofetch-tech/vimpp"
API="https://api.github.com/repos/$REPO/releases/latest"
BIN_DIR="${HOME}/.local/bin"
mkdir -p "$BIN_DIR"

if command -v curl >/dev/null 2>&1; then
    release=$(curl -fsSL "$API")
elif command -v wget >/dev/null 2>&1; then
    release=$(wget -qO- "$API")
else
    echo "Install curl or wget first." >&2
    exit 1
fi

url=$(printf '%s' "$release" | sed -n 's/.*"browser_download_url": "\([^"]*linux-x86_64\.tar\.gz\)".*/\1/p' | head -n 1)
if [ -z "$url" ]; then
    echo "No Linux x86_64 release asset found for $REPO." >&2
    exit 1
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
if command -v curl >/dev/null 2>&1; then curl -fsSL "$url" -o "$tmp/vimpp.tar.gz"; else wget -q "$url" -O "$tmp/vimpp.tar.gz"; fi
tar -xzf "$tmp/vimpp.tar.gz" -C "$tmp"
install -m 755 "$tmp/vimpp" "$BIN_DIR/vimpp"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Add this to your shell profile: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac
echo "Installed vimpp to $BIN_DIR/vimpp"
echo "Run: vimpp [file-or-directory]"

#!/bin/sh
set -e

REPO="aleandros/mdx"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  os="unknown-linux-gnu" ;;
  Darwin) os="apple-darwin" ;;
  *)      echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)  arch="x86_64" ;;
  aarch64|arm64) arch="aarch64" ;;
  *)             echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

TARGET="${arch}-${os}"

# Get latest release tag
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST" ]; then
  echo "Could not determine latest release" >&2
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${LATEST}/mdx-${TARGET}.tar.gz"
echo "Downloading mdx ${LATEST} for ${TARGET}..."

# Choose install directory
if [ -w /usr/local/bin ]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="${HOME}/.local/bin"
  mkdir -p "$INSTALL_DIR"
fi

# Download and extract
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
curl -fsSL "$URL" | tar xz -C "$TMPDIR"
mv "$TMPDIR/mdx" "$INSTALL_DIR/mdx"
chmod +x "$INSTALL_DIR/mdx"

echo "Installed mdx to ${INSTALL_DIR}/mdx"

# Check if install dir is in PATH
case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "Note: Add ${INSTALL_DIR} to your PATH" ;;
esac

#!/usr/bin/env bash
set -e

REPO="mr-lexus/harnesscope"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

echo "============================================================"
echo "  Harnesscope Installer (macOS & Linux)"
echo "============================================================"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64) TARGET="aarch64-apple-darwin" ;;
      x86_64) TARGET="x86_64-apple-darwin" ;;
      *) echo "Unsupported macOS architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
      *) echo "Unsupported Linux architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS. On Windows, please run install.ps1 in PowerShell."
    exit 1
    ;;
esac

echo "Detected platform: $OS ($ARCH) -> $TARGET"

TAG="$(curl -fsSL https://api.github.com/repos/$REPO/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)"
if [ -z "$TAG" ]; then
  TAG="v0.1.0"
fi

ARCHIVE="harnesscope-${TAG}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"

echo "Downloading ${URL}..."
TMP_DIR="$(mktemp -d)"
curl -fsSL "$URL" -o "${TMP_DIR}/${ARCHIVE}"

echo "Extracting binary..."
tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "${TMP_DIR}"

mkdir -p "$INSTALL_DIR"
mv "${TMP_DIR}/harnesscope" "${INSTALL_DIR}/harnesscope"
chmod +x "${INSTALL_DIR}/harnesscope"
rm -rf "$TMP_DIR"

echo "✓ Successfully installed harnesscope to ${INSTALL_DIR}/harnesscope"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo ""
  echo "NOTE: ${INSTALL_DIR} is not in your current PATH."
  echo "Add it by running:"
  echo "  export PATH=\"\$PATH:${INSTALL_DIR}\""
  echo "Or add that line to your ~/.bashrc or ~/.zshrc."
fi

echo ""
echo "Verify installation:"
echo "  harnesscope --version"
echo "  harnesscope doctor"
echo "  harnesscope ui"

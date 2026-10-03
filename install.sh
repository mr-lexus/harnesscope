#!/usr/bin/env bash
set -euo pipefail

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
if [[ ! "$TAG" =~ ^v[0-9][0-9A-Za-z.-]*$ ]]; then
  echo "Could not determine a valid release version. Please retry later." >&2
  exit 1
fi

ARCHIVE="harnesscope-${TAG}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"

echo "Downloading ${URL}..."
TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TMP_DIR"' EXIT
curl -fsSL "$URL" -o "${TMP_DIR}/${ARCHIVE}"
curl -fsSL "https://github.com/${REPO}/releases/download/${TAG}/checksums.txt" -o "${TMP_DIR}/checksums.txt"
EXPECTED="$(awk -v name="$ARCHIVE" '$2 == name {print $1}' "${TMP_DIR}/checksums.txt")"
if [[ ! "$EXPECTED" =~ ^[a-fA-F0-9]{64}$ ]]; then
  echo "Release checksum missing or invalid." >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL="$(sha256sum "${TMP_DIR}/${ARCHIVE}" | awk '{print $1}')"
else
  ACTUAL="$(shasum -a 256 "${TMP_DIR}/${ARCHIVE}" | awk '{print $1}')"
fi
if [ "$ACTUAL" != "$EXPECTED" ]; then
  echo "Checksum mismatch; installation aborted." >&2
  exit 1
fi

echo "Extracting binary..."
tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "${TMP_DIR}" harnesscope

mkdir -p "$INSTALL_DIR"
mv "${TMP_DIR}/harnesscope" "${INSTALL_DIR}/harnesscope"
chmod +x "${INSTALL_DIR}/harnesscope"

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

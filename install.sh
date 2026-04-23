#!/bin/sh
# Install community-agent binary from GitHub releases.
# Usage: curl -sSL https://raw.githubusercontent.com/jackeycui7/awp-community-skill/main/install.sh | sh
set -e

REPO="jackeycui7/awp-community-skill"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)   OS_NAME="linux" ;;
  Darwin)  OS_NAME="darwin" ;;
  *)       echo "Error: unsupported OS: ${OS}"; exit 1 ;;
esac

case "${ARCH}" in
  x86_64|amd64)   ARCH_NAME="x86_64" ;;
  aarch64|arm64)   ARCH_NAME="aarch64" ;;
  *)               echo "Error: unsupported architecture: ${ARCH}"; exit 1 ;;
esac

if [ "${OS_NAME}" = "linux" ] && [ "${ARCH_NAME}" = "x86_64" ]; then
  BINARY_NAME="community-agent-linux-x86_64-musl"
else
  BINARY_NAME="community-agent-${OS_NAME}-${ARCH_NAME}"
fi

echo "Fetching latest release..."
LATEST=$(curl -sSL -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | sed 's/.*: "\(.*\)".*/\1/')

if [ -z "${LATEST}" ]; then
  echo "Error: could not find latest release."
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${LATEST}/${BINARY_NAME}"

echo "Downloading community-agent ${LATEST} for ${OS_NAME}/${ARCH_NAME}..."
TMPFILE=$(mktemp)
HTTP_CODE=$(curl -sSL -w "%{http_code}" -o "${TMPFILE}" "${URL}")

if [ "${HTTP_CODE}" != "200" ]; then
  rm -f "${TMPFILE}"
  echo "Error: download failed (HTTP ${HTTP_CODE})"
  exit 1
fi

chmod +x "${TMPFILE}"

if [ -w "${INSTALL_DIR}" ]; then
  mv "${TMPFILE}" "${INSTALL_DIR}/community-agent"
else
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "${TMPFILE}" "${INSTALL_DIR}/community-agent"
fi

if [ "${OS_NAME}" = "darwin" ]; then
  xattr -d com.apple.quarantine "${INSTALL_DIR}/community-agent" 2>/dev/null || true
fi

echo ""
echo "community-agent ${LATEST} installed to ${INSTALL_DIR}/community-agent"
echo "Getting started: community-agent status"

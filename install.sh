#!/bin/sh
# Install community-agent binary from GitHub releases.
# Usage: sh install.sh   (or pipe from curl/wget/python)
#
# Runtime requirements — any ONE of:
#   * curl
#   * wget
#   * python3 (stdlib only; uses urllib.request)
#
# Plus: a shell (sh/bash) and write access to INSTALL_DIR or sudo.

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
  aarch64|arm64)  ARCH_NAME="aarch64" ;;
  *)              echo "Error: unsupported architecture: ${ARCH}"; exit 1 ;;
esac

if [ "${OS_NAME}" = "linux" ] && [ "${ARCH_NAME}" = "x86_64" ]; then
  BINARY_NAME="community-agent-linux-x86_64-musl"
else
  BINARY_NAME="community-agent-${OS_NAME}-${ARCH_NAME}"
fi

# Fetch helper: prints URL contents to stdout. First available wins.
fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c "
import sys, urllib.request
try:
    with urllib.request.urlopen(sys.argv[1], timeout=60) as r:
        sys.stdout.buffer.write(r.read())
except Exception as e:
    print('fetch failed:', e, file=sys.stderr); sys.exit(1)
" "$1"
  else
    echo "Error: need curl, wget, or python3 to download — none found" >&2
    return 1
  fi
}

# Save helper: writes URL contents to a file.
save() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1"
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c "
import sys, urllib.request
try:
    urllib.request.urlretrieve(sys.argv[1], sys.argv[2])
except Exception as e:
    print('download failed:', e, file=sys.stderr); sys.exit(1)
" "$1" "$2"
  else
    return 1
  fi
}

echo "Fetching latest release..."
LATEST="$(fetch "https://api.github.com/repos/${REPO}/releases/latest" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])' 2>/dev/null \
  || fetch "https://api.github.com/repos/${REPO}/releases/latest" \
       | grep '"tag_name"' | head -1 | sed 's/.*: "\(.*\)".*/\1/')"

if [ -z "${LATEST}" ]; then
  echo "Error: could not find latest release of ${REPO}" >&2
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${LATEST}/${BINARY_NAME}"

echo "Downloading community-agent ${LATEST} for ${OS_NAME}/${ARCH_NAME}..."
TMPFILE="$(mktemp)"
if ! save "${URL}" "${TMPFILE}"; then
  rm -f "${TMPFILE}"
  echo "Error: download failed from ${URL}" >&2
  exit 1
fi

chmod +x "${TMPFILE}"
mkdir -p "${INSTALL_DIR}"

if [ -w "${INSTALL_DIR}" ] || [ "${INSTALL_DIR}" = "${HOME}/.local/bin" ]; then
  mv "${TMPFILE}" "${INSTALL_DIR}/community-agent"
elif command -v sudo >/dev/null 2>&1; then
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "${TMPFILE}" "${INSTALL_DIR}/community-agent"
else
  # No sudo, target not writable — fall back to ~/.local/bin
  mkdir -p "${HOME}/.local/bin"
  mv "${TMPFILE}" "${HOME}/.local/bin/community-agent"
  INSTALL_DIR="${HOME}/.local/bin"
  echo "Note: installed to ${HOME}/.local/bin — add it to PATH if not already"
fi

if [ "${OS_NAME}" = "darwin" ]; then
  xattr -d com.apple.quarantine "${INSTALL_DIR}/community-agent" 2>/dev/null || true
fi

echo ""
echo "community-agent ${LATEST} installed to ${INSTALL_DIR}/community-agent"
if ! command -v community-agent >/dev/null 2>&1; then
  echo ""
  echo "Warning: ${INSTALL_DIR} is not in PATH. Add this to your shell rc:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

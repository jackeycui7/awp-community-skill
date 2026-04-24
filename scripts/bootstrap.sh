#!/bin/sh
# Thin wrapper around `community-agent bootstrap`.
# Installs community-agent if missing, then hands off to the Rust
# subcommand which does all real work (no shell loops, no python).
set -e
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
export PATH="$INSTALL_DIR:$PATH"
if ! command -v community-agent >/dev/null 2>&1; then
  # Fetch + run our platform install.sh (handles curl/wget/python3
  # fallbacks itself).
  TMP="$(mktemp)"
  URL="https://raw.githubusercontent.com/jackeycui7/awp-community-skill/main/install.sh"
  if   command -v curl    >/dev/null 2>&1; then curl -fsSL -o "$TMP" "$URL"
  elif command -v wget    >/dev/null 2>&1; then wget -qO  "$TMP" "$URL"
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c "import sys, urllib.request; urllib.request.urlretrieve(sys.argv[1], sys.argv[2])" "$URL" "$TMP"
  else
    echo "need curl, wget, or python3 to fetch community-agent" >&2; exit 1
  fi
  INSTALL_DIR="$INSTALL_DIR" sh "$TMP"
  rm -f "$TMP"
fi
exec community-agent bootstrap "$@"

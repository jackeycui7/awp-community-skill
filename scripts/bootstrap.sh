#!/bin/sh
# community-worknet bootstrap — idempotent, non-interactive.
#
# What this does:
#   1. Ensures `community-agent` is installed (from our GitHub releases).
#   2. Ensures `awp-wallet` is installed (from the AWP Wallet skill repo).
#   3. Ensures the wallet is initialized (`awp-wallet init` — zero user input).
#   4. Ensures the wallet address is registered on the AWP network. If
#      not, installs awp-skill and runs its onboarding script.
#
# Does NOT do:
#   - Register the community identity (that's `community-agent register`,
#     invoked by the caller with a chosen name).
#   - Ask the user for any secret. The agent wallet is generated locally
#     with no password / seed / PIN. See README for the full "never ask"
#     list.
#
# Re-run safely: every step checks current state first and exits early.

set -e

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"
export PATH="$INSTALL_DIR:$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.yarn/bin:$PATH"

log() { echo "[bootstrap] $*" >&2; }
die() { echo "[bootstrap ERROR] $*" >&2; exit 1; }

# ---------- 1. community-agent ----------
if ! command -v community-agent >/dev/null 2>&1; then
  log "installing community-agent from GitHub releases..."
  curl -fsSL https://raw.githubusercontent.com/jackeycui7/awp-community-skill/main/install.sh \
    | INSTALL_DIR="$INSTALL_DIR" sh
fi
command -v community-agent >/dev/null 2>&1 \
  || die "community-agent install failed — check $INSTALL_DIR is in PATH"
log "community-agent $(community-agent --version 2>&1 | awk '{print $2}') ready"

# ---------- 2. awp-wallet ----------
find_wallet() {
  command -v awp-wallet 2>/dev/null && return
  for p in "$HOME/.local/bin/awp-wallet" "$HOME/.npm-global/bin/awp-wallet" \
           "$HOME/.yarn/bin/awp-wallet" "/usr/local/bin/awp-wallet"; do
    [ -x "$p" ] && echo "$p" && return
  done
  return 1
}

WALLET_BIN="$(find_wallet || true)"
if [ -z "$WALLET_BIN" ]; then
  log "installing awp-wallet..."
  # Prefer npm if available — awp-wallet publishes a node package
  if command -v npm >/dev/null 2>&1; then
    npm install -g awp-wallet >/dev/null 2>&1 \
      || die "npm install awp-wallet failed"
  else
    die "awp-wallet not found and npm unavailable — install Node first or clone https://github.com/awp-core/awp-wallet manually"
  fi
  WALLET_BIN="$(find_wallet || true)"
fi
[ -n "$WALLET_BIN" ] || die "awp-wallet still not on PATH after install"
export PATH="$(dirname "$WALLET_BIN"):$PATH"
log "awp-wallet at $WALLET_BIN"

# ---------- 3. wallet init (non-interactive) ----------
if ! awp-wallet receive >/dev/null 2>&1; then
  log "initializing agent wallet (generates fresh keypair locally, no prompts)..."
  awp-wallet init >/dev/null 2>&1 || die "awp-wallet init failed"
fi
WALLET_ADDR="$(awp-wallet receive 2>/dev/null \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("eoaAddress") or d.get("address",""))' \
  2>/dev/null || true)"
case "$WALLET_ADDR" in
  0x????????????????????????????????????????) ;;
  *) die "could not read wallet address: got \"$WALLET_ADDR\"" ;;
esac
log "agent wallet: $WALLET_ADDR"

# ---------- 4. AWP-chain registration ----------
REG_STATUS="$(community-agent awp-register --address "$WALLET_ADDR" 2>/dev/null \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print("yes" if d.get("ok") and d.get("data",{}).get("registered") else "no")' \
  2>/dev/null || echo no)"

if [ "$REG_STATUS" != "yes" ]; then
  log "not yet registered on AWP — attempting gasless self-register via community-agent awp-register"
  if community-agent awp-register --address "$WALLET_ADDR" >/dev/null 2>&1; then
    log "registered via community-agent awp-register"
  else
    log "community-agent awp-register failed — falling back to awp-skill onboarding"
    AWPSKILL_DIR="${AWPSKILL_DIR:-$HOME/.local/share/awp-skill}"
    if [ ! -d "$AWPSKILL_DIR" ]; then
      git clone --depth 1 https://github.com/awp-core/awp-skill "$AWPSKILL_DIR" \
        || die "awp-skill clone failed"
    fi
    # The onboarding script prefers a token from awp-wallet. Newer
    # wallets run tokenless — try both.
    TOKEN="${AWP_WALLET_TOKEN:-$(awp-wallet unlock 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("token",""))' 2>/dev/null || true)}"
    if [ -n "$TOKEN" ]; then
      python3 "$AWPSKILL_DIR/scripts/onchain-onboard.py" --token "$TOKEN" \
        || die "awp-skill onboarding failed — inspect script output"
    else
      python3 "$AWPSKILL_DIR/scripts/onchain-onboard.py" \
        || die "awp-skill onboarding failed — inspect script output"
    fi
  fi
fi

# Verify
if ! community-agent awp-register --address "$WALLET_ADDR" >/dev/null 2>&1; then
  die "verification still reports unregistered after onboarding — check api.awp.sh reachable and the address indeed got registered"
fi
log "AWP-chain registration confirmed for $WALLET_ADDR"

log "bootstrap complete. next step: community-agent register --name <your_agent_name>"
log "the register command will auto-pick up $WALLET_ADDR from awp-wallet."

#!/bin/sh
# community-worknet smoke test — verifies the runtime is ready to post.
#
# Exits 0 if every check passes, 1 otherwise. Safe to run at any time:
# reads only, never posts, never signs.

set -e
log() { echo "[smoke] $*" >&2; }
fail() { echo "[smoke FAIL] $*" >&2; exit 1; }

command -v community-agent >/dev/null 2>&1 \
  || fail "community-agent not on PATH"
log "community-agent $(community-agent --version 2>&1 | awk '{print $2}')"

STATUS=$(community-agent status 2>/dev/null \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print("ok" if d.get("ok") else "fail")' \
  2>/dev/null || echo "fail")
[ "$STATUS" = "ok" ] || fail "community-agent status — server unreachable"
log "server reachable"

command -v awp-wallet >/dev/null 2>&1 \
  || fail "awp-wallet not on PATH (run scripts/bootstrap.sh)"
WADDR=$(awp-wallet receive 2>/dev/null \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("eoaAddress") or d.get("address",""))' \
  2>/dev/null || true)
case "$WADDR" in
  0x????????????????????????????????????????) ;;
  *) fail "awp-wallet not initialized — run scripts/bootstrap.sh" ;;
esac
log "wallet $WADDR"

community-agent awp-register --address "$WADDR" 2>/dev/null \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") and d.get("data",{}).get("registered") else 1)' \
  || fail "wallet $WADDR not registered on AWP — run scripts/bootstrap.sh"
log "AWP registration ok"

if [ -n "${COMMUNITY_API_KEY:-}" ]; then
  community-agent me 2>/dev/null \
    | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") else 1)' \
    || fail "community-agent me failed — api_key rejected"
  log "community identity ok"
else
  log "COMMUNITY_API_KEY not set — skipping identity check"
fi

echo "[smoke] all checks passed"

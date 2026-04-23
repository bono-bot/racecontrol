#!/bin/bash
# deploy-server.sh — Bulletproof racecontrol server deploy (v3.0)
# MMA-audited: 8 models across 2 rounds. All P1s fixed.
# Usage: bash deploy-server.sh [hash]

set -euo pipefail

HASH="${1:-}"
if [ -z "$HASH" ]; then
  HASH=$(cd C:/Users/bono/racingpoint/racecontrol && git rev-parse --short HEAD)
  echo "Auto-detected hash: $HASH"
fi

BINARY="racecontrol-${HASH}.exe"
STAGING="C:/Users/bono/racingpoint/deploy-staging"
SERVER_LAN="192.168.31.23"
SERVER_TS="100.125.108.37"
SERVER_USER="ADMIN"
REMOTE_DIR="C:\\RacingPoint"
HTTP_SERVER="http://192.168.31.27:18889"

# ─── Rollback helper ─────────────────────────────────────────────────────────
# StartRCTemp is RETIRED per Phase 413.1-04 (silent no-op when ADMIN is not
# interactively logged in). Use StartRCDirect which launches under
# SYSTEM/Interactive-Background via start-racecontrol-direct.bat. Verified
# working 2026-04-19 post-swap-incident recovery.
rollback() {
  echo "!!! ROLLING BACK !!!"
  # Each cmd.exe step gated on prior success via `&&` (conditional-and),
  # not `&` (unconditional). Previous version would print ROLLBACK_OK even
  # when `del racecontrol.exe` silently failed on a locked handle (Windows
  # held the exe while watchdog respawner restarted the new binary), making
  # the caller think rollback succeeded when racecontrol-prev.exe was never
  # moved back. Fix: `del && ren && echo` — no echo means either del or ren
  # genuinely failed, caller sees the failure, operator can intervene.
  # Observed 2026-04-20 07:10 + 17:27 IST: both rollbacks were cosmetic-only
  # (new binary stayed live, prev never restored) because the echo always fired.
  local rb_out rollback_class
  rb_out=$($SSH "cd /d ${REMOTE_DIR} && if exist racecontrol-prev.exe (del /F /Q racecontrol.exe && ren racecontrol-prev.exe racecontrol.exe && echo ROLLBACK_OK) else (echo NO_PREV_BINARY)" 2>/dev/null || echo "SSH_FAIL")
  if echo "$rb_out" | grep -q "ROLLBACK_OK"; then
    echo "  Rollback swap OK"
    rollback_class="SWAP_OK"
  elif echo "$rb_out" | grep -q "NO_PREV_BINARY"; then
    echo "  ROLLBACK ABORTED: racecontrol-prev.exe not present — cannot rollback"
    echo "  Manual recovery: ssh ${SERVER_USER}@${SERVER_IP} — inspect ${REMOTE_DIR}"
    rollback_class="NO_PREV_BINARY"
  else
    echo "  ROLLBACK SWAP FAILED (del or ren errored): $rb_out"
    echo "  Manual recovery: ssh ${SERVER_USER}@${SERVER_IP} — inspect ${REMOTE_DIR}"
    echo "  Most likely cause: racecontrol.exe is held open by a running process;"
    echo "  taskkill /F /IM racecontrol.exe first, then retry the rollback."
    rollback_class="FAILED"
  fi
  $SSH "schtasks /Run /TN StartRCDirect" 2>/dev/null || echo "  StartRCDirect trigger failed (non-fatal — watchdog may recover)"
  echo "Rolled back. Waiting 15s for startup..."
  sleep 15
  local rb_health
  rb_health=$(curl -sf --max-time 5 "http://${SERVER_LAN}:8080/api/v1/health" 2>/dev/null || echo "UNREACHABLE")
  echo "Rollback health: $rb_health"

  # rollback() is invoked from steps 5/6/7/7b after step 4 binary swap completed.
  # Every attempt (SWAP_OK / NO_PREV_BINARY / FAILED) gets a SWAPLOG row so
  # fleet-swaplog-parity can correlate server-SHA-change events with rollbacks.
  # Best-effort — a SWAPLOG append failure must NOT block rollback exit.
  # Path detection matches step 7c; SWAPLOG_PATH env override supported.
  SWAPLOG_PATH="${SWAPLOG_PATH:-$(cd "$(dirname "$0")/../racecontrol" 2>/dev/null && pwd)/SWAPLOG.md}"
  if [ -f "$SWAPLOG_PATH" ]; then
    TS_IST=$(python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M:%S IST'))" 2>/dev/null || date -u +"%Y-%m-%d %H:%M:%S UTC")
    TRIGGERED_BY="${SWAPLOG_TRIGGERED_BY:-${USER:-unknown}@$(hostname 2>/dev/null || echo unknown)}"
    REASON="${SWAPLOG_REASON:-deploy-server.sh rollback from $HASH (no SWAPLOG_REASON env set)}"
    printf '| %s | ROLLBACK | - | - | %s | [ROLLBACK %s from %s -> prev] %s |\n' \
      "$TS_IST" "$TRIGGERED_BY" "$rollback_class" "$HASH" "$REASON" \
      >> "$SWAPLOG_PATH" 2>/dev/null \
      && echo "  SWAPLOG rollback appended: $SWAPLOG_PATH" \
      || echo "  SWAPLOG rollback append failed (non-fatal): $SWAPLOG_PATH"
  else
    echo "  SWAPLOG rollback skip: $SWAPLOG_PATH not found (set SWAPLOG_PATH env to override)"
  fi

  exit 1
}

# ─── Step 0a: Deploy lock (GAP-5 wiring) ─────────────────────────────────────
LOCK_SCRIPT="C:/Users/bono/racingpoint/comms-link/shared/deploy-lock.js"
if [ -f "$LOCK_SCRIPT" ]; then
  echo "[0a] Acquiring deploy lock..."
  node "$LOCK_SCRIPT" acquire server "deploy racecontrol $HASH" || {
    echo "  ABORT: Another deploy is in progress. Run: node $LOCK_SCRIPT check"
    exit 1
  }
  # Release lock on exit (success or failure)
  trap 'node "$LOCK_SCRIPT" release 2>/dev/null; echo "Deploy lock released."' EXIT
fi

# ─── Step 0b: Connectivity (LAN primary, Tailscale fallback) ─────────────────
echo "[0/8] Testing connectivity..."
if ssh -o ConnectTimeout=3 -o StrictHostKeyChecking=no ${SERVER_USER}@${SERVER_LAN} "echo OK" 2>/dev/null | grep -q OK; then
  SERVER_IP="$SERVER_LAN"
  echo "  Using LAN: $SERVER_IP"
elif ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no ${SERVER_USER}@${SERVER_TS} "echo OK" 2>/dev/null | grep -q OK; then
  SERVER_IP="$SERVER_TS"
  echo "  Using Tailscale: $SERVER_IP"
else
  echo "  ABORT: Cannot reach server via LAN or Tailscale"
  exit 1
fi

SSH="ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no ${SERVER_USER}@${SERVER_IP}"

# ─── Step 1: Verify binary staged locally ─────────────────────────────────────
echo "[1/8] Verifying local binary..."
LOCAL_BIN="${STAGING}/${BINARY}"
if [ ! -f "$LOCAL_BIN" ]; then
  echo "  ABORT: ${BINARY} not found in deploy-staging/"
  exit 1
fi
LOCAL_SIZE=$(stat -c%s "$LOCAL_BIN" 2>/dev/null || wc -c < "$LOCAL_BIN" | tr -d ' ')
echo "  Found: $BINARY ($LOCAL_SIZE bytes)"

# Truth-gate: verify the binary's embedded GIT_HASH matches the filename.
# racecontrol --build-id exits before mutex/config/DB/port-bind (introspection
# fast-path in main.rs) so this is safe to run on the build machine against
# the staging binary. Catches the "built before commit" class of bug that
# landed a97c7491-dirty into rc-agent-b2463d81.exe on 2026-04-19 Pod 8 canary.
LOCAL_EMBEDDED=$("$LOCAL_BIN" --build-id 2>/dev/null | tr -d '\r\n' | head -1)
if [ "$LOCAL_EMBEDDED" != "$HASH" ]; then
  echo "  ABORT: $LOCAL_BIN embeds GIT_HASH='${LOCAL_EMBEDDED:-EMPTY}', filename claims '$HASH'."
  echo "         Rebuild from clean tree: (cd racecontrol && git status && cargo build --release --bin racecontrol)"
  exit 1
fi
echo "  Truth-gate OK: binary embeds GIT_HASH='$LOCAL_EMBEDDED'"

# ─── Step 2: Download to server ───────────────────────────────────────────────
echo "[2/8] Downloading to server..."
RESULT=$($SSH "curl.exe -s -o ${REMOTE_DIR}\\${BINARY} ${HTTP_SERVER}/${BINARY} && echo DL_OK" 2>/dev/null || echo "FAIL")
if ! echo "$RESULT" | grep -q "DL_OK"; then
  echo "  HTTP download failed. Trying SCP..."
  scp -o ConnectTimeout=10 "$LOCAL_BIN" "${SERVER_USER}@${SERVER_IP}:${REMOTE_DIR}/${BINARY}"
fi
# Verify size matches (filter SSH banners — P1 fix from MMA Round 2)
REMOTE_SIZE=$($SSH "for %a in (${REMOTE_DIR}\\${BINARY}) do @echo %~za" 2>/dev/null | tr -d '\r' | grep -E '^[0-9]+$' | tail -1)
if [ -z "$REMOTE_SIZE" ] || [ "$LOCAL_SIZE" != "$REMOTE_SIZE" ]; then
  echo "  ABORT: Size mismatch! Local=$LOCAL_SIZE Remote=${REMOTE_SIZE:-EMPTY}"
  exit 1
fi
echo "  Downloaded ($REMOTE_SIZE bytes, verified)"

# ─── Step 3: Kill old process with confirmed death ────────────────────────────
echo "[3/8] Atomic binary swap WHILE old process runs (Windows allows rename of running exe)..."
# Three-step swap: del prev → ren current→prev → ren new→current.
#
# IMPORTANT: del prev uses `&` not `&&`. If racecontrol-prev.exe doesn't exist,
# `del ... 2>nul` returns non-zero on some Windows builds — which with `&&`
# short-circuits the chain and renames NEVER RUN. But the `||` fallback branch
# sees racecontrol.exe still present and prints SWAP_FAILED even though nothing
# was actually attempted. 2026-04-19 incident: the three renames DID succeed
# (racecontrol.exe on disk was the new binary post-abort) but grep missed
# SWAP_OK because `del`'s non-zero exit broke the cmd chain before echo.
# Fix: `&` (unconditional) so del failure is non-fatal, matches pod-deploy.sh.
# Defensive pipe: grep no-match returns 1 and with `set -o pipefail` the whole
# capture fails, which with `set -e` kills the script BEFORE the size-fallback
# below can run. Wrap with `|| true` so empty SWAP_RESULT just falls through.
SWAP_RESULT=$({ $SSH "cd /d ${REMOTE_DIR} && del racecontrol-prev.exe 2>nul & ren racecontrol.exe racecontrol-prev.exe && ren ${BINARY} racecontrol.exe && echo SWAP_OK" 2>/dev/null || true; } | tr -d '\r' | { grep -E "SWAP_OK" || true; } | tail -1)

# If the three-step rename didn't print SWAP_OK, one of three things happened:
# (1) ren racecontrol.exe failed (running-process lock — rare on Windows which
#     normally allows renaming a running exe);
# (2) ren ${BINARY} failed (new binary missing on disk, or name conflict);
# (3) cmd.exe mangled operator precedence between & and && and dropped the echo
#     (observed 2026-04-19 even with renames materially succeeding on disk).
# Fallback: directly verify by stat'ing racecontrol.exe size vs the expected
# local binary size, and inspect whether the new binary actually landed.
if [ "$SWAP_RESULT" != "SWAP_OK" ]; then
  # Defensive pipeline: set -o pipefail would otherwise kill the script when
  # grep returns no match. Wrap in `|| echo ""` so the assignment always
  # succeeds even if racecontrol.exe is missing.
  POST_SIZE=$({ $SSH "for %a in (${REMOTE_DIR}\\racecontrol.exe) do @echo %~za" 2>/dev/null || echo ""; } | tr -d '\r' | { grep -E '^[0-9]+$' || true; } | tail -1)
  if [ -n "$POST_SIZE" ] && [ "$POST_SIZE" = "$LOCAL_SIZE" ]; then
    echo "  Swap echo missing but racecontrol.exe size=$POST_SIZE matches new binary — treating as SWAP_OK (cmd.exe echo-drop)"
    SWAP_RESULT="SWAP_OK"
  else
    echo "  Swap verify: echo='${SWAP_RESULT}' remote-size='${POST_SIZE}' local-size='${LOCAL_SIZE}'"
  fi
fi

if [ "$SWAP_RESULT" = "SWAP_OK" ]; then
  echo "  Swapped: ${BINARY} -> racecontrol.exe (prev preserved, old process still running)"
elif [ "$SWAP_RESULT" = "SWAP_RECOVERED" ]; then
  echo "  ABORT: Swap failed but recovered prev binary. New binary may be missing."
  exit 1
else
  echo "  ABORT: Swap failed! Check server manually."
  exit 1
fi

echo "[4/8] Killing old process — watchdog/sentry will auto-restart new binary..."
$SSH "taskkill /F /IM racecontrol.exe 2>nul" 2>/dev/null || echo "  (not running — unusual but OK)"
sleep 2

# ─── Step 5: Wait for watchdog to restart new binary ─────────────────────────
echo "[5/8] Waiting for watchdog auto-restart of new binary..."
# Watchdog/sentry will spawn new racecontrol.exe (which is now our new binary) within seconds.
# Poll up to 30s for a new racecontrol.exe process.
RUNNING=0
for i in $(seq 1 10); do
  sleep 3
  RUNNING=$($SSH "tasklist /FI \"IMAGENAME eq racecontrol.exe\" /NH 2>nul" 2>/dev/null | grep -ic "racecontrol" || echo "0")
  if [ "$RUNNING" != "0" ]; then
    echo "  racecontrol.exe respawned (attempt $i, ~$((i*3))s)"
    break
  fi
  echo "  Waiting for respawn... (attempt $i)"
done
if [ "$RUNNING" = "0" ]; then
  echo "  Watchdog did not restart — manual trigger via StartRCDirect..."
  $SSH "schtasks /Run /TN StartRCDirect" 2>/dev/null || true
  sleep 10
  RUNNING=$($SSH "tasklist /FI \"IMAGENAME eq racecontrol.exe\" /NH 2>nul" 2>/dev/null | grep -ic "racecontrol" || echo "0")
fi
if [ "$RUNNING" = "0" ]; then
  echo "  FATAL: Server did not start"
  rollback
fi
echo "  Server process running"

# ─── Step 6: Verify build_id (fail = rollback) ──────────────────────────────
# Previously: 3 retries × 3s sleep = 9s total wait. racecontrol cold-start takes
# 15-25s on .23 (DB migrations, encryption key load, 44-table venue_id migration,
# route registration, TLS setup). 9s hit UNREACHABLE and triggered a false
# rollback on 2026-04-19 even though the new binary started cleanly and was
# answering ~5s after the window closed. Widen to 20 retries × 3s = 60s total.
echo "[6/8] Verifying build_id..."
sleep 5
BUILD_ID="UNREACHABLE"
for i in $(seq 1 20); do
  BUILD_ID=$(curl -sf --max-time 5 "http://${SERVER_LAN}:8080/api/v1/health" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('build_id','UNKNOWN'))" 2>/dev/null || echo "UNREACHABLE")
  if [ "$BUILD_ID" = "$HASH" ]; then break; fi
  sleep 3
done

if [ "$BUILD_ID" = "$HASH" ]; then
  echo "  Build ID verified: $BUILD_ID"
else
  echo "  FATAL: Build ID mismatch! Expected=$HASH Got=$BUILD_ID"
  rollback
fi

# ─── Step 7: Smoke test (fail = rollback) ────────────────────────────────────
# Per-endpoint expected status. Public endpoints expect 200. Staff-JWT-protected
# endpoints (debug/activity, debug/playbooks since v27.0 security hardening)
# expect 401 — that proves both (a) the route is registered and (b) the auth
# middleware is wired. A 200 on a protected endpoint would mean auth was
# bypassed (worse); 404 means the route is gone; 5xx means the handler panicked.
#
# Prior bug (observed 2026-04-20 07:10 + 17:27 IST): loop accepted only 200 or
# 307, so debug endpoints returning 401 were logged as FAIL, triggering false
# rollbacks on otherwise-healthy deploys. `curl -sf` with -f also exits 22 on
# non-2xx which complicates stdout capture — drop -f, handle status codes
# explicitly.
echo "[7/8] Smoke testing endpoints..."
PASS=0; FAIL=0
smoke_check() {
  local ep="$1"
  local expected="$2"
  local code
  code=$(curl -s --max-time 5 -o /dev/null -w '%{http_code}' "http://${SERVER_LAN}:8080/api/v1/${ep}" 2>/dev/null || echo "000")
  if [ "$code" = "$expected" ]; then
    echo "  /${ep} -> $code OK (expected $expected)"
    PASS=$((PASS+1))
  else
    echo "  /${ep} -> $code FAIL (expected $expected)"
    FAIL=$((FAIL+1))
  fi
}
smoke_check "health"           "200"
smoke_check "fleet/health"     "200"
smoke_check "debug/activity"   "401"
smoke_check "debug/playbooks"  "401"

echo "  Smoke test: $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
  echo "  FATAL: $FAIL endpoint(s) failed smoke test."
  rollback
fi

# ─── Step 7b: SHA256 verification (GAP-10 wiring) ────────────────────────────
echo "[7b] Verifying SHA256..."
LOCAL_SHA=$(sha256sum "$LOCAL_BIN" 2>/dev/null | awk '{print $1}')
REMOTE_SHA=$($SSH "certutil -hashfile ${REMOTE_DIR}\\racecontrol.exe SHA256 2>nul" 2>/dev/null | tr -d '\r' | grep -E '^[a-f0-9]{64}$' | head -1)
if [ -n "$LOCAL_SHA" ] && [ -n "$REMOTE_SHA" ]; then
  if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
    echo "  SHA256 match: ${LOCAL_SHA:0:16}..."
  else
    echo "  FATAL: SHA256 mismatch! Local=${LOCAL_SHA:0:16} Remote=${REMOTE_SHA:0:16}"
    echo "  Binary corrupted during transfer — rolling back."
    rollback
  fi
else
  echo "  SKIP: Could not compute SHA256 (local=$LOCAL_SHA remote=$REMOTE_SHA)"
fi

# ─── Step 7c: Append to SWAPLOG.md ───────────────────────────────────────────
# Every successful swap is recorded in racecontrol/SWAPLOG.md so parallel
# sessions can detect between-session drift. Rule documented in CLAUDE.md.
# Best-effort — a SWAPLOG append failure must NOT rollback the deploy.
# Prior bug (observed: 11 consecutive manual appends through 2026-04-20 17:27 IST):
# path derivation had an extra `racingpoint/` — script at deploy-staging/ would
# resolve `$(dirname)/../racingpoint/racecontrol` to .../racingpoint/racingpoint/
# racecontrol which doesn't exist. `if [ -f ... ]` always false → block fell into
# "SWAPLOG skip: not found" branch. Correct path from deploy-staging/ is
# `../racecontrol/SWAPLOG.md` (drop the extra racingpoint/ component).
SWAPLOG_PATH="${SWAPLOG_PATH:-$(cd "$(dirname "$0")/../racecontrol" 2>/dev/null && pwd)/SWAPLOG.md}"
if [ -f "$SWAPLOG_PATH" ]; then
  SIZE_LOCAL=$(stat -c%s "$LOCAL_BIN" 2>/dev/null || echo "unknown")
  SHA_SHORT="${LOCAL_SHA:0:16}"
  TS_IST=$(python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M:%S IST'))" 2>/dev/null || date -u +"%Y-%m-%d %H:%M:%S UTC")
  TRIGGERED_BY="${SWAPLOG_TRIGGERED_BY:-${USER:-unknown}@$(hostname 2>/dev/null || echo unknown)}"
  REASON="${SWAPLOG_REASON:-deploy-server.sh $HASH (no SWAPLOG_REASON env set)}"
  printf '| %s | %s | %s | %s | %s | %s |\n' \
    "$TS_IST" "$HASH" "$SIZE_LOCAL" "$SHA_SHORT" "$TRIGGERED_BY" "$REASON" \
    >> "$SWAPLOG_PATH" 2>/dev/null \
    && echo "  SWAPLOG appended: $SWAPLOG_PATH" \
    || echo "  SWAPLOG append failed (non-fatal): $SWAPLOG_PATH"
else
  echo "  SWAPLOG skip: $SWAPLOG_PATH not found (set SWAPLOG_PATH env to override)"
fi

# ─── Step 8: Cleanup stale binaries ──────────────────────────────────────────
echo "[8/8] Cleaning stale binaries..."
# List hash-named binaries that are NOT racecontrol-prev.exe, delete them
# Uses dir /b to list, then explicit exclusion (safe for cmd.exe over SSH)
$SSH "cd /d ${REMOTE_DIR} && for /f \"delims=\" %f in ('dir /b racecontrol-*.exe 2^>nul') do @if /I not \"%f\"==\"racecontrol-prev.exe\" del \"%f\" 2>nul" 2>/dev/null || true
echo "  Kept: racecontrol.exe + racecontrol-prev.exe"

echo ""
echo "========================================="
echo "  DEPLOY SUCCESS: racecontrol $HASH"
echo "  Server: http://${SERVER_LAN}:8080"
echo "  Rollback: ssh ${SERVER_USER}@${SERVER_IP} 'cd /d ${REMOTE_DIR} && ren racecontrol.exe racecontrol-bad.exe && ren racecontrol-prev.exe racecontrol.exe && schtasks /Run /TN StartRCTemp'"
echo "========================================="

#!/usr/bin/env bash
# bono-auto-detect.sh — Bono-side Autonomous Bug Detection (James Failover)
#
# Runs on Bono VPS when James is down. Checks all reachable infrastructure,
# applies safe fixes, and notifies Uday via WhatsApp.
#
# Usage (on Bono VPS):
#   AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh [--mode quick|standard]
#
# Cron (recommended):
#   0 2 * * * AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh --mode standard >> /root/auto-detect.log 2>&1
#
# What Bono CAN check (without James):
#   - Venue server health (via Tailscale)
#   - Cloud racecontrol health (local)
#   - Pod fleet health (via server API)
#   - Next.js apps (via server IP)
#   - Build consistency
#   - Git sync state
#
# What Bono CANNOT do (James required):
#   - Run audit protocol (requires server_ops :8090 exec)
#   - Run comms-link integration tests (requires James relay)
#   - Direct pod exec (requires LAN access)

set -euo pipefail

TIMESTAMP=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')
LOG_DIR="/root/auto-detect-logs"
LOG_FILE="$LOG_DIR/bono-auto-detect-$(TZ=Asia/Kolkata date '+%Y-%m-%d_%H-%M').log"
SERVER_URL="${SERVER_URL:-http://192.168.31.23:8080}"
CLOUD_URL="${CLOUD_URL:-http://localhost:8080}"
JAMES_RELAY="${JAMES_RELAY:-http://100.82.33.94:8766}"
MODE="${1:-quick}"
EVOLUTION_URL="${EVOLUTION_URL:-http://localhost:53622}"
EVOLUTION_API_KEY="${EVOLUTION_API_KEY:-zNAKEHsXudyqL3dFngyBJAZWw9W4hWN0}"
EVOLUTION_INSTANCE="${EVOLUTION_INSTANCE:-Racing Point Reception}"
UDAY_PHONE="${UDAY_PHONE:-919059833001}"

mkdir -p "$LOG_DIR"

log() {
  local level="$1"; shift
  echo "[$(TZ=Asia/Kolkata date '+%H:%M:%S')] [$level] $*" | tee -a "$LOG_FILE"
}

notify_uday() {
  local msg="$1"
  # WhatsApp via Evolution API (staff phone)
  curl -s --max-time 10 -X POST \
    "${EVOLUTION_URL}/message/sendText/${EVOLUTION_INSTANCE}" \
    -H "Content-Type: application/json" \
    -H "apikey: ${EVOLUTION_API_KEY}" \
    -d "{\"number\":\"${UDAY_PHONE}\",\"text\":\"${msg}\"}" >/dev/null 2>&1 || log WARN "WhatsApp notify failed"
}

# ─── Check James Status ──────────────────────────────────────────────────────
james_alive=false
james_health=$(curl -s --max-time 5 "$JAMES_RELAY/relay/health" 2>/dev/null || echo "")
if echo "$james_health" | jq -e '.status == "ok"' >/dev/null 2>&1; then
  james_alive=true
  log INFO "James relay: ALIVE — delegating to James auto-detect"
  # If James is alive, tell James to run auto-detect instead
  curl -s --max-time 5 -X POST "$JAMES_RELAY/relay/exec/run" \
    -H "Content-Type: application/json" \
    -d '{"command":"shell","args":"cd /c/Users/bono/racingpoint/racecontrol && AUDIT_PIN=261121 bash scripts/auto-detect.sh --mode standard","reason":"bono-triggered auto-detect"}' >/dev/null 2>&1 || true
  log INFO "Delegated to James. Exiting."
  exit 0
fi

log INFO "╔══════════════════════════════════════════════╗"
log INFO "║  BONO AUTONOMOUS DETECTION (James DOWN)      ║"
log INFO "║  $TIMESTAMP                                   ║"
log INFO "╚══════════════════════════════════════════════╝"

BUGS_FOUND=0
BUGS_FIXED=0

# ─── Check 1: Venue Server ───────────────────────────────────────────────────
log INFO "=== Check 1: Venue Server ==="
server_health=$(curl -s --max-time 10 "$SERVER_URL/api/v1/health" 2>/dev/null || echo "")
server_status=$(echo "$server_health" | jq -r '.status // ""' 2>/dev/null || echo "")
server_build=$(echo "$server_health" | jq -r '.build_id // ""' 2>/dev/null || echo "")

if [[ "$server_status" == "ok" ]]; then
  log INFO "Venue server: OK (build=$server_build)"
else
  log ERROR "Venue server: DOWN or unreachable"
  BUGS_FOUND=$((BUGS_FOUND + 1))

  # Auto-fix: try to restart via Tailscale SSH
  log INFO "  Attempting restart via Tailscale SSH..."
  ssh -o ConnectTimeout=10 -o StrictHostKeyChecking=no ADMIN@100.125.108.37 \
    "schtasks /Run /TN StartRCDirect" 2>/dev/null && {
    log INFO "  Restart command sent. Waiting 30s..."
    sleep 30
    server_health=$(curl -s --max-time 10 "$SERVER_URL/api/v1/health" 2>/dev/null || echo "")
    server_status=$(echo "$server_health" | jq -r '.status // ""' 2>/dev/null || echo "")
    if [[ "$server_status" == "ok" ]]; then
      log INFO "  Server recovered after restart!"
      BUGS_FIXED=$((BUGS_FIXED + 1))
    else
      log ERROR "  Server still down after restart attempt"
      # Activate failover
      log INFO "  Activating cloud failover..."
      pm2 start racecontrol 2>/dev/null || true
      notify_uday "ALERT: Venue server down. Cloud failover ACTIVATED. James also down. Manual check needed."
    fi
  } || {
    log ERROR "  SSH to server failed. Activating failover..."
    pm2 start racecontrol 2>/dev/null || true
    BUGS_FOUND=$((BUGS_FOUND + 1))
    notify_uday "CRITICAL: Venue server AND James both unreachable. Cloud failover activated. Uday please check venue."
  }
fi

# ─── Check 2: Cloud Racecontrol ──────────────────────────────────────────────
log INFO "=== Check 2: Cloud Racecontrol ==="
cloud_health=$(curl -s --max-time 5 "$CLOUD_URL/api/v1/health" 2>/dev/null || echo "")
cloud_status=$(echo "$cloud_health" | jq -r '.status // ""' 2>/dev/null || echo "")
cloud_build=$(echo "$cloud_health" | jq -r '.build_id // ""' 2>/dev/null || echo "")

if [[ "$cloud_status" == "ok" ]]; then
  log INFO "Cloud racecontrol: OK (build=$cloud_build)"
else
  log WARN "Cloud racecontrol: not running (expected if no failover active)"
fi

# ─── Check 3: Fleet Health (via server, if reachable) ────────────────────────
if [[ "$server_status" == "ok" ]]; then
  log INFO "=== Check 3: Fleet Health ==="
  fleet_health=$(curl -s --max-time 10 "$SERVER_URL/api/v1/fleet/health" 2>/dev/null || echo "[]")
  pod_count=$(echo "$fleet_health" | jq 'length' 2>/dev/null || echo "0")
  pods_connected=$(echo "$fleet_health" | jq '[.[] | select(.ws_connected == true)] | length' 2>/dev/null || echo "0")
  pods_down=$(echo "$fleet_health" | jq '[.[] | select(.ws_connected == false or .ws_connected == null)] | length' 2>/dev/null || echo "0")

  log INFO "Fleet: $pods_connected/$pod_count pods connected, $pods_down down"

  if [[ "$pods_down" -gt 0 ]]; then
    local down_pods
    down_pods=$(echo "$fleet_health" | jq -r '.[] | select(.ws_connected == false or .ws_connected == null) | .pod_number' 2>/dev/null || echo "")
    log WARN "Pods DOWN: $down_pods"
    BUGS_FOUND=$((BUGS_FOUND + 1))
  fi
fi

# ─── Check 4: Next.js Apps ───────────────────────────────────────────────────
if [[ "$server_status" == "ok" ]]; then
  log INFO "=== Check 4: Next.js Apps ==="
  for app_check in "web:3200:/api/health" "admin:3201:/api/health" "kiosk:3300:/kiosk/api/health"; do
    IFS=':' read -r app_name app_port app_path <<< "$app_check"
    local app_health
    app_health=$(curl -s --max-time 5 "http://192.168.31.23:${app_port}${app_path}" 2>/dev/null || echo "")
    local app_status
    app_status=$(echo "$app_health" | jq -r '.status // ""' 2>/dev/null || echo "")
    if [[ "$app_status" == "ok" ]]; then
      log INFO "  $app_name (:$app_port): OK"
    else
      log WARN "  $app_name (:$app_port): $app_status"
      BUGS_FOUND=$((BUGS_FOUND + 1))
    fi
  done
fi

# ─── Check 5: Git Sync ───────────────────────────────────────────────────────
log INFO "=== Check 5: Git Sync ==="
cd /root/comms-link 2>/dev/null && {
  git fetch origin --quiet 2>/dev/null || true
  local behind
  behind=$(git rev-list HEAD..origin/main --count 2>/dev/null || echo "0")
  if [[ "$behind" -gt 0 ]]; then
    log WARN "comms-link: $behind commits behind origin — pulling..."
    git pull --quiet 2>/dev/null || true
    BUGS_FIXED=$((BUGS_FIXED + 1))
  else
    log INFO "comms-link: up to date"
  fi
}

cd /root/racecontrol 2>/dev/null && {
  git fetch origin --quiet 2>/dev/null || true
  local behind_rc
  behind_rc=$(git rev-list HEAD..origin/main --count 2>/dev/null || echo "0")
  if [[ "$behind_rc" -gt 0 ]]; then
    log WARN "racecontrol: $behind_rc commits behind origin — pulling..."
    git pull --quiet 2>/dev/null || true
    BUGS_FIXED=$((BUGS_FIXED + 1))
  else
    log INFO "racecontrol: up to date"
  fi
}

# ─── Summary ──────────────────────────────────────────────────────────────────
log INFO "─── BONO AUTO-DETECT SUMMARY ───"
log INFO "James: DOWN"
log INFO "Bugs found: $BUGS_FOUND | Fixed: $BUGS_FIXED"
log INFO "Server: $server_status | Cloud: $cloud_status"
log INFO "Log: $LOG_FILE"
log INFO "Standing rule: When James recovers, run full auto-detect on James side"
log INFO "────────────────────────────────"

# Notify Uday if bugs found
if [[ "$BUGS_FOUND" -gt 0 ]]; then
  notify_uday "Bono Auto-Detect ($TIMESTAMP): James DOWN. Found $BUGS_FOUND issues, fixed $BUGS_FIXED. Server: $server_status. Check $LOG_FILE for details."
fi

exit $(( BUGS_FOUND - BUGS_FIXED > 0 ? 1 : 0 ))

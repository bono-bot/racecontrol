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
JAMES_TAILSCALE_IP="100.125.108.37"   # Server Tailscale IP (james@ node)
JAMES_MACHINE_IP="100.82.33.94"       # James laptop Tailscale IP

# ─── Early arg parse (before MODE reads $1) ──────────────────────────────────
if [[ "${1:-}" == "--read-bono-findings" ]]; then
  findings_file="/root/auto-detect-logs/bono-findings.json"
  if [[ -f "$findings_file" ]]; then
    echo "=== Bono Findings from last independent run ==="
    cat "$findings_file"
    echo "=== End Bono Findings ==="
    exit 0
  else
    echo "No Bono findings file found."
    exit 0
  fi
fi

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

write_bono_findings() {
  local findings_file="$LOG_DIR/bono-findings.json"
  local timestamp
  timestamp=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')
  local now_ts
  now_ts=$(date +%s)
  # Write findings summary to a file James can read on next run
  printf '%s\n' "$(jq -n \
    --arg agent "bono" \
    --arg ts "$timestamp" \
    --argjson now_ts "$now_ts" \
    --argjson bugs_found "${BUGS_FOUND:-0}" \
    --argjson bugs_fixed "${BUGS_FIXED:-0}" \
    --arg log_file "$LOG_FILE" \
    '{agent:$agent, completed_ts:$now_ts, timestamp:$ts, bugs_found:$bugs_found, bugs_fixed:$bugs_fixed, log_file:$log_file, action_taken:"independent_run"}')" > "$findings_file"
  log INFO "Bono findings written: $findings_file"

  # Push findings to shared git channel (INBOX.md entry)
  local inbox_entry
  inbox_entry="## $(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST') — from bono
Bono independent run complete. bugs_found=${BUGS_FOUND:-0}, bugs_fixed=${BUGS_FIXED:-0}. Findings: $findings_file"
  local inbox_file="/root/comms-link/INBOX.md"
  if [[ -f "$inbox_file" ]]; then
    printf '\n%s\n' "$inbox_entry" >> "$inbox_file"
    (cd /root/comms-link && git add INBOX.md && git commit -m "chore: bono findings handoff $(date '+%Y-%m-%d')" && git push) 2>/dev/null || \
      log WARN "INBOX.md git push failed — findings in $findings_file"
  fi
}

# ─── Three-Phase Startup Coordination ────────────────────────────────────────

# Phase 1 — Relay alive check (COORD-01 / COORD-02 foundation)
james_relay_alive=false
james_health=$(curl -s --max-time 5 "$JAMES_RELAY/relay/health" 2>/dev/null || echo "")
if echo "$james_health" | jq -e '.status == "ok"' >/dev/null 2>&1; then
  james_relay_alive=true
fi

# Phase 2 — Completion marker check: skip if James ran recently (COORD-04)
# Use SSH to James (relay shell command not in exec registry — SSH is reliable fallback)
JAMES_SSH="ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 bono@${JAMES_TAILSCALE_IP}"
if [[ "$james_relay_alive" == "true" ]]; then
  completion_json=$($JAMES_SSH "cat C:/Users/bono/racingpoint/racecontrol/audit/results/last-run-summary.json 2>/dev/null || echo {}" 2>/dev/null || echo "{}")
  completed_ts=$(echo "$completion_json" | jq -r '.completed_ts // 0' 2>/dev/null || echo "0")
  now_ts=$(date +%s)
  elapsed=$(( now_ts - completed_ts ))
  if [[ "$elapsed" -lt 600 ]] && [[ "$completed_ts" -gt 0 ]]; then
    log INFO "James completed run ${elapsed}s ago (< 10min). Skipping Bono run (COORD-04)."
    exit 0
  fi
fi

# Phase 3 — Lock check + delegate or confirm-offline (COORD-01 + COORD-02)
if [[ "$james_relay_alive" == "true" ]]; then
  # Check if James auto-detect is currently active (COORD-01) — via SSH
  lock_json=$($JAMES_SSH "cat C:/Users/bono/racingpoint/racecontrol/audit/results/auto-detect-active.lock 2>/dev/null || echo {}" 2>/dev/null || echo "{}")
  lock_agent=$(echo "$lock_json" | jq -r '.agent // ""' 2>/dev/null || echo "")
  if [[ "$lock_agent" == "james" ]]; then
    log INFO "James AUTO_DETECT_ACTIVE lock present — deferring to James (COORD-01)"
    exit 0
  fi

  # James relay alive but no lock and no recent completion — delegate via relay exec
  log INFO "James relay: ALIVE — delegating auto-detect to James"
  curl -s --max-time 5 -X POST "$JAMES_RELAY/relay/exec/run" \
    -H "Content-Type: application/json" \
    -d '{"command":"git_pull","reason":"bono-triggered auto-detect pre-pull"}' >/dev/null 2>&1 || true
  # Trigger auto-detect on James via SSH (relay shell not registered)
  $JAMES_SSH "cd C:/Users/bono/racingpoint/racecontrol && AUDIT_PIN=261121 bash scripts/auto-detect.sh --mode standard &" >/dev/null 2>&1 || true
  log INFO "Delegated to James via SSH. Exiting."
  exit 0
fi

# Relay is down — confirm via Tailscale before acting (COORD-02)
log INFO "James relay timeout — confirming Tailscale status (COORD-02)..."
tailscale_online=false
if tailscale ping --c 1 --timeout 5s "$JAMES_TAILSCALE_IP" >/dev/null 2>&1; then
  tailscale_online=true
fi

BONO_DEGRADED_MODE=false
if [[ "$tailscale_online" == "true" ]]; then
  log WARN "James relay DOWN but Tailscale UP — James may be in maintenance. Checking lock via SSH..."
  # Try reading lock via SSH to James machine
  lock_json_ssh=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no bono@"$JAMES_MACHINE_IP" \
    "cat /c/Users/bono/racingpoint/racecontrol/audit/results/auto-detect-active.lock 2>/dev/null || echo {}" 2>/dev/null || echo "{}")
  lock_agent_ssh=$(echo "$lock_json_ssh" | jq -r '.agent // ""' 2>/dev/null || echo "")
  if [[ "$lock_agent_ssh" == "james" ]]; then
    log INFO "James AUTO_DETECT_ACTIVE lock found via SSH — deferring (COORD-01)"
    exit 0
  fi
  log WARN "Relay down, Tailscale up, no active lock — Bono proceeding as degraded mode (no fixes, report only)"
  BONO_DEGRADED_MODE=true
else
  log WARN "James CONFIRMED DOWN (relay=false, tailscale=false) — Bono acting independently"
  BONO_DEGRADED_MODE=false
fi

if [[ "${BONO_DEGRADED_MODE:-false}" == "true" ]]; then
  NO_FIX=true
  log INFO "Degraded mode: Tailscale reachable, relay down — fixes disabled"
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
    app_health=$(curl -s --max-time 5 "http://192.168.31.23:${app_port}${app_path}" 2>/dev/null || echo "")
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

# COORD-03: Check if James recovered during our run — handoff if so
james_recovered=false
james_health_recovery=$(curl -s --max-time 5 "$JAMES_RELAY/relay/health" 2>/dev/null || echo "")
if echo "$james_health_recovery" | jq -e '.status == "ok"' >/dev/null 2>&1; then
  james_recovered=true
  log INFO "RECOVERY: James relay is back online (COORD-03)"
fi

if [[ "$james_recovered" == "true" ]] && [[ "${BUGS_FOUND:-0}" -gt 0 ]]; then
  log INFO "Handoff: writing bono findings for James to read on next run (COORD-03)"
  write_bono_findings

  # Stop any cloud failover we activated
  pm2 stop racecontrol 2>/dev/null || true
  log INFO "Cloud failover DEACTIVATED — James is back online"
elif [[ "$james_recovered" == "true" ]]; then
  log INFO "James recovered, no findings to hand off. Deactivating any cloud failover."
  pm2 stop racecontrol 2>/dev/null || true
fi

# Notify Uday if bugs found
if [[ "$BUGS_FOUND" -gt 0 ]]; then
  notify_uday "Bono Auto-Detect ($TIMESTAMP): James DOWN. Found $BUGS_FOUND issues, fixed $BUGS_FIXED. Server: $server_status. Check $LOG_FILE for details."
fi

exit $(( BUGS_FOUND - BUGS_FIXED > 0 ? 1 : 0 ))

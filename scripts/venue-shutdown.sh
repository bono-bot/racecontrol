#!/usr/bin/env bash
# venue-shutdown.sh -- Ordered venue shutdown
# Called by venue_shutdown_handler via SSH from server (.23)
# Order: Pods (1-8 parallel) -> wait 45s -> POS PC (.20) -> Server (.23, 60s delay)
# James (.27) stays alive.
#
# Usage: bash scripts/venue-shutdown.sh
# Env:   SERVER_URL (default: http://192.168.31.23:8080)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_FILE="$REPO_ROOT/audit/results/venue-shutdown-$(TZ=Asia/Kolkata date '+%Y-%m-%d_%H-%M').log"
SERVER_URL="${SERVER_URL:-http://192.168.31.23:8080}"
TIMESTAMP=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')

mkdir -p "$(dirname "$LOG_FILE")"

log() {
  echo "[$(TZ=Asia/Kolkata date '+%H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

log "=== Venue Shutdown Started at $TIMESTAMP ==="
log "Server URL: $SERVER_URL"
log "Log: $LOG_FILE"

# ─── Step 1: Save pre-shutdown findings for boot-time fix ────────────────────
log "Step 1: Saving pre-shutdown findings..."
FINDINGS_FILE="$REPO_ROOT/audit/results/pre-shutdown-findings.json"
LATEST_AUTODETECT=$(ls -td "$REPO_ROOT/audit/results/auto-detect-"* 2>/dev/null | head -1)
if [[ -n "$LATEST_AUTODETECT" ]] && [[ -f "$LATEST_AUTODETECT/summary.json" ]]; then
  cp "$LATEST_AUTODETECT/summary.json" "$FINDINGS_FILE"
  log "Saved pre-shutdown findings from $LATEST_AUTODETECT"
else
  log "No auto-detect results found to save (first run or no prior audit)"
fi

# ─── Step 2: Shutdown pods 1-8 in parallel via server API ───────────────────
log "Step 2: Shutting down pods 1-8 in parallel..."
for i in $(seq 1 8); do
  (
    result=$(curl -s --max-time 30 -X POST "$SERVER_URL/api/v1/pods/pod_$i/shutdown" \
      -H "Content-Type: application/json" 2>&1 || echo "curl_failed")
    log "Pod $i: $result"
  ) &
done
wait
log "All pod shutdown commands sent"

# ─── Step 3: Wait 45 seconds for pods to power off ──────────────────────────
log "Step 3: Waiting 45s for pods to power off..."
sleep 45
log "Wait complete"

# ─── Step 4: Shutdown POS PC (.20) ──────────────────────────────────────────
log "Step 4: Shutting down POS PC (192.168.31.20)..."
ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 -o BatchMode=yes \
  POS@192.168.31.20 "shutdown /s /t 30" 2>&1 | tee -a "$LOG_FILE" \
  || log "POS shutdown failed or already offline -- continuing"

# ─── Step 5: Shutdown Server (.23) via Tailscale with 60s delay ─────────────
log "Step 5: Scheduling Server shutdown (192.168.31.23 / 100.125.108.37) in 60s..."
ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 -o BatchMode=yes \
  ADMIN@100.125.108.37 "shutdown /s /t 60" 2>&1 | tee -a "$LOG_FILE" \
  || log "Server shutdown SSH failed -- continuing"

# ─── Step 6: Notify Bono ────────────────────────────────────────────────────
log "Step 6: Notifying Bono..."
COMMS_LINK_DIR="$(cd "$REPO_ROOT/../comms-link" && pwd)"
if [[ -f "$COMMS_LINK_DIR/send-message.js" ]]; then
  cd "$COMMS_LINK_DIR" && \
    COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" \
    COMMS_URL="ws://srv1422716.hstgr.cloud:8765" \
    node send-message.js "Venue shutdown sequence complete at $TIMESTAMP. Pods->POS->Server scheduled. James (.27) staying alive. Pre-shutdown findings saved." \
    2>&1 | tee -a "$LOG_FILE" || log "Bono notify failed -- non-fatal"
else
  log "comms-link send-message.js not found at $COMMS_LINK_DIR -- Bono notify skipped"
fi

log "=== Venue Shutdown Sequence Complete. James (.27) staying alive. ==="

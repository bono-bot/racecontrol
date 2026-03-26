#!/usr/bin/env bash
# boot-time-fix.sh -- Reads pre-shutdown findings and applies safe auto-fixes
# Called by James on boot (scheduled task or startup hook).
# Checks two sources:
#   1. audit/results/pre-shutdown-findings.json (from venue-shutdown.sh)
#   2. comms-link/INBOX.md (findings from Bono fallback shutdown)
#
# Idempotent: running with no findings file is a no-op.
# Audit trail: all actions logged to LOGBOOK.md and FIXES_LOG.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMMS_LINK_DIR="$(cd "$REPO_ROOT/../comms-link" && pwd)"
FINDINGS_FILE="$REPO_ROOT/audit/results/pre-shutdown-findings.json"
LOGBOOK="$REPO_ROOT/LOGBOOK.md"
TIMESTAMP=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')
FIXES_APPLIED=0
DRY_RUN="${DRY_RUN:-false}"

log() {
  echo "[boot-time-fix] $*"
}

# ─── Step 1: Check for pre-shutdown findings file ────────────────────────────
if [[ ! -f "$FINDINGS_FILE" ]]; then
  log "No pre-shutdown findings file at $FINDINGS_FILE. Nothing to do."
  exit 0
fi

log "Found pre-shutdown findings: $FINDINGS_FILE"

# ─── Step 2: Parse findings JSON ─────────────────────────────────────────────
if ! command -v jq &>/dev/null; then
  log "ERROR: jq not found — cannot parse findings JSON. Exiting."
  exit 1
fi

if ! jq -e "." "$FINDINGS_FILE" >/dev/null 2>&1; then
  log "ERROR: findings file is not valid JSON. Skipping."
  exit 1
fi

BUGS_FOUND=$(jq -r '.bugs_found // 0' "$FINDINGS_FILE" 2>/dev/null || echo "0")
VERDICT=$(jq -r '.verdict // "UNKNOWN"' "$FINDINGS_FILE" 2>/dev/null || echo "UNKNOWN")

log "Findings: bugs_found=$BUGS_FOUND, verdict=$VERDICT"

if [[ "$BUGS_FOUND" == "0" || "$VERDICT" == "CLEAN" ]]; then
  log "Findings indicate clean state. Nothing to fix."
  ARCHIVE_NAME="$REPO_ROOT/audit/results/pre-shutdown-findings-processed-$(date '+%Y%m%d_%H%M').json"
  mv "$FINDINGS_FILE" "$ARCHIVE_NAME"
  log "Archived findings to $ARCHIVE_NAME"
  exit 0
fi

# ─── Step 3: Source fixes.sh and core.sh for APPROVED_FIXES whitelist ────────
if [[ -f "$REPO_ROOT/audit/lib/fixes.sh" ]]; then
  # shellcheck source=../audit/lib/fixes.sh
  source "$REPO_ROOT/audit/lib/fixes.sh" 2>/dev/null || true
  log "Loaded fixes.sh — APPROVED_FIXES: ${APPROVED_FIXES[*]}"
else
  log "WARNING: audit/lib/fixes.sh not found. Using empty APPROVED_FIXES."
  APPROVED_FIXES=()
fi

if [[ -f "$REPO_ROOT/audit/lib/core.sh" ]]; then
  # shellcheck source=../audit/lib/core.sh
  source "$REPO_ROOT/audit/lib/core.sh" 2>/dev/null || true
fi

# ─── Step 4: Apply safe fixes from APPROVED_FIXES whitelist ──────────────────
# Parse the steps array from findings to find fixable issues
STEPS_JSON=$(jq -c '.steps // []' "$FINDINGS_FILE" 2>/dev/null || echo "[]")
STEP_COUNT=$(echo "$STEPS_JSON" | jq 'length' 2>/dev/null || echo "0")

log "Checking $STEP_COUNT steps from pre-shutdown findings..."

# For each step with FAIL status, check if it matches an APPROVED_FIXES entry
while IFS= read -r step_json; do
  step_name=$(echo "$step_json" | jq -r '.step // ""' 2>/dev/null || echo "")
  step_status=$(echo "$step_json" | jq -r '.status // ""' 2>/dev/null || echo "")
  step_detail=$(echo "$step_json" | jq -r '.detail // ""' 2>/dev/null || echo "")

  if [[ "$step_status" != "FAIL" ]]; then
    continue
  fi

  log "Found failed step: step=$step_name detail=$step_detail"

  # Map step names to fix functions where possible
  FIX_APPLIED=false
  for fix_name in "${APPROVED_FIXES[@]:-}"; do
    # Match step name against fix name (loose match)
    if echo "$step_name $step_detail" | grep -qi "${fix_name//_/ }"; then
      if [[ "$DRY_RUN" == "true" ]]; then
        log "[DRY-RUN] Would apply fix: $fix_name"
        FIX_APPLIED=true
      elif declare -f "$fix_name" > /dev/null 2>&1; then
        # Extract pod IP from step detail if present
        pod_ip=$(echo "$step_detail" | grep -oE '192\.168\.31\.[0-9]+' | head -1 || echo "")
        if [[ -n "$pod_ip" ]]; then
          log "Applying fix $fix_name to pod $pod_ip"
          "$fix_name" "$pod_ip" && FIX_APPLIED=true || log "WARNING: fix $fix_name failed for $pod_ip"
        else
          log "Fix $fix_name requires pod_ip — skipping (no IP in: $step_detail)"
        fi
      fi
    fi
  done

  if [[ "$FIX_APPLIED" == "true" ]]; then
    FIXES_APPLIED=$((FIXES_APPLIED + 1))
    log "Fix applied for: $step_name"
  fi
done < <(echo "$STEPS_JSON" | jq -c '.[]' 2>/dev/null || true)

log "Fixes applied: $FIXES_APPLIED"

# ─── Step 5: Log to LOGBOOK.md ───────────────────────────────────────────────
if [[ -f "$LOGBOOK" ]]; then
  LOGBOOK_ENTRY="| $TIMESTAMP | James | boot-fix | Boot-time fix: applied $FIXES_APPLIED fixes from pre-shutdown findings (bugs_found=$BUGS_FOUND) |"
  echo "$LOGBOOK_ENTRY" >> "$LOGBOOK"
  log "Logged to LOGBOOK.md"
fi

# ─── Step 6: Notify Bono of fixes applied ────────────────────────────────────
if [[ $FIXES_APPLIED -gt 0 ]]; then
  log "Notifying Bono: $FIXES_APPLIED fixes applied at $TIMESTAMP"
  if [[ -f "$COMMS_LINK_DIR/send-message.js" ]]; then
    cd "$COMMS_LINK_DIR" && \
      COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" \
      COMMS_URL="ws://srv1422716.hstgr.cloud:8765" \
      node send-message.js "Boot-time fix: applied $FIXES_APPLIED fixes from pre-shutdown findings (bugs_found=$BUGS_FOUND) at $TIMESTAMP" || \
      log "WARNING: Bono notification failed (relay may be down)"
  else
    log "WARNING: send-message.js not found at $COMMS_LINK_DIR/send-message.js"
  fi
fi

# ─── Step 7: Archive the findings file (mv, not delete -- audit trail) ───────
ARCHIVE_NAME="$REPO_ROOT/audit/results/pre-shutdown-findings-processed-$(date '+%Y%m%d_%H%M').json"
mv "$FINDINGS_FILE" "$ARCHIVE_NAME"
log "Archived processed findings to $ARCHIVE_NAME"

# ─── Step 8: Check INBOX.md for Bono fallback shutdown findings ──────────────
INBOX_FILE="$COMMS_LINK_DIR/INBOX.md"
if [[ -f "$INBOX_FILE" ]]; then
  if grep -q "pre-shutdown-findings\|venue-shutdown\|bono.*fallback.*shutdown\|fallback.*shutdown" "$INBOX_FILE" 2>/dev/null; then
    log "Found Bono fallback shutdown findings in INBOX.md — manual review recommended"
    if [[ -f "$LOGBOOK" ]]; then
      echo "| $TIMESTAMP | James | boot-fix | Bono fallback findings detected in INBOX.md — manual review recommended |" >> "$LOGBOOK"
    fi
  else
    log "No Bono fallback findings in INBOX.md"
  fi
else
  log "INBOX.md not found at $INBOX_FILE"
fi

log "Boot-time fix complete. fixes_applied=$FIXES_APPLIED"

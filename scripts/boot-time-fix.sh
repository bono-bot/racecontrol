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
# Read the detailed findings array (per-finding with category/severity/pod_ip/message)
# venue-shutdown.sh saves this as pre-shutdown-findings-detail.json
DETAIL_FILE="$REPO_ROOT/audit/results/pre-shutdown-findings-detail.json"
if [[ -f "$DETAIL_FILE" ]]; then
  FINDINGS_ARRAY="$DETAIL_FILE"
else
  # Fallback: try reading findings array directly from the summary file
  # (older format may embed findings differently)
  FINDINGS_ARRAY="$FINDINGS_FILE"
fi

# Parse findings — each entry has: category, severity, pod_ip, message, issue_type
FINDING_COUNT=$(jq 'if type == "array" then length else 0 end' "$FINDINGS_ARRAY" 2>/dev/null || echo "0")

log "Checking $FINDING_COUNT findings from pre-shutdown audit..."

# For each P1/P2 finding, check if its category matches an APPROVED_FIXES entry
while IFS= read -r finding_json; do
  finding_category=$(echo "$finding_json" | jq -r '.category // .issue_type // ""' 2>/dev/null || echo "")
  finding_severity=$(echo "$finding_json" | jq -r '.severity // ""' 2>/dev/null || echo "")
  finding_pod_ip=$(echo "$finding_json" | jq -r '.pod_ip // ""' 2>/dev/null || echo "")
  finding_message=$(echo "$finding_json" | jq -r '.message // ""' 2>/dev/null || echo "")

  if [[ -z "$finding_category" ]]; then
    continue
  fi

  log "Found finding: category=$finding_category severity=$finding_severity pod=$finding_pod_ip"

  # Map finding category to fix functions
  FIX_APPLIED=false
  for fix_name in "${APPROVED_FIXES[@]:-}"; do
    # Match category or message against fix name (loose match)
    if echo "$finding_category $finding_message" | grep -qi "${fix_name//_/ }"; then
      if [[ "$DRY_RUN" == "true" ]]; then
        log "[DRY-RUN] Would apply fix: $fix_name for $finding_category on $finding_pod_ip"
        FIX_APPLIED=true
      elif declare -f "$fix_name" > /dev/null 2>&1; then
        if [[ -n "$finding_pod_ip" ]] && [[ "$finding_pod_ip" =~ ^192\.168\. ]]; then
          log "Applying fix $fix_name to pod $finding_pod_ip"
          "$fix_name" "$finding_pod_ip" && FIX_APPLIED=true || log "WARNING: fix $fix_name failed for $finding_pod_ip"
        else
          log "Fix $fix_name requires pod_ip — skipping (pod=$finding_pod_ip)"
        fi
      fi
    fi
  done

  if [[ "$FIX_APPLIED" == "true" ]]; then
    FIXES_APPLIED=$((FIXES_APPLIED + 1))
    log "Fix applied for: $finding_category on $finding_pod_ip"
  fi
done < <(jq -c 'if type == "array" then .[] else empty end' "$FINDINGS_ARRAY" 2>/dev/null || true)

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

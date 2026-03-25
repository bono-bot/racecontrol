#!/usr/bin/env bash
# audit/lib/notify.sh — Notification engine for Racing Point fleet audit
#
# Three notification channels (all failure-safe — notification failure NEVER aborts the audit):
#   1. Bono WS     — send-message.js WebSocket push for real-time AI coordination (NOTF-01)
#   2. Bono INBOX  — INBOX.md append + git push for persistent record (NOTF-02)
#   3. WhatsApp    — Uday SMS via Bono relay Evolution API (NOTF-03)
#
# All channels are gated behind --notify flag (NOTIFY env var): off by default (NOTF-04).
# Delta summary included when previous run exists (NOTF-05).
#
# Usage: source this file after core.sh in audit.sh, then call send_notifications
#        at the end of the audit run (after generate_report).
#
# Env vars required at call time:
#   NOTIFY              -- "true" to enable (set by --notify flag in audit.sh)
#   RESULT_DIR          -- path to current run results (audit-summary.json, delta.json)
#   AUDIT_MODE          -- current mode string (full/standard/quick/pre-ship/post-incident)
#   COMMS_PSK           -- pre-shared key for comms-link WS (NEVER hardcoded)
#   COMMS_URL           -- WebSocket URL (e.g. ws://srv1422716.hstgr.cloud:8765)
#   UDAY_WHATSAPP       -- Uday's number with country code (e.g. 919059833001); skip if unset

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
COMMS_LINK_DIR="C:/Users/bono/racingpoint/comms-link"

# ---------------------------------------------------------------------------
# HELPER — _build_summary_text
# Reads audit-summary.json and delta.json from $RESULT_DIR.
# Builds a compact text message suitable for WS, INBOX.md, and WhatsApp.
# Returns text via stdout. Returns 0 always.
# ---------------------------------------------------------------------------
_build_summary_text() {
  local summary_file="${RESULT_DIR:-}/audit-summary.json"
  local delta_file="${RESULT_DIR:-}/delta.json"

  # If no summary file, return minimal fallback immediately
  if [ ! -f "$summary_file" ]; then
    printf 'Fleet Audit (%s) completed. Results in %s' "${AUDIT_MODE:-unknown}" "${RESULT_DIR:-unknown}"
    return 0
  fi

  # Parse audit-summary.json fields
  local verdict pass_count fail_count warn_count quiet_count p1_count p2_count p3_count fix_count
  verdict=$(jq -r '.verdict // "UNKNOWN"' "$summary_file" 2>/dev/null || echo "UNKNOWN")
  pass_count=$(jq -r '.counts.pass // 0' "$summary_file" 2>/dev/null)
  fail_count=$(jq -r '.counts.fail // 0' "$summary_file" 2>/dev/null)
  warn_count=$(jq -r '.counts.warn // 0' "$summary_file" 2>/dev/null)
  quiet_count=$(jq -r '.counts.quiet // 0' "$summary_file" 2>/dev/null)
  p1_count=$(jq -r '.p1_count // 0' "$summary_file" 2>/dev/null)
  p2_count=$(jq -r '.p2_count // 0' "$summary_file" 2>/dev/null)
  p3_count=$(jq -r '.p3_count // 0' "$summary_file" 2>/dev/null)
  fix_count=$(jq -r '.fix_count // 0' "$summary_file" 2>/dev/null)

  # Core summary text
  local text
  text="Fleet Audit (${AUDIT_MODE:-unknown}) -- ${verdict}
Counts: ${pass_count} PASS, ${fail_count} FAIL, ${warn_count} WARN, ${quiet_count} QUIET
Severity: ${p1_count} P1, ${p2_count} P2, ${p3_count} P3
Fixes applied: ${fix_count}"

  # Append delta section if delta.json exists and has_previous is true (NOTF-05)
  if [ -f "$delta_file" ]; then
    local has_previous regressions improvements new_issues
    has_previous=$(jq -r '.has_previous // false' "$delta_file" 2>/dev/null || echo "false")
    if [ "$has_previous" = "true" ]; then
      regressions=$(jq -r '.summary.regressions // 0' "$delta_file" 2>/dev/null)
      improvements=$(jq -r '.summary.improvements // 0' "$delta_file" 2>/dev/null)
      new_issues=$(jq -r '.summary.new_issues // 0' "$delta_file" 2>/dev/null)
      text="${text}
Delta: ${regressions} regressions, ${improvements} improvements, ${new_issues} new issues"
      if [ "${regressions:-0}" -gt 0 ]; then
        text="${text}
REGRESSIONS DETECTED -- review required"
      fi
    fi
  fi

  printf '%s' "$text"
  return 0
}
export -f _build_summary_text

# ---------------------------------------------------------------------------
# CHANNEL 1 — _notify_bono_ws (message)
# Send message to Bono via comms-link WebSocket (send-message.js). (NOTF-01)
# Requires COMMS_PSK and COMMS_URL env vars. Prints warning if not set.
# Wraps execution in subshell with 15s timeout. Non-fatal on any failure.
# ---------------------------------------------------------------------------
_notify_bono_ws() {
  local message="$1"

  if [ -z "${COMMS_PSK:-}" ]; then
    echo "WARN: [notify] COMMS_PSK not set — skipping Bono WS notification" >&2
    return 0
  fi
  if [ -z "${COMMS_URL:-}" ]; then
    echo "WARN: [notify] COMMS_URL not set — skipping Bono WS notification" >&2
    return 0
  fi
  if [ ! -d "$COMMS_LINK_DIR" ]; then
    echo "WARN: [notify] comms-link directory not found at $COMMS_LINK_DIR — skipping WS notification" >&2
    return 0
  fi

  # Run in subshell with timeout; failures are non-fatal
  (
    cd "$COMMS_LINK_DIR" 2>/dev/null || exit 0
    timeout 15 env COMMS_PSK="$COMMS_PSK" COMMS_URL="$COMMS_URL" \
      node send-message.js "$message" 2>/dev/null || true
  ) || true

  return 0
}
export -f _notify_bono_ws

# ---------------------------------------------------------------------------
# CHANNEL 2 — _notify_bono_inbox (message)
# Append message to comms-link INBOX.md with IST timestamp header, then git push. (NOTF-02)
# Standing rule: INBOX.md format is "## YYYY-MM-DD HH:MM IST -- from james (audit)".
# Wraps everything in a subshell. Non-fatal on any failure.
# ---------------------------------------------------------------------------
_notify_bono_inbox() {
  local message="$1"
  local inbox_file="${COMMS_LINK_DIR}/INBOX.md"

  if [ ! -f "$inbox_file" ]; then
    echo "WARN: [notify] INBOX.md not found at $inbox_file — skipping INBOX notification" >&2
    return 0
  fi

  # Run in subshell; failures are non-fatal
  (
    # Generate IST timestamp header
    local ts_header
    ts_header=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')

    # Append to INBOX.md
    {
      printf '\n## %s -- from james (audit)\n' "$ts_header"
      printf '%s\n' "$message"
    } >> "$inbox_file" 2>/dev/null || exit 0

    # Commit and push
    cd "$COMMS_LINK_DIR" 2>/dev/null || exit 0
    git add INBOX.md 2>/dev/null || exit 0
    git commit -m "audit: fleet audit summary (${AUDIT_MODE:-unknown})" 2>/dev/null || exit 0
    git push 2>/dev/null || true
  ) || true

  return 0
}
export -f _notify_bono_inbox

# ---------------------------------------------------------------------------
# CHANNEL 3 — _notify_whatsapp_uday (message)
# Send WhatsApp to Uday via Bono relay Evolution API. (NOTF-03)
# Number read from UDAY_WHATSAPP env var (must include country code 91).
# If UDAY_WHATSAPP is not set, skip with warning (non-fatal).
# Uses comms-link relay exec endpoint. Writes payload to temp file (standing rule).
# ---------------------------------------------------------------------------
_notify_whatsapp_uday() {
  local message="$1"

  if [ -z "${UDAY_WHATSAPP:-}" ]; then
    echo "WARN: [notify] UDAY_WHATSAPP not set — skipping WhatsApp notification to Uday" >&2
    return 0
  fi

  # Run in subshell; failures are non-fatal
  (
    local tmpfile
    tmpfile=$(mktemp) || exit 0

    # Write JSON payload to temp file (bash string escaping safety rule)
    jq -n \
      --arg number "$UDAY_WHATSAPP" \
      --arg message "$message" \
      '{"command":"whatsapp_send","params":{"number":$number,"message":$message}}' \
      > "$tmpfile" 2>/dev/null || { rm -f "$tmpfile"; exit 0; }

    # POST to comms-link relay exec endpoint
    curl -s -m 15 -X POST http://localhost:8766/relay/exec/run \
      -H "Content-Type: application/json" \
      -d @"$tmpfile" 2>/dev/null || true

    rm -f "$tmpfile"
  ) || true

  return 0
}
export -f _notify_whatsapp_uday

# ---------------------------------------------------------------------------
# ENTRY POINT — send_notifications
# Gate: only runs when NOTIFY=true (set by --notify flag in audit.sh). (NOTF-04)
# Builds summary text from result artifacts, then fires all three channels.
# Returns 0 always — notification failures must never affect audit exit code.
# ---------------------------------------------------------------------------
send_notifications() {
  # Off-by-default gate (NOTF-04) — must be first line
  if [[ "${NOTIFY:-false}" != "true" ]]; then return 0; fi

  echo "--- Notifications ---"

  # Build summary text; fallback if empty
  local summary
  summary=$(_build_summary_text 2>/dev/null || true)
  if [ -z "$summary" ]; then
    summary="Fleet Audit (${AUDIT_MODE:-unknown}) completed. Results in ${RESULT_DIR:-unknown}"
  fi

  # Channel 1: Bono WS (NOTF-01)
  _notify_bono_ws "$summary"

  # Channel 2: Bono INBOX.md + git push (NOTF-02)
  _notify_bono_inbox "$summary"

  # Channel 3: WhatsApp to Uday via relay (NOTF-03)
  _notify_whatsapp_uday "$summary"

  echo "--- Notifications Complete ---"

  # ALWAYS return 0 -- notification failures must never affect audit exit code
  return 0
}
export -f send_notifications

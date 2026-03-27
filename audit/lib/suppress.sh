#!/usr/bin/env bash
# audit/lib/suppress.sh — Suppression engine for Racing Point audit framework
#
# Purpose: Known recurring issues (e.g. "Pod 8 display at 1024x768 — waiting for physical
# setup") clutter every audit report. Suppression lets operators acknowledge known issues
# with mandatory expiry dates so they don't get forgotten.
#
# Functions:
#   check_suppression  — per-result check: returns reason if suppressed, 1 if not
#   apply_suppressions — batch rewrite: FAIL/WARN → SUPPRESSED across all phase-*.json
#   get_severity_score — numeric score for priority sorting
#
# All functions follow core.sh style:
#   - local variables only
#   - no set -e
#   - SCRIPT_DIR expected to point to the audit/ directory (set by audit.sh)
#   - exported for use in subshells and background jobs

# ---------------------------------------------------------------------------
# FUNCTION 1 — check_suppression (phase host message)
# Check if a phase result matches any active (non-expired) suppress.json entry.
#
# Matching logic:
#   1. .phase matches the phase arg exactly
#   2. .host_pattern regex matches the host arg (jq test())
#   3. .message_pattern is empty OR matches the message arg (jq test())
#   4. .expires_date >= today's IST date (ISO string comparison)
#
# Returns: prints reason to stdout and returns 0 if suppressed
#          returns 1 (prints nothing) if not suppressed
# ---------------------------------------------------------------------------
check_suppression() {
  local phase=$1 host=$2 message=$3
  local suppress_file="${SCRIPT_DIR:-audit}/suppress.json"

  if [[ ! -f "$suppress_file" ]]; then return 1; fi

  local today
  today=$(TZ=Asia/Kolkata date '+%Y-%m-%d')

  # Pre-validate: filter out entries with invalid expires_date (must be YYYY-MM-DD)
  local reason
  reason=$(jq -r \
    --arg phase   "$phase"   \
    --arg host    "$host"    \
    --arg message "$message" \
    --arg today   "$today"   \
    '
      .[] |
      select(.phase == $phase) |
      select(.expires_date | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}$")) |
      select(.expires_date >= $today) |
      select(.host_pattern as $pat | $host | test($pat)) |
      select(
        .message_pattern == "" or
        .message_pattern == null or
        (.message_pattern as $mpat | $message | test($mpat))
      ) |
      .reason
    ' "$suppress_file" 2>/dev/null | head -1)

  if [[ -n "$reason" ]]; then
    printf '%s' "$reason"
    return 0
  fi
  return 1
}
export -f check_suppression

# ---------------------------------------------------------------------------
# FUNCTION 2 — apply_suppressions
# Batch process all phase-*.json files in $RESULT_DIR.
# For each file with status FAIL or WARN:
#   - Calls check_suppression with the phase, host, and message fields
#   - If suppressed: rewrites the JSON with status=SUPPRESSED and suppression_reason added
#   - If not suppressed: leaves the file unchanged
# Prints summary line: "Suppressed: N phase(s)"
# ---------------------------------------------------------------------------
apply_suppressions() {
  local result_dir="${RESULT_DIR:-/tmp/audit-fallback}"
  local suppress_count=0

  if [[ ! -d "$result_dir" ]]; then
    printf 'Suppressed: 0 phase(s)\n'
    return 0
  fi

  local file phase host status message reason
  for file in "$result_dir"/phase-*.json; do
    [[ -f "$file" ]] || continue

    status=$(jq -r '.status // ""' "$file" 2>/dev/null)
    # Only process FAIL or WARN results
    if [[ "$status" != "FAIL" && "$status" != "WARN" ]]; then continue; fi

    phase=$(jq -r '.phase // ""' "$file" 2>/dev/null)
    host=$(jq -r '.host // ""' "$file" 2>/dev/null)
    message=$(jq -r '.message // ""' "$file" 2>/dev/null)

    reason=$(check_suppression "$phase" "$host" "$message")
    if [[ $? -eq 0 && -n "$reason" ]]; then
      # Rewrite JSON: update status to SUPPRESSED, add suppression_reason field
      local tmpfile; tmpfile=$(mktemp)
      jq --arg reason "$reason" \
        '. + {status: "SUPPRESSED", suppression_reason: $reason}' \
        "$file" > "$tmpfile" && mv "$tmpfile" "$file"
      suppress_count=$(( suppress_count + 1 ))
    fi
  done

  printf 'Suppressed: %d phase(s)\n' "$suppress_count"
}
export -f apply_suppressions

# ---------------------------------------------------------------------------
# FUNCTION 3 — get_severity_score (status severity)
# Returns numeric priority score for sorting/prioritization.
# Higher score = higher priority (needs attention first).
#
# Scores:
#   FAIL  + P1 = 100   (service down — critical)
#   FAIL  + P2 = 80    (degraded — high)
#   FAIL  + P3 = 60    (informational fail — medium)
#   WARN  + P1 = 50    (degraded warning — high)
#   WARN  + P2 = 40    (degraded warning — medium)
#   WARN  + P3 = 30    (informational warning — low)
#   SUPPRESSED = 10    (known issue, acknowledged)
#   QUIET  = 5         (venue-closed, not applicable)
#   PASS   = 0         (healthy)
#   (unknown) = 0
# ---------------------------------------------------------------------------
get_severity_score() {
  local status=$1 severity=$2

  case "$status" in
    FAIL)
      case "$severity" in
        P1) printf '100' ;;
        P2) printf '80'  ;;
        P3) printf '60'  ;;
        *)  printf '60'  ;;
      esac
      ;;
    WARN)
      case "$severity" in
        P1) printf '50' ;;
        P2) printf '40' ;;
        P3) printf '30' ;;
        *)  printf '30' ;;
      esac
      ;;
    SUPPRESSED) printf '10' ;;
    QUIET)      printf '5'  ;;
    PASS)       printf '0'  ;;
    *)          printf '0'  ;;
  esac
  return 0
}
export -f get_severity_score

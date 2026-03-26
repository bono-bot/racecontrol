#!/usr/bin/env bash
# scripts/intelligence/pattern-tracker.sh — LEARN-01: Pattern Database
#
# Logs each auto-detect finding to suggestions.jsonl for self-improving loop.
# Every run that finds bugs grows suggestions.jsonl by N entries (one per finding).
#
# Designed to be sourced into auto-detect.sh. Exports: update_pattern_db
#
# Dependencies (inherited from auto-detect.sh context):
#   REPO_ROOT       — repo root path
#   RESULT_DIR      — per-run results directory
#   TIMESTAMP       — current run timestamp string

set -uo pipefail
# NO set -e — errors are encoded in JSONL, never propagated

# ─── Source guard ────────────────────────────────────────────────────────────
[[ "${_PATTERN_TRACKER_SOURCED:-}" == "1" ]] && return 0
_PATTERN_TRACKER_SOURCED=1

# ─── Constants ───────────────────────────────────────────────────────────────
# SUGGESTIONS_JSONL is written here; trend-analyzer reads from the same path
SUGGESTIONS_JSONL="${REPO_ROOT:-/tmp}/audit/results/suggestions.jsonl"

# ─── update_pattern_db ───────────────────────────────────────────────────────
# Called as post-report step in generate_report_and_notify() after every run.
#
# For each finding in findings.json, appends one JSONL entry to suggestions.jsonl:
#   run_ts       — TIMESTAMP from env
#   bug_type     — finding.issue_type (falls back to finding.category)
#   pod_ip       — finding.pod_ip
#   severity     — finding.severity
#   fix_applied  — true if pod_ip appears as host in fixes.jsonl for same run
#   fix_success  — true if fix_applied AND after_state has no failure indicators
#   frequency    — 1 (single-run increment; trend-analyzer accumulates across runs)
#   source       — "auto_detect"
#
# Degrades gracefully: missing findings.json, unset RESULT_DIR, or jq failure
# all return 0 without propagating errors.
update_pattern_db() {
  local findings_file="${RESULT_DIR:-}/findings.json"
  local fixes_file="${RESULT_DIR:-}/fixes.jsonl"
  local run_ts="${TIMESTAMP:-$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')}"

  # Guard: RESULT_DIR must be set and findings.json must exist
  if [[ -z "${RESULT_DIR:-}" ]]; then
    log WARN "[LEARN-01] pattern_db update skipped: RESULT_DIR unset" 2>/dev/null || true
    return 0
  fi

  if [[ ! -f "$findings_file" ]]; then
    log INFO "[LEARN-01] pattern_db update skipped: no findings.json in $RESULT_DIR" 2>/dev/null || true
    return 0
  fi

  # Ensure suggestions.jsonl directory exists
  local suggestions_dir
  suggestions_dir="$(dirname "$SUGGESTIONS_JSONL")"
  mkdir -p "$suggestions_dir" 2>/dev/null || true

  # Read fixes.jsonl into a variable for pod lookup (degrade gracefully if absent)
  local fixes_json="[]"
  if [[ -f "$fixes_file" ]]; then
    fixes_json=$(jq -sc '.' "$fixes_file" 2>/dev/null || echo "[]")
  fi

  # Count findings processed for log message
  local n_entries=0

  # Process each finding from findings.json
  local findings_array
  findings_array=$(jq -c '.[]' "$findings_file" 2>/dev/null)
  if [[ $? -ne 0 ]]; then
    log WARN "[LEARN-01] pattern_db update failed: jq error reading findings.json" 2>/dev/null || true
    return 0
  fi

  while IFS= read -r finding; do
    [[ -z "$finding" ]] && continue

    # Extract fields from finding
    local bug_type pod_ip severity
    bug_type=$(echo "$finding" | jq -r '.issue_type // .category // "unknown"' 2>/dev/null || echo "unknown")
    pod_ip=$(echo "$finding" | jq -r '.pod_ip // "unknown"' 2>/dev/null || echo "unknown")
    severity=$(echo "$finding" | jq -r '.severity // "P2"' 2>/dev/null || echo "P2")

    # Determine fix_applied: true if pod_ip appears as host in fixes.jsonl
    local fix_applied="false"
    local fix_match
    fix_match=$(echo "$fixes_json" | jq -r --arg ip "$pod_ip" \
      '[.[] | select(.host == $ip)] | length' 2>/dev/null || echo "0")
    if [[ "$fix_match" -gt 0 ]]; then
      fix_applied="true"
    fi

    # Determine fix_success: fix_applied AND no failure indicators in after_state
    local fix_success="false"
    if [[ "$fix_applied" == "true" ]]; then
      local failure_check
      # after_state must NOT contain: FAIL, blocked, skipped, UNRESOLVED
      failure_check=$(echo "$fixes_json" | jq -r --arg ip "$pod_ip" \
        '[.[] | select(.host == $ip) | .after // "" | test("FAIL|blocked|skipped|UNRESOLVED")] | any' \
        2>/dev/null || echo "true")
      if [[ "$failure_check" == "false" ]]; then
        fix_success="true"
      fi
    fi

    # Write one JSONL entry to suggestions.jsonl
    local entry
    entry=$(jq -n \
      --arg run_ts     "$run_ts"     \
      --arg bug_type   "$bug_type"   \
      --arg pod_ip     "$pod_ip"     \
      --arg severity   "$severity"   \
      --argjson fix_applied  "$fix_applied"  \
      --argjson fix_success  "$fix_success"  \
      --argjson frequency    1               \
      --arg source     "auto_detect"         \
      '{run_ts:$run_ts,bug_type:$bug_type,pod_ip:$pod_ip,severity:$severity,fix_applied:$fix_applied,fix_success:$fix_success,frequency:$frequency,source:$source}' \
      2>/dev/null)

    if [[ $? -ne 0 ]] || [[ -z "$entry" ]]; then
      log WARN "[LEARN-01] pattern_db update failed: jq error building entry for $pod_ip" 2>/dev/null || true
      continue
    fi

    echo "$entry" >> "$SUGGESTIONS_JSONL"
    n_entries=$((n_entries + 1))

  done <<< "$findings_array"

  log INFO "[LEARN-01] pattern_db updated: $n_entries entries written" 2>/dev/null || true
  return 0
}

export -f update_pattern_db

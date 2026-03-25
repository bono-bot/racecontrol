#!/usr/bin/env bash
# audit/lib/results.sh — Results storage, index management, and delta detection
#
# Provides structured storage for audit run results:
#   - finalize_results: count PASS/WARN/FAIL/QUIET per run, write to run-meta.json, update index
#   - update_index:     append run entry to results/index.json atomically
#   - find_previous_run: return result_dir of the second-most-recent run for delta comparison
#
# All timestamps: IST (UTC+5:30) via ist_now() from core.sh
# Requires: RESULT_DIR, SCRIPT_DIR, AUDIT_MODE, VENUE_STATE to be exported by audit.sh
#
# Usage: source this file after core.sh and parallel.sh in audit.sh

# ---------------------------------------------------------------------------
# FUNCTION 1 — finalize_results
# Count PASS/WARN/FAIL/QUIET from phase JSON files, merge into run-meta.json,
# then call update_index to append the run to results/index.json.
# Called after all phases complete, before exit-code counting.
# ---------------------------------------------------------------------------
finalize_results() {
  local result_dir="${RESULT_DIR:-}"
  if [ -z "$result_dir" ] || [ ! -d "$result_dir" ]; then
    return 0
  fi

  local pass_count=0 warn_count=0 fail_count=0 quiet_count=0 total=0

  for f in "$result_dir"/phase-*.json; do
    [ -f "$f" ] || continue
    local status
    status=$(jq -r '.status // "UNKNOWN"' "$f" 2>/dev/null || echo "UNKNOWN")
    case "$status" in
      PASS)  pass_count=$((pass_count+1)) ;;
      WARN)  warn_count=$((warn_count+1)) ;;
      FAIL)  fail_count=$((fail_count+1)) ;;
      QUIET) quiet_count=$((quiet_count+1)) ;;
    esac
    total=$((total+1))
  done

  local completed_at
  completed_at=$(ist_now 2>/dev/null || TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30')

  local meta_file="$result_dir/run-meta.json"
  local existing_meta="{}"
  if [ -f "$meta_file" ]; then
    existing_meta=$(cat "$meta_file")
  fi

  # Merge counts and completed_at into existing run-meta.json
  local updated_meta
  updated_meta=$(printf '%s' "$existing_meta" | jq \
    --arg completed_at "$completed_at" \
    --arg venue_state "${VENUE_STATE:-unknown}" \
    --argjson pass "$pass_count" \
    --argjson warn "$warn_count" \
    --argjson fail "$fail_count" \
    --argjson quiet "$quiet_count" \
    --argjson total "$total" \
    '. + {
      completed_at: $completed_at,
      venue_state: $venue_state,
      counts: {
        pass: $pass,
        warn: $warn,
        fail: $fail,
        quiet: $quiet,
        total: $total
      }
    }' 2>/dev/null)

  if [ -n "$updated_meta" ]; then
    printf '%s' "$updated_meta" > "$meta_file"
  fi

  # Append run to index
  update_index "$pass_count" "$warn_count" "$fail_count" "$quiet_count" "$total"
}
export -f finalize_results

# ---------------------------------------------------------------------------
# FUNCTION 2 — update_index (pass warn fail quiet total)
# Append one run entry to results/index.json.
# Writes atomically: jq to temp file, then mv to prevent corruption on ctrl-C.
# Index path: ${SCRIPT_DIR}/results/index.json
# ---------------------------------------------------------------------------
update_index() {
  local pass_count="${1:-0}" warn_count="${2:-0}" fail_count="${3:-0}" quiet_count="${4:-0}" total="${5:-0}"
  local index_file="${SCRIPT_DIR}/results/index.json"

  # Create empty array if index doesn't exist
  local current_index="[]"
  if [ -f "$index_file" ]; then
    current_index=$(cat "$index_file" 2>/dev/null || echo "[]")
    # Validate it's a JSON array; reset if not
    if ! printf '%s' "$current_index" | jq -e 'type == "array"' >/dev/null 2>&1; then
      current_index="[]"
    fi
  fi

  local ts
  ts=$(ist_now 2>/dev/null || TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30')

  local new_entry
  new_entry=$(jq -n \
    --arg timestamp "$ts" \
    --arg mode "${AUDIT_MODE:-unknown}" \
    --arg result_dir "${RESULT_DIR:-}" \
    --arg venue_state "${VENUE_STATE:-unknown}" \
    --argjson pass "$pass_count" \
    --argjson warn "$warn_count" \
    --argjson fail "$fail_count" \
    --argjson quiet "$quiet_count" \
    --argjson total "$total" \
    '{
      timestamp: $timestamp,
      mode: $mode,
      result_dir: $result_dir,
      venue_state: $venue_state,
      counts: {
        pass: $pass,
        warn: $warn,
        fail: $fail,
        quiet: $quiet,
        total: $total
      }
    }' 2>/dev/null)

  if [ -z "$new_entry" ]; then
    return 0
  fi

  # Append entry to index array, write atomically
  local tmp_index
  tmp_index=$(mktemp)
  printf '%s' "$current_index" | jq --argjson entry "$new_entry" '. + [$entry]' > "$tmp_index" 2>/dev/null
  if [ $? -eq 0 ] && [ -s "$tmp_index" ]; then
    mv "$tmp_index" "$index_file"
  else
    rm -f "$tmp_index"
  fi
}
export -f update_index

# ---------------------------------------------------------------------------
# FUNCTION 3 — find_previous_run
# Returns the result_dir of the second-to-last index entry for delta comparison.
# Returns empty string if fewer than 2 entries exist (no previous run).
# Callers: PREV_DIR=$(find_previous_run)
# ---------------------------------------------------------------------------
find_previous_run() {
  local index_file="${SCRIPT_DIR}/results/index.json"
  if [ ! -f "$index_file" ]; then
    printf ''
    return 0
  fi
  local prev_dir
  prev_dir=$(jq -r '.[-2].result_dir // empty' "$index_file" 2>/dev/null || printf '')
  printf '%s' "$prev_dir"
}
export -f find_previous_run

#!/usr/bin/env bash
# scripts/intelligence/trend-analyzer.sh — LEARN-04: Trend Outlier Analysis
#
# Reads suggestions.jsonl written by pattern-tracker.sh and flags pods that have
# significantly more occurrences of a bug_type than the fleet average.
#
# Outlier definition: pod_count > fleet_avg * OUTLIER_THRESHOLD (default 4.0)
# The threshold is configurable via auto-detect-config.json "trend_outlier_multiplier".
#
# Designed to be sourced into auto-detect.sh. Exports: run_trend_analysis
#
# Dependencies (inherited from auto-detect.sh context):
#   REPO_ROOT       — repo root path
#   TIMESTAMP       — current run timestamp string
#   AUTO_DETECT_CONFIG (optional) — path to auto-detect-config.json

set -uo pipefail
# NO set -e — errors are encoded in JSONL, never propagated

# ─── Source guard ────────────────────────────────────────────────────────────
[[ "${_TREND_ANALYZER_SOURCED:-}" == "1" ]] && return 0
_TREND_ANALYZER_SOURCED=1

# ─── Constants ───────────────────────────────────────────────────────────────
# Same path used by pattern-tracker.sh — both read and write here
SUGGESTIONS_JSONL="${REPO_ROOT:-/tmp}/audit/results/suggestions.jsonl"

# Minimum entries needed before statistical analysis is meaningful
_TREND_MIN_ENTRIES=10

# ─── run_trend_analysis ──────────────────────────────────────────────────────
# Called as post-report step in generate_report_and_notify() AFTER update_pattern_db.
#
# Per-bug-type analysis:
#   1. Read all entries from suggestions.jsonl
#   2. For each unique bug_type: count occurrences per pod_ip, compute fleet_avg
#   3. Flag pod where pod_count > fleet_avg * OUTLIER_THRESHOLD
#   4. Append TREND_OUTLIER entry to suggestions.jsonl for each flagged pod
#
# Degrades gracefully: insufficient data, missing file, or jq failure all return 0.
run_trend_analysis() {
  local run_ts="${TIMESTAMP:-$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')}"

  # Guard: suggestions.jsonl must exist
  if [[ ! -f "$SUGGESTIONS_JSONL" ]]; then
    log INFO "[LEARN-04] trend analysis skipped: suggestions.jsonl not found" 2>/dev/null || true
    return 0
  fi

  # Guard: minimum statistical sample required
  local entry_count
  entry_count=$(wc -l < "$SUGGESTIONS_JSONL" 2>/dev/null || echo "0")
  entry_count="${entry_count// /}"  # trim whitespace

  if [[ "$entry_count" -lt "$_TREND_MIN_ENTRIES" ]]; then
    log INFO "[LEARN-04] trend analysis skipped: insufficient data ($entry_count entries, need $_TREND_MIN_ENTRIES)" 2>/dev/null || true
    return 0
  fi

  # Read configurable outlier threshold from auto-detect-config.json
  local config_file="${AUTO_DETECT_CONFIG:-${REPO_ROOT:-}/audit/results/auto-detect-config.json}"
  local outlier_threshold="4.0"
  if [[ -f "$config_file" ]]; then
    local cfg_val
    cfg_val=$(jq -r '.trend_outlier_multiplier // empty' "$config_file" 2>/dev/null || true)
    if [[ -n "${cfg_val:-}" ]]; then
      outlier_threshold="$cfg_val"
    fi
  fi

  # Slurp all entries (excluding TREND_OUTLIER entries themselves to avoid feedback loop)
  local all_entries
  all_entries=$(jq -sc '[.[] | select(.entry_type != "TREND_OUTLIER")]' "$SUGGESTIONS_JSONL" 2>/dev/null)
  if [[ $? -ne 0 ]] || [[ -z "$all_entries" ]] || [[ "$all_entries" == "[]" ]]; then
    log WARN "[LEARN-04] trend analysis failed: jq error reading suggestions.jsonl" 2>/dev/null || true
    return 0
  fi

  # Get unique bug_types
  local bug_types
  bug_types=$(echo "$all_entries" | jq -r '[.[].bug_type] | unique | .[]' 2>/dev/null || echo "")
  if [[ -z "$bug_types" ]]; then
    log INFO "[LEARN-04] trend analysis: no bug_types found in data" 2>/dev/null || true
    return 0
  fi

  local total_outliers=0
  local total_bug_types=0

  while IFS= read -r bug_type; do
    [[ -z "$bug_type" ]] && continue
    total_bug_types=$((total_bug_types + 1))

    # For this bug_type: build per-pod counts and fleet average
    # pod_counts: object {pod_ip: count}
    # fleet_avg: total_count / unique_pod_count
    local analysis
    analysis=$(echo "$all_entries" | jq -r \
      --arg bt "$bug_type" \
      --arg threshold "$outlier_threshold" '
      [.[] | select(.bug_type == $bt)] |
      group_by(.pod_ip) |
      map({pod_ip: .[0].pod_ip, pod_count: length}) |
      . as $pod_data |
      (map(.pod_count) | add) as $total |
      (length) as $unique_pods |
      if $unique_pods == 0 then empty
      else
        ($total / $unique_pods) as $fleet_avg |
        ($fleet_avg * ($threshold | tonumber)) as $outlier_threshold_val |
        $pod_data |
        map(select(.pod_count > $outlier_threshold_val)) |
        map(. + {fleet_avg: ($fleet_avg | (. * 100 | round) / 100), threshold_used: ($threshold | tonumber)})
      end
    ' 2>/dev/null || echo "[]")

    if [[ $? -ne 0 ]] || [[ -z "$analysis" ]] || [[ "$analysis" == "[]" ]] || [[ "$analysis" == "null" ]]; then
      continue
    fi

    # For each outlier pod, write a TREND_OUTLIER entry
    local outlier_count
    outlier_count=$(echo "$analysis" | jq 'length' 2>/dev/null || echo "0")
    if [[ "$outlier_count" -eq 0 ]]; then
      continue
    fi

    # Process each outlier
    while IFS= read -r outlier; do
      [[ -z "$outlier" ]] && continue

      local outlier_pod_ip outlier_pod_count outlier_fleet_avg outlier_multiplier
      outlier_pod_ip=$(echo "$outlier" | jq -r '.pod_ip' 2>/dev/null || echo "unknown")
      outlier_pod_count=$(echo "$outlier" | jq -r '.pod_count' 2>/dev/null || echo "0")
      outlier_fleet_avg=$(echo "$outlier" | jq -r '.fleet_avg' 2>/dev/null || echo "0")

      # Compute multiplier = pod_count / fleet_avg (round to 2 decimal places)
      # Use jq for float division to avoid bash float issues
      outlier_multiplier=$(jq -n \
        --argjson pc "$outlier_pod_count" \
        --argjson fa "$outlier_fleet_avg" \
        'if $fa == 0 then 0 else ($pc / $fa * 100 | round) / 100 end' \
        2>/dev/null || echo "0")

      # Write TREND_OUTLIER entry
      local entry
      entry=$(jq -n \
        --arg run_ts           "$run_ts"            \
        --arg entry_type       "TREND_OUTLIER"       \
        --arg bug_type         "$bug_type"           \
        --arg pod_ip           "$outlier_pod_ip"     \
        --argjson pod_count    "$outlier_pod_count"  \
        --argjson fleet_avg    "$outlier_fleet_avg"  \
        --argjson multiplier   "$outlier_multiplier" \
        --argjson threshold_used "$outlier_threshold" \
        --arg source           "trend_analyzer"      \
        '{run_ts:$run_ts,entry_type:$entry_type,bug_type:$bug_type,pod_ip:$pod_ip,pod_count:$pod_count,fleet_avg:$fleet_avg,multiplier:$multiplier,threshold_used:$threshold_used,source:$source}' \
        2>/dev/null)

      if [[ $? -ne 0 ]] || [[ -z "$entry" ]]; then
        log WARN "[LEARN-04] failed to build TREND_OUTLIER entry for $outlier_pod_ip/$bug_type" 2>/dev/null || true
        continue
      fi

      echo "$entry" >> "$SUGGESTIONS_JSONL"
      total_outliers=$((total_outliers + 1))

    done <<< "$(echo "$analysis" | jq -c '.[]' 2>/dev/null || echo "")"

  done <<< "$bug_types"

  log INFO "[LEARN-04] trend analysis: $total_outliers outliers flagged across $total_bug_types bug types" 2>/dev/null || true
  return 0
}

export -f run_trend_analysis

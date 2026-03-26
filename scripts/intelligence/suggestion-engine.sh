#!/usr/bin/env bash
# scripts/intelligence/suggestion-engine.sh — LEARN-02/03/06: Suggestion Engine
#
# Analyzes accumulated patterns in suggestions.jsonl and generates structured
# improvement proposals. Also exports get_suggestions_json() for relay exec queries.
#
# Designed to be sourced into auto-detect.sh. Exports: run_suggestion_engine, get_suggestions_json
#
# Dependencies (inherited from auto-detect.sh context):
#   REPO_ROOT       — repo root path

set -uo pipefail
# NO set -e — all errors return 0 (non-fatal)

# ─── Source guard ─────────────────────────────────────────────────────────────
[[ "${_SUGGESTION_ENGINE_SOURCED:-}" == "1" ]] && return 0
_SUGGESTION_ENGINE_SOURCED=1

# ─── Constants ────────────────────────────────────────────────────────────────
SUGGESTIONS_JSONL="${REPO_ROOT:-/tmp}/audit/results/suggestions.jsonl"
PROPOSALS_DIR="${REPO_ROOT:-/tmp}/audit/results/proposals"

# ─── run_suggestion_engine ────────────────────────────────────────────────────
# Called as post-trend-analysis step in generate_report_and_notify().
#
# Reads suggestions.jsonl, groups by bug_type+pod_ip, generates structured
# proposal JSON files in PROPOSALS_DIR for each pair exceeding MIN_FREQUENCY.
#
# TREND_OUTLIER entries are processed separately as threshold_tune proposals.
# Deduplicates against existing PENDING proposals to avoid duplicates.
#
# Output: one JSON file per proposal in PROPOSALS_DIR/
#   Fields: id, category, bug_type, pod_ip, confidence, evidence, status,
#           created_ts, total_count, fix_success_rate
run_suggestion_engine() {
  # Guard: SUGGESTIONS_JSONL must exist
  if [[ ! -f "$SUGGESTIONS_JSONL" ]]; then
    log INFO "[LEARN-02] suggestion engine skipped: no suggestions.jsonl" 2>/dev/null || true
    return 0
  fi

  mkdir -p "$PROPOSALS_DIR" 2>/dev/null || true

  # Read MIN_FREQUENCY from auto-detect-config.json (default 3)
  local auto_detect_config="${REPO_ROOT:-/tmp}/audit/results/auto-detect-config.json"
  local min_frequency=3
  if [[ -f "$auto_detect_config" ]]; then
    local cfg_val
    cfg_val=$(jq -r '.suggestion_min_frequency // 3' "$auto_detect_config" 2>/dev/null || echo "3")
    if [[ "$cfg_val" =~ ^[0-9]+$ ]]; then
      min_frequency="$cfg_val"
    fi
  fi

  local n_written=0
  local n_skipped=0

  # ── Process regular (non-TREND_OUTLIER) entries ──────────────────────────────
  # Load all non-TREND_OUTLIER entries from SUGGESTIONS_JSONL using jq
  local regular_entries
  regular_entries=$(jq -Rsc '
    split("\n")
    | map(select(length > 0))
    | map(. as $line | try fromjson catch null)
    | map(select(. != null and .entry_type != "TREND_OUTLIER"))
  ' "$SUGGESTIONS_JSONL" 2>/dev/null || echo "[]")

  # Aggregate by bug_type + pod_ip
  local aggregated
  aggregated=$(echo "$regular_entries" | jq -c '
    group_by(.bug_type + "|||" + .pod_ip)
    | map({
        bug_type: .[0].bug_type,
        pod_ip: .[0].pod_ip,
        total_count: length,
        fix_applied_count: (map(select(.fix_applied == true)) | length),
        fix_success_count: (map(select(.fix_success == true)) | length)
      })
  ' 2>/dev/null || echo "[]")

  # Process each aggregated group
  local group
  while IFS= read -r group; do
    [[ -z "$group" ]] && continue

    local bug_type pod_ip total_count fix_applied_count fix_success_count
    bug_type=$(echo "$group" | jq -r '.bug_type' 2>/dev/null || echo "unknown")
    pod_ip=$(echo "$group" | jq -r '.pod_ip' 2>/dev/null || echo "unknown")
    total_count=$(echo "$group" | jq -r '.total_count' 2>/dev/null || echo "0")
    fix_applied_count=$(echo "$group" | jq -r '.fix_applied_count' 2>/dev/null || echo "0")
    fix_success_count=$(echo "$group" | jq -r '.fix_success_count' 2>/dev/null || echo "0")

    # Only propose if frequency threshold met
    if [[ "$total_count" -lt "$min_frequency" ]]; then
      continue
    fi

    # Check for existing pending proposal covering same bug_type + pod_ip
    local existing_pending=0
    if ls "$PROPOSALS_DIR"/*.json 2>/dev/null | head -1 | grep -q "."; then
      # Scan existing proposals for status==pending AND bug_type match
      local existing_check
      existing_check=$(jq -r --arg bt "$bug_type" --arg ip "$pod_ip" \
        'select(.status == "pending" and .bug_type == $bt and .pod_ip == $ip) | "found"' \
        "$PROPOSALS_DIR"/*.json 2>/dev/null | head -1 || echo "")
      if [[ "$existing_check" == "found" ]]; then
        n_skipped=$((n_skipped + 1))
        continue
      fi
    fi

    # Compute rates (avoid division by zero)
    local fix_applied_rate fix_success_rate
    if [[ "$total_count" -gt 0 ]]; then
      fix_applied_rate=$(echo "scale=4; $fix_applied_count / $total_count" | bc 2>/dev/null || echo "0")
      if [[ "$fix_applied_count" -gt 0 ]]; then
        fix_success_rate=$(echo "scale=4; $fix_success_count / $fix_applied_count" | bc 2>/dev/null || echo "0")
      else
        fix_success_rate="0"
      fi
    else
      fix_applied_rate="0"
      fix_success_rate="0"
    fi

    # Determine category based on mapping rules
    local category
    # Rule 1: fix attempted but failing
    if awk "BEGIN {exit !($fix_success_rate < 0.5 && $fix_applied_rate > 0.5)}" 2>/dev/null; then
      category="new_autofix_candidate"
    # Rule 2: never attempted
    elif [[ "$fix_applied_count" -eq 0 ]]; then
      category="new_autofix_candidate"
    # Rule 3: drift/mismatch/desync patterns → cascade coverage gap
    elif echo "$bug_type" | grep -qi "drift\|mismatch\|desync"; then
      category="cascade_coverage_gap"
    # Rule 4: threshold/anomaly/rate patterns → threshold tune
    elif echo "$bug_type" | grep -qi "threshold\|anomaly\|rate"; then
      category="threshold_tune"
    # Default: repeated unfixed issue → standing rule gap
    else
      category="standing_rule_gap"
    fi

    # Compute confidence score: min(1.0, total_count / 10.0), rounded to 2 decimals
    local confidence
    confidence=$(awk "BEGIN {
      c = $total_count / 10.0
      if (c > 1.0) c = 1.0
      printf \"%.2f\", c
    }" 2>/dev/null || echo "0.30")

    # Build evidence string
    local evidence
    evidence="Seen ${total_count}x on pod ${pod_ip}. Fix applied: ${fix_applied_count}x. Fix succeeded: ${fix_success_count}x."

    # Build fix_success_rate rounded to 2 decimals for proposal field
    local fix_success_rate_rounded
    fix_success_rate_rounded=$(awk "BEGIN { printf \"%.2f\", $fix_success_rate }" 2>/dev/null || echo "0.00")

    # Write proposal file
    local created_ts
    created_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M IST')
    local epoch
    epoch=$(date +%s)
    local bug_type_slug="${bug_type//\//_}"
    local proposal_id="${epoch}_${category}_${bug_type_slug}"
    local proposal_file="$PROPOSALS_DIR/${proposal_id}.json"

    jq -n \
      --arg id              "$proposal_id"             \
      --arg category        "$category"                \
      --arg bug_type        "$bug_type"                \
      --arg pod_ip          "$pod_ip"                  \
      --argjson confidence  "$confidence"              \
      --arg evidence        "$evidence"                \
      --arg status          "pending"                  \
      --arg created_ts      "$created_ts"              \
      --argjson total_count "$total_count"             \
      --argjson fix_success_rate "$fix_success_rate_rounded" \
      '{id:$id,category:$category,bug_type:$bug_type,pod_ip:$pod_ip,confidence:$confidence,evidence:$evidence,status:$status,created_ts:$created_ts,total_count:$total_count,fix_success_rate:$fix_success_rate}' \
      > "$proposal_file" 2>/dev/null

    if [[ $? -eq 0 ]]; then
      n_written=$((n_written + 1))
    fi

  done < <(echo "$aggregated" | jq -c '.[]' 2>/dev/null)

  # ── Process TREND_OUTLIER entries ─────────────────────────────────────────────
  local trend_entries
  trend_entries=$(jq -Rsc '
    split("\n")
    | map(select(length > 0))
    | map(. as $line | try fromjson catch null)
    | map(select(. != null and .entry_type == "TREND_OUTLIER"))
  ' "$SUGGESTIONS_JSONL" 2>/dev/null || echo "[]")

  local trend_entry
  while IFS= read -r trend_entry; do
    [[ -z "$trend_entry" ]] && continue

    local t_bug_type t_pod_ip t_pod_count t_fleet_avg t_multiplier
    t_bug_type=$(echo "$trend_entry" | jq -r '.bug_type // "unknown"' 2>/dev/null || echo "unknown")
    t_pod_ip=$(echo "$trend_entry" | jq -r '.pod_ip // "unknown"' 2>/dev/null || echo "unknown")
    t_pod_count=$(echo "$trend_entry" | jq -r '.pod_count // 0' 2>/dev/null || echo "0")
    t_fleet_avg=$(echo "$trend_entry" | jq -r '.fleet_avg // 0' 2>/dev/null || echo "0")
    t_multiplier=$(echo "$trend_entry" | jq -r '.multiplier // 0' 2>/dev/null || echo "0")

    # Check for existing pending proposal covering same bug_type + pod_ip
    if ls "$PROPOSALS_DIR"/*.json 2>/dev/null | head -1 | grep -q "."; then
      local t_existing
      t_existing=$(jq -r --arg bt "$t_bug_type" --arg ip "$t_pod_ip" \
        'select(.status == "pending" and .bug_type == $bt and .pod_ip == $ip) | "found"' \
        "$PROPOSALS_DIR"/*.json 2>/dev/null | head -1 || echo "")
      if [[ "$t_existing" == "found" ]]; then
        n_skipped=$((n_skipped + 1))
        continue
      fi
    fi

    # Build evidence for trend outlier
    local t_evidence
    t_evidence="Pod ${t_pod_ip} has ${t_pod_count} occurrences vs fleet avg ${t_fleet_avg} (${t_multiplier}x threshold)."

    local t_created_ts
    t_created_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M IST')
    local t_epoch
    t_epoch=$(date +%s)
    local t_slug="${t_bug_type//\//_}"
    local t_proposal_id="${t_epoch}_threshold_tune_${t_slug}"
    local t_proposal_file="$PROPOSALS_DIR/${t_proposal_id}.json"

    jq -n \
      --arg id          "$t_proposal_id"   \
      --arg category    "threshold_tune"   \
      --arg bug_type    "$t_bug_type"      \
      --arg pod_ip      "$t_pod_ip"        \
      --argjson confidence 0.50            \
      --arg evidence    "$t_evidence"      \
      --arg status      "pending"          \
      --arg created_ts  "$t_created_ts"    \
      --argjson total_count "$t_pod_count" \
      --argjson fix_success_rate 0.00      \
      '{id:$id,category:$category,bug_type:$bug_type,pod_ip:$pod_ip,confidence:$confidence,evidence:$evidence,status:$status,created_ts:$created_ts,total_count:$total_count,fix_success_rate:$fix_success_rate}' \
      > "$t_proposal_file" 2>/dev/null

    if [[ $? -eq 0 ]]; then
      n_written=$((n_written + 1))
    fi

  done < <(echo "$trend_entries" | jq -c '.[]' 2>/dev/null)

  log INFO "[LEARN-02] suggestion engine: ${n_written} proposals written, ${n_skipped} skipped (existing pending)" 2>/dev/null || true
  return 0
}

export -f run_suggestion_engine

# ─── get_suggestions_json ─────────────────────────────────────────────────────
# Returns all proposals from PROPOSALS_DIR as a JSON array sorted by confidence DESC.
# Returns [] if no proposals or directory missing.
# Used by relay exec 'get_suggestions' command.
get_suggestions_json() {
  if [[ ! -d "$PROPOSALS_DIR" ]]; then
    echo "[]"
    return 0
  fi

  local proposal_files
  proposal_files=$(ls "$PROPOSALS_DIR"/*.json 2>/dev/null || true)

  if [[ -z "$proposal_files" ]]; then
    echo "[]"
    return 0
  fi

  # Read all proposal files and merge into sorted array
  jq -s 'sort_by(-.confidence)' $PROPOSALS_DIR/*.json 2>/dev/null || echo "[]"

  return 0
}

export -f get_suggestions_json

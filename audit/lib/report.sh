#!/usr/bin/env bash
# audit/lib/report.sh — Report generation for Racing Point audit framework
#
# Produces two output files per run:
#   - ${RESULT_DIR}/audit-report.md   — Human-readable Markdown report
#   - ${RESULT_DIR}/audit-summary.json — Machine-readable JSON counts + verdict
#
# Depends on: RESULT_DIR, AUDIT_MODE, VENUE_STATE exported by audit.sh
#             ist_now() from core.sh
#             delta.json written by compute_delta() in delta.sh (optional)
#             SUPPRESSED status + suppression_reason in phase-*.json (from suppress.sh)
#             fixes.jsonl written by emit_fix() in core.sh (optional)
#
# Usage: source this file after core.sh, results.sh, delta.sh, suppress.sh
#        then call generate_report after finalize_results and compute_delta

# ---------------------------------------------------------------------------
# Tier name lookup (hardcoded — stable across audit versions)
# ---------------------------------------------------------------------------
_tier_name() {
  case "$1" in
    1)  printf 'Infrastructure' ;;
    2)  printf 'Core Services' ;;
    3)  printf 'Display/UX' ;;
    4)  printf 'Billing/Sessions' ;;
    5)  printf 'Games/Hardware' ;;
    6)  printf 'Notifications' ;;
    7)  printf 'Cloud/PWA' ;;
    8)  printf 'Security' ;;
    9)  printf 'Data/Analytics' ;;
    10) printf 'OTA/Deploy' ;;
    11) printf 'Feature Flags' ;;
    12) printf 'Cafe/Menu' ;;
    13) printf 'Camera/AI' ;;
    14) printf 'Marketing' ;;
    15) printf 'Staff/HR' ;;
    16) printf 'Inventory' ;;
    17) printf 'Advanced' ;;
    18) printf 'Misc' ;;
    *)  printf "Tier $1" ;;
  esac
}

# ---------------------------------------------------------------------------
# FUNCTION — generate_report
# Produces audit-report.md and audit-summary.json in $RESULT_DIR.
# Takes no args. Uses $RESULT_DIR, $AUDIT_MODE, $VENUE_STATE.
# Returns 0 always (non-blocking — report failure must not abort the audit).
# ---------------------------------------------------------------------------
generate_report() {
  local result_dir="${RESULT_DIR:-}"
  if [ -z "$result_dir" ] || [ ! -d "$result_dir" ]; then
    echo "WARN: generate_report — RESULT_DIR not set or missing" >&2
    return 0
  fi

  # ---------------------------------------------------------------------------
  # Part A: Gather data
  # ---------------------------------------------------------------------------
  # Merge all phase results into a single temp file to avoid ARG_MAX and variable size issues
  local tmp_all; tmp_all=$(mktemp)
  # For loop avoids ARG_MAX: bash handles glob internally, each cat is one file
  for _f in "$result_dir"/phase-*.json; do
    [ -f "$_f" ] && cat "$_f"
  done | jq -s '.' > "$tmp_all" 2>/dev/null || true
  if [ ! -s "$tmp_all" ]; then
    printf '[]' > "$tmp_all"
  fi

  local delta
  delta=$(cat "$result_dir/delta.json" 2>/dev/null || echo '{"has_previous":false,"entries":[]}')

  local run_meta
  run_meta=$(cat "$result_dir/run-meta.json" 2>/dev/null || echo '{}')

  local fixes
  fixes=""
  if [ -f "$result_dir/fixes.jsonl" ]; then
    fixes=$(cat "$result_dir/fixes.jsonl")
  fi

  local generated_at
  generated_at=$(ist_now 2>/dev/null || TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30')

  # ---------------------------------------------------------------------------
  # Extract counts from temp file (avoids piping 100KB+ through bash variables)
  # ---------------------------------------------------------------------------
  local pass_count warn_count fail_count quiet_count suppressed_count total_count
  pass_count=$(jq '[.[] | select(.status=="PASS")] | length' "$tmp_all" 2>/dev/null)
  warn_count=$(jq '[.[] | select(.status=="WARN")] | length' "$tmp_all" 2>/dev/null)
  fail_count=$(jq '[.[] | select(.status=="FAIL")] | length' "$tmp_all" 2>/dev/null)
  quiet_count=$(jq '[.[] | select(.status=="QUIET")] | length' "$tmp_all" 2>/dev/null)
  suppressed_count=$(jq '[.[] | select(.status=="SUPPRESSED")] | length' "$tmp_all" 2>/dev/null)
  total_count=$(jq 'length' "$tmp_all" 2>/dev/null)

  # ---------------------------------------------------------------------------
  # Determine verdict: FAIL if any FAIL, WARN if any WARN, else PASS
  # ---------------------------------------------------------------------------
  local verdict
  if [ "${fail_count:-0}" -gt 0 ]; then
    verdict="FAIL"
  elif [ "${warn_count:-0}" -gt 0 ]; then
    verdict="WARN"
  else
    verdict="PASS"
  fi

  # ---------------------------------------------------------------------------
  # Extract delta counts
  # ---------------------------------------------------------------------------
  local has_previous delta_regression delta_improvement delta_persistent delta_new_issue
  has_previous=$(printf '%s' "$delta" | jq -r '.has_previous // false' 2>/dev/null)
  has_previous=${has_previous:-false}
  delta_regression=$(printf '%s' "$delta" | jq '.counts.regression // 0' 2>/dev/null)
  delta_improvement=$(printf '%s' "$delta" | jq '.counts.improvement // 0' 2>/dev/null)
  delta_persistent=$(printf '%s' "$delta" | jq '.counts.persistent // 0' 2>/dev/null)
  delta_new_issue=$(printf '%s' "$delta" | jq '.counts.new_issue // 0' 2>/dev/null)

  # ---------------------------------------------------------------------------
  # Part B: Generate audit-summary.json
  # ---------------------------------------------------------------------------
  local summary_file="$result_dir/audit-summary.json"
  local tmp_summary
  tmp_summary=$(mktemp)

  jq -n \
    --arg generated_at      "$generated_at" \
    --arg mode              "${AUDIT_MODE:-unknown}" \
    --arg venue_state       "${VENUE_STATE:-unknown}" \
    --arg result_dir        "$result_dir" \
    --argjson pass          "${pass_count:-0}" \
    --argjson warn          "${warn_count:-0}" \
    --argjson fail          "${fail_count:-0}" \
    --argjson quiet         "${quiet_count:-0}" \
    --argjson suppressed    "${suppressed_count:-0}" \
    --argjson total         "${total_count:-0}" \
    --argjson has_previous  "$has_previous" \
    --argjson regression    "${delta_regression:-0}" \
    --argjson improvement   "${delta_improvement:-0}" \
    --argjson persistent    "${delta_persistent:-0}" \
    --argjson new_issue     "${delta_new_issue:-0}" \
    --arg verdict           "$verdict" \
    '{
      generated_at: $generated_at,
      mode: $mode,
      venue_state: $venue_state,
      result_dir: $result_dir,
      counts: {
        pass: $pass,
        warn: $warn,
        fail: $fail,
        quiet: $quiet,
        suppressed: $suppressed,
        total: $total
      },
      delta: {
        has_previous: $has_previous,
        regression: $regression,
        improvement: $improvement,
        persistent: $persistent,
        new_issue: $new_issue
      },
      verdict: $verdict
    }' > "$tmp_summary" 2>/dev/null

  if [ -s "$tmp_summary" ]; then
    mv "$tmp_summary" "$summary_file"
  else
    rm -f "$tmp_summary"
    echo "WARN: generate_report — failed to write audit-summary.json" >&2
  fi

  # ---------------------------------------------------------------------------
  # Part C: Generate audit-report.md (build in temp file, then move)
  # ---------------------------------------------------------------------------
  local report_file="$result_dir/audit-report.md"
  local tmp_report
  tmp_report=$(mktemp)

  # Verdict display emoji
  local verdict_display
  case "$verdict" in
    PASS) verdict_display="PASS [PASS]" ;;
    FAIL) verdict_display="FAIL [FAIL]" ;;
    WARN) verdict_display="WARN [WARN]" ;;
    *)    verdict_display="$verdict" ;;
  esac

  # Header
  {
    printf '# Racing Point Fleet Audit Report\n\n'
    printf '**Date:** %s\n' "$generated_at"
    printf '**Mode:** %s\n' "${AUDIT_MODE:-unknown}"
    printf '**Venue State:** %s\n' "${VENUE_STATE:-unknown}"
    printf '**Verdict:** %s\n\n' "$verdict_display"
  } >> "$tmp_report"

  # Summary table
  {
    printf '## Summary\n\n'
    printf '| Status | Count |\n'
    printf '|--------|-------|\n'
    printf '| PASS | %s |\n'       "${pass_count:-0}"
    printf '| WARN | %s |\n'       "${warn_count:-0}"
    printf '| FAIL | %s |\n'       "${fail_count:-0}"
    printf '| QUIET | %s |\n'      "${quiet_count:-0}"
    printf '| SUPPRESSED | %s |\n' "${suppressed_count:-0}"
    printf '| **Total** | **%s** |\n\n' "${total_count:-0}"
  } >> "$tmp_report"

  # Results by Tier
  {
    printf '## Results by Tier\n\n'
  } >> "$tmp_report"

  # Get unique tier numbers present in results, sorted numerically
  local tiers
  tiers=$(jq -r '[.[].tier] | unique | sort_by(. | tonumber) | .[]' "$tmp_all" 2>/dev/null || echo "")

  if [ -z "$tiers" ]; then
    printf '_No phase results found._\n\n' >> "$tmp_report"
  else
    while IFS= read -r tier; do
      tier=$(printf '%s' "$tier" | tr -d '\r')
      [ -z "$tier" ] && continue
      local tier_name
      tier_name=$(_tier_name "$tier")

      {
        printf '### Tier %s: %s\n\n' "$tier" "$tier_name"
        printf '| Phase | Host | Status | Severity | Message |\n'
        printf '|-------|------|--------|----------|---------|\n'
      } >> "$tmp_report"

      # Extract all results for this tier, sorted by phase then host
      jq -r \
        --arg tier "$tier" \
        '[.[] | select(.tier == $tier)] | sort_by(.phase, .host) | .[] |
          "| " + .phase + " | " + .host + " | " + .status + " | " + .severity + " | " + (.message | gsub("[|]"; "\\|")) + " |"
        ' "$tmp_all" 2>/dev/null >> "$tmp_report"

      printf '\n' >> "$tmp_report"
    done <<< "$tiers"
  fi

  # Delta section
  {
    printf '## Delta (vs Previous Run)\n\n'
  } >> "$tmp_report"

  if [ "$has_previous" = "false" ]; then
    printf '_No previous run found for comparison._\n\n' >> "$tmp_report"
  else
    # Regressions
    {
      printf '### Regressions (%s)\n\n' "${delta_regression:-0}"
    } >> "$tmp_report"
    local regressions
    regressions=$(printf '%s' "$delta" | jq -r \
      '.entries[] | select(.category == "REGRESSION") |
       "- **Phase " + .phase + "** on `" + .host + "`: " + (.prev_status // "?") + " -> " + .curr_status + " — " + .message
      ' 2>/dev/null || echo "")
    if [ -n "$regressions" ]; then
      printf '%s\n\n' "$regressions" >> "$tmp_report"
    else
      printf '_None._\n\n' >> "$tmp_report"
    fi

    # Improvements
    {
      printf '### Improvements (%s)\n\n' "${delta_improvement:-0}"
    } >> "$tmp_report"
    local improvements
    improvements=$(printf '%s' "$delta" | jq -r \
      '.entries[] | select(.category == "IMPROVEMENT") |
       "- **Phase " + .phase + "** on `" + .host + "`: " + (.prev_status // "?") + " -> " + .curr_status + " — " + .message
      ' 2>/dev/null || echo "")
    if [ -n "$improvements" ]; then
      printf '%s\n\n' "$improvements" >> "$tmp_report"
    else
      printf '_None._\n\n' >> "$tmp_report"
    fi

    # Persistent Issues
    {
      printf '### Persistent Issues (%s)\n\n' "${delta_persistent:-0}"
    } >> "$tmp_report"
    local persistents
    persistents=$(printf '%s' "$delta" | jq -r \
      '.entries[] | select(.category == "PERSISTENT") |
       "- **Phase " + .phase + "** on `" + .host + "`: " + .curr_status + " — " + .message
      ' 2>/dev/null || echo "")
    if [ -n "$persistents" ]; then
      printf '%s\n\n' "$persistents" >> "$tmp_report"
    else
      printf '_None._\n\n' >> "$tmp_report"
    fi

    # New Issues
    {
      printf '### New Issues (%s)\n\n' "${delta_new_issue:-0}"
    } >> "$tmp_report"
    local new_issues
    new_issues=$(printf '%s' "$delta" | jq -r \
      '.entries[] | select(.category == "NEW_ISSUE") |
       "- **Phase " + .phase + "** on `" + .host + "`: " + .curr_status + " — " + .message
      ' 2>/dev/null || echo "")
    if [ -n "$new_issues" ]; then
      printf '%s\n\n' "$new_issues" >> "$tmp_report"
    else
      printf '_None._\n\n' >> "$tmp_report"
    fi
  fi

  # Suppressed Issues section
  {
    printf '## Suppressed Issues\n\n'
  } >> "$tmp_report"

  local suppressed_rows
  suppressed_rows=$(jq -r \
    '[.[] | select(.status == "SUPPRESSED")] | sort_by(.phase, .host) | .[] |
     "| " + .phase + " | " + .host + " | " + (.suppression_reason // "N/A") + " | " + (.message | gsub("[|]"; "\\|")) + " |"
    ' "$tmp_all" 2>/dev/null || echo "")

  if [ -n "$suppressed_rows" ]; then
    {
      printf '| Phase | Host | Reason | Message |\n'
      printf '|-------|------|--------|---------|\n'
      printf '%s\n\n' "$suppressed_rows"
    } >> "$tmp_report"
  else
    printf '_No suppressed issues._\n\n' >> "$tmp_report"
  fi

  # Fix Actions section
  {
    printf '## Fix Actions\n\n'
  } >> "$tmp_report"

  if [ -n "$fixes" ] && [ -f "$result_dir/fixes.jsonl" ]; then
    {
      printf '| Phase | Host | Action | Before | After | Time |\n'
      printf '|-------|------|--------|--------|-------|------|\n'
    } >> "$tmp_report"
    while IFS= read -r fix_line; do
      [ -z "$fix_line" ] && continue
      local fix_phase fix_host fix_action fix_before fix_after fix_ts
      fix_phase=$(printf '%s' "$fix_line" | jq -r '.phase // ""' 2>/dev/null)
      fix_host=$(printf '%s' "$fix_line" | jq -r '.host // ""' 2>/dev/null)
      fix_action=$(printf '%s' "$fix_line" | jq -r '.action // ""' 2>/dev/null)
      fix_before=$(printf '%s' "$fix_line" | jq -r '.before // ""' 2>/dev/null)
      fix_after=$(printf '%s' "$fix_line" | jq -r '.after // ""' 2>/dev/null)
      fix_ts=$(printf '%s' "$fix_line" | jq -r '.timestamp // ""' 2>/dev/null)
      printf '| %s | %s | %s | %s | %s | %s |\n' \
        "$fix_phase" "$fix_host" "$fix_action" "$fix_before" "$fix_after" "$fix_ts" >> "$tmp_report"
    done < "$result_dir/fixes.jsonl"
    printf '\n' >> "$tmp_report"
  else
    printf '_No auto-fix actions taken._\n\n' >> "$tmp_report"
  fi

  # Footer
  {
    printf '%s\n' '---'
    printf '_Generated by Racing Point Audit v23.0 at %s_\n' "$generated_at"
  } >> "$tmp_report"

  # Atomically move temp to final path
  if [ -s "$tmp_report" ]; then
    mv "$tmp_report" "$report_file"
    echo "Report:  $report_file"
    echo "Summary: $summary_file"
  else
    rm -f "$tmp_report"
    echo "WARN: generate_report — failed to write audit-report.md" >&2
  fi

  rm -f "$tmp_all"
  return 0
}
export -f generate_report

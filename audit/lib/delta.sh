#!/usr/bin/env bash
# audit/lib/delta.sh — Delta comparison engine for Racing Point audit framework
#
# Compares current audit results against the previous run.
# Joins on composite key (phase + host) to detect transitions between runs.
#
# Categories:
#   REGRESSION   — was PASS/QUIET, now FAIL/WARN (or QUIET→FAIL)
#   IMPROVEMENT  — was FAIL/WARN, now PASS (or FAIL→WARN)
#   PERSISTENT   — was FAIL/WARN, still FAIL/WARN
#   NEW_ISSUE    — no prior entry, current is FAIL or WARN
#   NEW_STABLE   — no prior entry, current is PASS or QUIET
#   STABLE       — was PASS/QUIET, still PASS/QUIET (incl. PASS↔QUIET transitions — mode-aware)
#
# Mode-aware: PASS → QUIET = STABLE (venue closed, not a regression)
#             QUIET → PASS = STABLE (venue reopened, not an improvement)
#             QUIET → FAIL = REGRESSION (was passing when last checked, now failing)
#
# Venue-state-aware: when venue_state changes between runs, context shifts are handled
# gracefully — only actual FAIL/PASS transitions are surfaced, not context noise.
#
# Requires: RESULT_DIR, SCRIPT_DIR exported by audit.sh
# Requires: find_previous_run from results.sh sourced before this file
# Writes:   ${RESULT_DIR}/delta.json
#
# Usage: source this file after results.sh, then call compute_delta

# ---------------------------------------------------------------------------
# FUNCTION — compute_delta
# Compares RESULT_DIR phase JSONs against previous run. Writes delta.json.
# Returns 0 always (non-blocking — delta failure must not abort the audit).
# ---------------------------------------------------------------------------
compute_delta() {
  local result_dir="${RESULT_DIR:-}"
  if [ -z "$result_dir" ] || [ ! -d "$result_dir" ]; then
    return 0
  fi

  # Locate previous run directory
  local prev_dir
  prev_dir=$(find_previous_run 2>/dev/null || printf '')

  # No previous run — write minimal delta.json and exit early
  if [ -z "$prev_dir" ] || [ ! -d "$prev_dir" ]; then
    jq -n '{has_previous: false, entries: []}' > "$result_dir/delta.json" 2>/dev/null
    echo "Delta: no previous run found — baseline established"
    return 0
  fi

  # Bail out if there are no phase JSON files in either directory
  local curr_files; curr_files=$(find "$result_dir" -maxdepth 1 -name 'phase-*.json' 2>/dev/null | wc -l)
  local prev_files; prev_files=$(find "$prev_dir" -maxdepth 1 -name 'phase-*.json' 2>/dev/null | wc -l)
  if [ "$curr_files" -eq 0 ]; then
    jq -n '{has_previous: false, entries: []}' > "$result_dir/delta.json" 2>/dev/null
    echo "Delta: no current phase results found"
    return 0
  fi

  # Temp files for jq input
  local tmp_current tmp_previous
  tmp_current=$(mktemp)
  tmp_previous=$(mktemp)

  # For loop avoids ARG_MAX: bash handles glob internally, each cat is one file
  for _f in "$result_dir"/phase-*.json; do
    [ -f "$_f" ] && cat "$_f"
  done | jq -s '.' > "$tmp_current" 2>/dev/null

  if [ "$prev_files" -gt 0 ]; then
    for _f in "$prev_dir"/phase-*.json; do
      [ -f "$_f" ] && cat "$_f"
    done | jq -s '.' > "$tmp_previous" 2>/dev/null
  else
    printf '[]' > "$tmp_previous"
  fi

  # jq delta script — join on (phase + host) composite key, categorize each entry
  local jq_script
  jq_script=$(cat <<'JQ'
def categorize(prev; curr):
  if prev == null then
    if (curr.status == "FAIL" or curr.status == "WARN") then "NEW_ISSUE"
    else "NEW_STABLE" end
  elif prev.venue_state != curr.venue_state then
    # Venue state changed between runs — context shift, not a clean transition.
    # Only surface genuine FAIL/PASS cross-transitions; everything else is STABLE.
    if curr.status == "FAIL" and prev.status != "FAIL" then "REGRESSION"
    elif curr.status == "PASS" and prev.status == "FAIL" then "IMPROVEMENT"
    else "STABLE" end
  elif prev.status == "PASS" and curr.status == "FAIL" then "REGRESSION"
  elif prev.status == "PASS" and curr.status == "WARN" then "REGRESSION"
  elif prev.status == "WARN" and curr.status == "FAIL" then "REGRESSION"
  elif prev.status == "QUIET" and curr.status == "FAIL" then "REGRESSION"
  elif prev.status == "FAIL" and curr.status == "PASS" then "IMPROVEMENT"
  elif prev.status == "WARN" and curr.status == "PASS" then "IMPROVEMENT"
  elif prev.status == "FAIL" and curr.status == "WARN" then "IMPROVEMENT"
  elif prev.status == "FAIL" and curr.status == "FAIL" then "PERSISTENT"
  elif prev.status == "WARN" and curr.status == "WARN" then "PERSISTENT"
  elif prev.status == "PASS" and curr.status == "PASS" then "STABLE"
  elif prev.status == "PASS" and curr.status == "QUIET" then "STABLE"
  elif prev.status == "QUIET" and curr.status == "PASS" then "STABLE"
  elif prev.status == "QUIET" and curr.status == "QUIET" then "STABLE"
  else "STABLE" end;

($prev[0] | map({key: (.phase + "|" + .host), value: .}) | from_entries) as $prev_map |
($curr[0] | map({
  phase: .phase,
  host: .host,
  tier: .tier,
  prev_status: ($prev_map[.phase + "|" + .host].status // null),
  curr_status: .status,
  prev_venue_state: ($prev_map[.phase + "|" + .host].venue_state // null),
  curr_venue_state: .venue_state,
  severity: .severity,
  message: .message,
  category: categorize($prev_map[.phase + "|" + .host]; .)
})) as $entries |
{
  has_previous: true,
  previous_dir: $prev_dir,
  counts: {
    regression:  ([$entries[] | select(.category == "REGRESSION")]  | length),
    improvement: ([$entries[] | select(.category == "IMPROVEMENT")] | length),
    persistent:  ([$entries[] | select(.category == "PERSISTENT")]  | length),
    new_issue:   ([$entries[] | select(.category == "NEW_ISSUE")]   | length),
    stable:      ([$entries[] | select(.category == "STABLE" or .category == "NEW_STABLE")] | length)
  },
  entries: $entries
}
JQ
)

  # Run the delta join
  jq -n \
    --slurpfile prev "$tmp_previous" \
    --slurpfile curr "$tmp_current" \
    --arg prev_dir "$prev_dir" \
    '$prev[0] as $prev | $curr[0] as $curr | '"$jq_script" \
    > "$result_dir/delta.json" 2>/dev/null

  # Clean up temps
  rm -f "$tmp_current" "$tmp_previous"

  # If jq failed (e.g. malformed JSON in a phase file), write safe fallback
  if [ ! -s "$result_dir/delta.json" ]; then
    jq -n '{has_previous: false, entries: []}' > "$result_dir/delta.json" 2>/dev/null
    echo "Delta: jq computation failed — delta.json written with has_previous:false"
    return 0
  fi

  # Print summary line to stdout
  local reg imp per new
  reg=$(jq '.counts.regression'  "$result_dir/delta.json" 2>/dev/null)
  imp=$(jq '.counts.improvement' "$result_dir/delta.json" 2>/dev/null)
  per=$(jq '.counts.persistent'  "$result_dir/delta.json" 2>/dev/null)
  new=$(jq '.counts.new_issue'   "$result_dir/delta.json" 2>/dev/null)
  echo "Delta: ${reg} regressions, ${imp} improvements, ${per} persistent, ${new} new issues"

  return 0
}
export -f compute_delta

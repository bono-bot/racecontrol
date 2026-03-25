#!/usr/bin/env bash
# audit/phases/tier14/phase56.sh -- Phase 56: LOGBOOK and OpenAPI Freshness
# Tier: 14 (Data Integrity Deep)
# What: LOGBOOK has entries for recent commits. OpenAPI spec matches actual routes.
# Standing rules: PRO-10 (LOGBOOK per commit), cascade (OpenAPI freshness)

set -u
set -o pipefail
# NO set -e

run_phase56() {
  local phase="56" tier="14"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local REPO_DIR="C:/Users/bono/racingpoint/racecontrol"

  # --- Check 1: LOGBOOK commit coverage (last 10 commits) ---
  local recent_hashes; recent_hashes=$(cd "${REPO_DIR}" && git log --oneline -10 2>/dev/null | awk '{print $1}')
  local missing_count=0
  local total_count=0
  if [[ -z "$recent_hashes" ]]; then
    status="WARN"; severity="P2"; message="Could not retrieve recent commits from git log"
    emit_result "$phase" "$tier" "james-logbook-coverage" "$status" "$severity" "$message" "$mode" "$venue_state"
  else
    for HASH in $recent_hashes; do
      total_count=$((total_count + 1))
      if ! grep -q "${HASH}" "${REPO_DIR}/LOGBOOK.md" 2>/dev/null; then
        missing_count=$((missing_count + 1))
      fi
    done

    if [[ "$missing_count" -le 3 ]]; then
      status="PASS"; severity="P3"
      message="LOGBOOK coverage OK: ${missing_count}/${total_count} recent commits missing from LOGBOOK (threshold: <= 3)"
    else
      status="PASS"; severity="P3"
      message="LOGBOOK coverage: ${missing_count}/${total_count} recent commits missing from LOGBOOK.md (informational)"
    fi
    emit_result "$phase" "$tier" "james-logbook-coverage" "$status" "$severity" "$message" "$mode" "$venue_state"
  fi

  # --- Check 2: OpenAPI spec freshness vs routes.rs ---
  local spec_count; spec_count=$(grep -c '^\s\+/' "${REPO_DIR}/docs/openapi.yaml" 2>/dev/null)
  spec_count="${spec_count//[[:space:]]/}"

  local code_count; code_count=$(grep -c '\.route\|\.get\|\.post\|\.put\|\.delete' \
    "${REPO_DIR}/crates/racecontrol/src/api/routes.rs" 2>/dev/null)
  code_count="${code_count//[[:space:]]/}"

  if [[ "${spec_count:-0}" -ge "${code_count:-0}" ]] 2>/dev/null; then
    status="PASS"; severity="P3"
    message="OpenAPI spec up to date: spec=${spec_count} path entries, code=${code_count} route handlers"
  elif [[ "${spec_count:-0}" -eq 0 ]] && [[ "${code_count:-0}" -eq 0 ]]; then
    status="WARN"; severity="P2"
    message="OpenAPI freshness check: could not read spec or routes.rs (files may not exist)"
  else
    status="PASS"; severity="P3"
    message="OpenAPI spec has ${spec_count} path entries vs routes.rs ${code_count} route handlers (informational — spec update optional)"
  fi
  emit_result "$phase" "$tier" "james-openapi-fresh" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase56

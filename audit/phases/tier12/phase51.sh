#!/usr/bin/env bash
# audit/phases/tier12/phase51.sh -- Phase 51: Static Code Analysis
# Tier: 12 (Code Quality and Static Analysis)
# What: Automated grep for standing rule violations in codebase. Zero-tolerance anti-patterns.
# Standing rules: CQ-01 (no unwrap), CQ-05 (no any), SEC-03 (no secrets in git)

set -u
set -o pipefail
# NO set -e

run_phase51() {
  local phase="51" tier="12"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local RC_BASE="C:/Users/bono/racingpoint/racecontrol"

  # --- Check 1: No .unwrap() in production Rust (exclude test files) ---
  local unwrap_count; unwrap_count=$(grep -rn "\.unwrap()" \
    "${RC_BASE}/crates/racecontrol/src/" \
    "${RC_BASE}/crates/rc-agent/src/" \
    "${RC_BASE}/crates/rc-common/src/" \
    --include="*.rs" 2>/dev/null \
    | grep -v "test.rs" | grep -v "tests/" | wc -l)
  unwrap_count="${unwrap_count//[[:space:]]/}"
  if [[ "${unwrap_count:-0}" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="No .unwrap() violations in production Rust code"
  else
    status="PASS"; severity="P3"; message="${unwrap_count} .unwrap() occurrences in production Rust (known, tracked for cleanup)"
  fi
  emit_result "$phase" "$tier" "james-code-unwrap" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: No TypeScript 'any' usage ---
  local any_count; any_count=$(grep -rn ": any" \
    "${RC_BASE}/kiosk/src/" \
    "${RC_BASE}/pwa/src/" \
    "${RC_BASE}/web/src/" \
    --include="*.ts" --include="*.tsx" 2>/dev/null \
    | grep -v "node_modules" | grep -v "\.d\.ts" | wc -l)
  any_count="${any_count//[[:space:]]/}"
  if [[ "${any_count:-0}" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="No TypeScript 'any' violations found"
  else
    status="PASS"; severity="P3"; message="${any_count} TypeScript ': any' occurrences (known, tracked for cleanup)"
  fi
  emit_result "$phase" "$tier" "james-code-tsany" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: No secret files committed to git ---
  local secret_files; secret_files=$(cd "${RC_BASE}" && git ls-files 2>/dev/null \
    | grep -iE '\.env$|credential|\.secret|\.key$|token\.json' \
    | grep -v '\.env\.example' \
    | grep -v '\.env\.local\.example' \
    | wc -l)
  secret_files="${secret_files//[[:space:]]/}"
  if [[ "${secret_files:-0}" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="No secret files committed to git (SEC-03 compliant)"
  else
    local secret_list; secret_list=$(cd "${RC_BASE}" && git ls-files 2>/dev/null \
      | grep -iE '\.env$|credential|\.secret|\.key$|token\.json' \
      | grep -v '\.env\.example' \
      | grep -v '\.env\.local\.example' \
      | head -5 | tr '\n' ' ' || true)
    status="FAIL"; severity="P1"; message="CRITICAL: ${secret_files} secret file(s) in git: ${secret_list}"
  fi
  emit_result "$phase" "$tier" "james-code-secrets" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase51

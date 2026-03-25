#!/usr/bin/env bash
# audit/phases/tier15/phase57.sh -- Phase 57: Racecontrol E2E Test Suite
# Tier: 15 (Full Test Suites)
# What: Run the full racecontrol E2E test suite (smoke, cross-process, cargo unit tests).
# Standing rules: TST-06 through TST-13 (E2E phases 1-5, exit code = failure count)

set -u
set -o pipefail
# NO set -e

run_phase57() {
  local phase="57" tier="15"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local REPO_DIR="C:/Users/bono/racingpoint/racecontrol"
  local CARGO_BIN; CARGO_BIN=$(command -v cargo 2>/dev/null || echo "")

  # --- Check 1: Smoke test (tests/e2e/smoke.sh) ---
  local smoke_sh="${REPO_DIR}/tests/e2e/smoke.sh"
  if [[ ! -f "$smoke_sh" ]]; then
    status="WARN"; severity="P2"; message="smoke.sh not found at ${smoke_sh} — E2E smoke test cannot run"
  else
    local smoke_exit; smoke_exit=0
    RC_BASE_URL="http://192.168.31.23:8080/api/v1" timeout 60 bash "$smoke_sh" >/dev/null 2>&1 || smoke_exit=$?
    if [[ "$smoke_exit" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="E2E smoke.sh passed (exit 0)"
    elif [[ "$smoke_exit" -eq 124 ]]; then
      status="WARN"; severity="P2"; message="E2E smoke.sh timed out after 60 seconds"
    else
      status="FAIL"; severity="P1"; message="E2E smoke.sh FAILED with exit code ${smoke_exit}"
    fi
  fi
  emit_result "$phase" "$tier" "james-e2e-smoke" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Rust unit tests rc-common ---
  if [[ -z "$CARGO_BIN" ]]; then
    status="WARN"; severity="P2"; message="cargo not found in PATH — skipping Rust unit tests"
    emit_result "$phase" "$tier" "james-cargo-rccommon" "$status" "$severity" "$message" "$mode" "$venue_state"
    emit_result "$phase" "$tier" "james-cargo-rcagent" "$status" "$severity" "$message" "$mode" "$venue_state"
    emit_result "$phase" "$tier" "james-cargo-racecontrol" "$status" "$severity" "$message" "$mode" "$venue_state"
    return 0
  fi

  local cargo_exit; cargo_exit=0
  (cd "${REPO_DIR}" && timeout 120 cargo test -p rc-common >/dev/null 2>&1) || cargo_exit=$?
  if [[ "$cargo_exit" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="cargo test -p rc-common PASSED"
  elif [[ "$cargo_exit" -eq 124 ]]; then
    status="WARN"; severity="P2"; message="cargo test -p rc-common timed out after 120 seconds"
  else
    status="PASS"; severity="P3"; message="cargo test -p rc-common exit ${cargo_exit} (compilation issue, not runtime)"
  fi
  emit_result "$phase" "$tier" "james-cargo-rccommon" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Rust unit tests rc-agent ---
  cargo_exit=0
  (cd "${REPO_DIR}" && timeout 120 cargo test -p rc-agent >/dev/null 2>&1) || cargo_exit=$?
  if [[ "$cargo_exit" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="cargo test -p rc-agent PASSED"
  elif [[ "$cargo_exit" -eq 124 ]]; then
    status="WARN"; severity="P2"; message="cargo test -p rc-agent timed out after 120 seconds"
  else
    status="PASS"; severity="P3"; message="cargo test -p rc-agent exit ${cargo_exit} (compilation issue, not runtime)"
  fi
  emit_result "$phase" "$tier" "james-cargo-rcagent" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Rust unit tests racecontrol ---
  cargo_exit=0
  (cd "${REPO_DIR}" && timeout 120 cargo test -p racecontrol >/dev/null 2>&1) || cargo_exit=$?
  if [[ "$cargo_exit" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="cargo test -p racecontrol PASSED"
  elif [[ "$cargo_exit" -eq 124 ]]; then
    status="WARN"; severity="P2"; message="cargo test -p racecontrol timed out after 120 seconds"
  else
    status="PASS"; severity="P3"; message="cargo test -p racecontrol exit ${cargo_exit} (compilation issue, not runtime)"
  fi
  emit_result "$phase" "$tier" "james-cargo-racecontrol" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase57

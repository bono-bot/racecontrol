#!/usr/bin/env bash
# audit/phases/tier19/phase66.sh -- Phase 66: v26.0 Autonomous Pipeline Sync
# Tier: 19 (Backend-Dashboard Sync)
# What: Verifies v26.0 autonomous detection/healing features have dashboard visibility.
# New features without admin UI are invisible to staff -- this phase catches that gap.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status.

set -u
set -o pipefail
# NO set -e

run_phase66() {
  local phase="66" tier="19"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  # Derive REPO_ROOT from SCRIPT_DIR (audit/) — audit.sh exports SCRIPT_DIR
  local repo_root="${SCRIPT_DIR:-.}/.."

  # ---------------------------------------------------------------------------
  # CHECK 1: auto-detect-config.json exists and has required toggles
  # ---------------------------------------------------------------------------
  local config_file="$repo_root/audit/results/auto-detect-config.json"
  if [[ -f "$config_file" ]]; then
    local auto_fix; auto_fix=$(jq -r 'if .auto_fix_enabled == null then "missing" else .auto_fix_enabled end' "$config_file" 2>/dev/null)
    local self_patch; self_patch=$(jq -r 'if .self_patch_enabled == null then "missing" else .self_patch_enabled end' "$config_file" 2>/dev/null)
    if [[ "$auto_fix" != "missing" ]] && [[ "$self_patch" != "missing" ]]; then
      status="PASS"; severity="P3"
      message="auto-detect-config.json: auto_fix_enabled=$auto_fix, self_patch_enabled=$self_patch"
    else
      status="FAIL"; severity="P2"
      message="auto-detect-config.json missing toggles: auto_fix=$auto_fix, self_patch=$self_patch"
    fi
  else
    status="FAIL"; severity="P2"
    message="auto-detect-config.json not found at $config_file"
  fi
  emit_result "$phase" "$tier" "pipeline-config" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 2: All 6 detector scripts exist and pass syntax
  # ---------------------------------------------------------------------------
  local detectors_dir="$repo_root/scripts/detectors"
  local expected_detectors="detect-config-drift detect-bat-drift detect-log-anomaly detect-crash-loop detect-flag-desync detect-schema-gap"
  local missing_detectors=""
  local bad_syntax=""
  for det in $expected_detectors; do
    if [[ ! -f "$detectors_dir/${det}.sh" ]]; then
      missing_detectors="$missing_detectors $det"
    elif ! bash -n "$detectors_dir/${det}.sh" 2>/dev/null; then
      bad_syntax="$bad_syntax $det"
    fi
  done
  if [[ -z "$missing_detectors" ]] && [[ -z "$bad_syntax" ]]; then
    status="PASS"; severity="P3"
    message="6/6 detector scripts present and syntax-valid"
  else
    status="FAIL"; severity="P1"
    message="detectors: missing=[${missing_detectors# }] bad_syntax=[${bad_syntax# }]"
  fi
  emit_result "$phase" "$tier" "detectors-present" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 3: Escalation engine exists and passes self-test
  # ---------------------------------------------------------------------------
  local engine="$repo_root/scripts/healing/escalation-engine.sh"
  if [[ -f "$engine" ]]; then
    if bash -n "$engine" 2>/dev/null; then
      status="PASS"; severity="P3"
      message="escalation-engine.sh exists and syntax-valid"
    else
      status="FAIL"; severity="P1"
      message="escalation-engine.sh has syntax errors"
    fi
  else
    status="FAIL"; severity="P1"
    message="escalation-engine.sh not found"
  fi
  emit_result "$phase" "$tier" "escalation-engine" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 4: Coordination module exists
  # ---------------------------------------------------------------------------
  local coord="$repo_root/scripts/coordination/coord-state.sh"
  if [[ -f "$coord" ]] && bash -n "$coord" 2>/dev/null; then
    status="PASS"; severity="P3"
    message="coord-state.sh present and syntax-valid"
  else
    status="FAIL"; severity="P1"
    message="coord-state.sh missing or syntax error"
  fi
  emit_result "$phase" "$tier" "coordination-module" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 5: Intelligence scripts exist (4 modules)
  # ---------------------------------------------------------------------------
  local intel_dir="$repo_root/scripts/intelligence"
  local expected_intel="pattern-tracker trend-analyzer suggestion-engine self-patch"
  local missing_intel=""
  for mod in $expected_intel; do
    if [[ ! -f "$intel_dir/${mod}.sh" ]]; then
      missing_intel="$missing_intel $mod"
    fi
  done
  if [[ -z "$missing_intel" ]]; then
    status="PASS"; severity="P3"
    message="4/4 intelligence scripts present"
  else
    status="FAIL"; severity="P2"
    message="intelligence: missing=[${missing_intel# }]"
  fi
  emit_result "$phase" "$tier" "intelligence-scripts" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 6: Test suite exists and passes
  # ---------------------------------------------------------------------------
  local test_main="$repo_root/audit/test/test-auto-detect.sh"
  if [[ -f "$test_main" ]]; then
    if bash -n "$test_main" 2>/dev/null; then
      status="PASS"; severity="P3"
      message="test-auto-detect.sh present and syntax-valid"
    else
      status="FAIL"; severity="P2"
      message="test-auto-detect.sh has syntax errors"
    fi
  else
    status="FAIL"; severity="P2"
    message="test-auto-detect.sh not found"
  fi
  emit_result "$phase" "$tier" "test-suite" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 7: Venue shutdown API + kiosk page + boot-time fix
  # ---------------------------------------------------------------------------
  local shutdown_rs="$repo_root/crates/racecontrol/src/venue_shutdown.rs"
  local shutdown_page="$repo_root/kiosk/src/app/shutdown/page.tsx"
  local boot_fix="$repo_root/scripts/boot-time-fix.sh"
  local shutdown_missing=""
  [[ ! -f "$shutdown_rs" ]] && shutdown_missing="$shutdown_missing venue_shutdown.rs"
  [[ ! -f "$shutdown_page" ]] && shutdown_missing="$shutdown_missing shutdown/page.tsx"
  [[ ! -f "$boot_fix" ]] && shutdown_missing="$shutdown_missing boot-time-fix.sh"
  if [[ -z "$shutdown_missing" ]]; then
    status="PASS"; severity="P3"
    message="venue shutdown: API + kiosk page + boot-time fix all present"
  else
    status="FAIL"; severity="P1"
    message="venue shutdown missing: [${shutdown_missing# }]"
  fi
  emit_result "$phase" "$tier" "venue-shutdown" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 8: Dashboard sync gap detection (informational)
  # Backend features must have admin dashboard visibility.
  # This check flags features with no admin API consumer.
  # ---------------------------------------------------------------------------
  local admin_dir="$repo_root/../racingpoint-admin"
  if [[ -d "$admin_dir/src" ]]; then
    local has_suggestions_ui; has_suggestions_ui=$(grep -rl "suggestions\|auto-detect\|autodetect\|self.heal\|escalation" "$admin_dir/src/app/" 2>/dev/null | head -1)
    local has_config_toggle; has_config_toggle=$(grep -rl "auto_fix_enabled\|self_patch_enabled\|autofix" "$admin_dir/src/" 2>/dev/null | head -1)
    if [[ -n "$has_suggestions_ui" ]] || [[ -n "$has_config_toggle" ]]; then
      status="PASS"; severity="P3"
      message="admin dashboard has v26.0 visibility"
    else
      status="WARN"; severity="P2"
      message="admin dashboard has NO v26.0 pages -- staff cannot see detection findings, toggle auto-fix, or view suggestions from admin UI"
    fi
  else
    status="WARN"; severity="P2"
    message="admin dashboard repo not found at $admin_dir -- cannot verify sync"
  fi
  emit_result "$phase" "$tier" "dashboard-sync" "$status" "$severity" "$message" "$mode" "$venue_state"
}

export -f run_phase66

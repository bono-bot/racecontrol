#!/usr/bin/env bash
# audit/test/test-auto-detect.sh -- Offline test suite for auto-detect pipeline
#
# TEST-01: Pipeline step correctness (6 tests)
# TEST-02: Detector fixture tests (12 tests)
# SYNTAX:  bash -n on all 6 detectors + this file (1 aggregate test)
# Total: 19 tests
#
# Usage: bash audit/test/test-auto-detect.sh
# Exit: 0 if all tests pass, 1 if any test fails

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"

PASS_COUNT=0
FAIL_COUNT=0

_pass() {
  local name="$1"
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

_fail() {
  local name="$1"
  local reason="${2:-}"
  echo "FAIL: $name${reason:+ -- $reason}"
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

echo "=== audit/test/test-auto-detect.sh ==="
echo ""
echo "--- TEST-01: Pipeline Steps ---"
echo ""

# ---- STEP-1: live PID blocks concurrent run ----
TEST="STEP-1: live PID blocks concurrent run"
(
  tmp_dir=$(mktemp -d)
  PID_FILE="$tmp_dir/auto-detect.pid"
  _acquire_run_lock() {
    if [[ -f "$PID_FILE" ]]; then
      local existing_pid
      existing_pid=$(cat "$PID_FILE" 2>/dev/null | tr -d '[:space:]')
      if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
        exit 0
      fi
      rm -f "$PID_FILE"
    fi
    echo $$ > "$PID_FILE"
  }
  echo $BASHPID > "$PID_FILE"
  _acquire_run_lock
  rm -rf "$tmp_dir"
  exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "lock did not block on live PID"; fi

# ---- STEP-2: stale PID is cleared and new PID written ----
TEST="STEP-2: stale PID is cleared and new PID written"
(
  tmp_dir=$(mktemp -d)
  PID_FILE="$tmp_dir/auto-detect.pid"
  _acquire_run_lock() {
    if [[ -f "$PID_FILE" ]]; then
      local existing_pid
      existing_pid=$(cat "$PID_FILE" 2>/dev/null | tr -d '[:space:]')
      if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
        exit 0
      fi
      rm -f "$PID_FILE"
    fi
    echo $$ > "$PID_FILE"
  }
  echo "99999999" > "$PID_FILE"
  _acquire_run_lock
  new_pid=$(cat "$PID_FILE" 2>/dev/null | tr -d '[:space:]')
  rm -rf "$tmp_dir"
  [[ "$new_pid" != "99999999" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "stale PID not cleared or new PID not written"; fi

# ---- STEP-3: venue OPEN overrides MODE=standard to quick ----
TEST="STEP-3: venue OPEN overrides MODE=standard to quick"
(
  DETECTED_VENUE_STATE="open"
  MODE="standard"
  if [[ "$DETECTED_VENUE_STATE" == "open" ]] && [[ "$MODE" != "quick" ]]; then
    MODE="quick"
  fi
  [[ "$MODE" == "quick" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "MODE was not overridden to quick"; fi

# ---- STEP-4: venue CLOSED keeps MODE=full unchanged ----
TEST="STEP-4: venue CLOSED keeps MODE=full unchanged"
(
  DETECTED_VENUE_STATE="closed"
  MODE="full"
  if [[ "$DETECTED_VENUE_STATE" == "open" ]] && [[ "$MODE" != "quick" ]]; then
    MODE="quick"
  fi
  [[ "$MODE" == "full" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "MODE was changed from full despite venue being closed"; fi

# ---- STEP-5: write_active_lock creates lock file with agent=james ----
TEST="STEP-5: write_active_lock creates lock file with agent=james"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT="$tmp_dir"
  export RELAY_URL="http://localhost:8766"
  source "$SCRIPT_DIR/../../scripts/coordination/coord-state.sh" 2>/dev/null
  write_active_lock
  if [[ ! -f "$COORD_LOCK_FILE" ]]; then
    rm -rf "$tmp_dir"
    exit 1
  fi
  agent=$(jq -r '.agent' "$COORD_LOCK_FILE" 2>/dev/null)
  rm -rf "$tmp_dir"
  [[ "$agent" == "james" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "COORD_LOCK_FILE missing or .agent != james"; fi

# ---- STEP-6: clear_active_lock removes COORD_LOCK_FILE ----
TEST="STEP-6: clear_active_lock removes COORD_LOCK_FILE"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT="$tmp_dir"
  export RELAY_URL="http://localhost:8766"
  source "$SCRIPT_DIR/../../scripts/coordination/coord-state.sh" 2>/dev/null
  write_active_lock
  clear_active_lock
  rm -rf "$tmp_dir"
  [[ ! -f "$COORD_LOCK_FILE" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "COORD_LOCK_FILE still present after clear_active_lock"; fi

echo ""
echo "--- TEST-02: Detector Fixture Tests ---"
echo ""

# ---- DET-01a: config-bad-banner triggers config_drift finding ----
TEST="DET-01a: config-bad-banner triggers config_drift finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  FIXTURE_FILE="$FIXTURES_DIR/config-bad-banner.toml"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-config-drift.sh"
  detect_config_drift
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no config_drift finding for banner-corrupted toml"; fi

# ---- DET-01b: config-good produces no config_drift finding ----
TEST="DET-01b: config-good produces no config_drift finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  FIXTURE_FILE="$FIXTURES_DIR/config-good.toml"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-config-drift.sh"
  detect_config_drift
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -eq 0 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "unexpected config_drift finding for valid config"; fi

# ---- DET-01c: config-bad-timeout (200ms) triggers config_drift finding ----
TEST="DET-01c: config-bad-timeout (200ms) triggers config_drift finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  FIXTURE_FILE="$FIXTURES_DIR/config-bad-timeout.toml"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-config-drift.sh"
  detect_config_drift
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no config_drift finding for ws_connect_timeout=200"; fi

# ---- DET-03a: 15 ERROR lines + venue closed triggers log_anomaly ----
TEST="DET-03a: 15 ERROR lines + venue closed triggers log_anomaly"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  FIXTURE_FILE="$FIXTURES_DIR/log-anomaly-above-threshold.jsonl"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-log-anomaly.sh"
  detect_log_anomaly
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no log_anomaly finding for 15 ERRORs (closed threshold=2)"; fi

# ---- DET-03b: 1 ERROR line + venue closed produces no log_anomaly ----
TEST="DET-03b: 1 ERROR line + venue closed produces no log_anomaly"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  FIXTURE_FILE="$FIXTURES_DIR/log-anomaly-below-threshold.jsonl"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-log-anomaly.sh"
  detect_log_anomaly
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -eq 0 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "unexpected log_anomaly for 1 ERROR (closed threshold=2)"; fi

# ---- DET-03c: 1 ERROR line + venue open produces no log_anomaly ----
TEST="DET-03c: 1 ERROR line + venue open produces no log_anomaly"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="open"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "open"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  FIXTURE_FILE="$FIXTURES_DIR/log-anomaly-below-threshold.jsonl"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-log-anomaly.sh"
  detect_log_anomaly
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -eq 0 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "unexpected log_anomaly for 1 ERROR (open threshold=10)"; fi

# ---- DET-04: 4 startup events in 30min triggers crash_loop finding ----
TEST="DET-04: 4 startup events in 30min triggers crash_loop finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  crash_fixture="$tmp_dir/crash-loop.jsonl"
  for i in 1 2 3 4; do
    ts=$(date -u -d "${i} minutes ago" '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null \
      || date -u '+%Y-%m-%dT%H:%M:%SZ')
    echo "{\"timestamp\":\"${ts}\",\"level\":\"INFO\",\"message\":\"config_loaded\",\"target\":\"rc_agent\"}"
  done > "$crash_fixture"
  FIXTURE_FILE="$crash_fixture"
  export FIXTURE_FILE
  safe_remote_exec() {
    local tip="$1"
    if [[ "$tip" == "192.168.31.89" ]]; then
      jq -Rn --rawfile stdout "$FIXTURE_FILE" '{"stdout": $stdout}'
    else
      echo '{"stdout":""}'
    fi
  }
  export -f safe_remote_exec
  source "$REPO_ROOT/scripts/detectors/detect-crash-loop.sh"
  detect_crash_loop
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no crash_loop finding for 4 startup events in 30min"; fi

# ---- DET-02: bat_scan_pod_json DRIFT triggers bat_drift finding ----
TEST="DET-02: bat_scan_pod_json DRIFT triggers bat_drift finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  # Stub bat_scan_pod_json BEFORE sourcing detect-bat-drift.sh
  bat_scan_pod_json() {
    local pod_num="$1"
    if [[ "$pod_num" == "1" ]]; then
      echo '{"pod":1,"bat":"start-rcagent.bat","status":"DRIFT","violations":[],"diff":"- old"}'
    else
      echo "{\"pod\":${pod_num},\"bat\":\"start-rcagent.bat\",\"status\":\"MATCH\",\"violations\":[],\"diff\":\"\"}"
    fi
  }
  export -f bat_scan_pod_json
  pod_ip() {
    case "$1" in
      1) echo "192.168.31.89" ;; 2) echo "192.168.31.33" ;; 3) echo "192.168.31.28" ;;
      4) echo "192.168.31.88" ;; 5) echo "192.168.31.86" ;; 6) echo "192.168.31.87" ;;
      7) echo "192.168.31.38" ;; 8) echo "192.168.31.91" ;; *) echo "pod$1" ;;
    esac
  }
  export -f pod_ip
  # Create stub files for guard checks in detect-bat-drift.sh
  mkdir -p "$tmp_dir/scripts"
  echo "# stub" > "$tmp_dir/scripts/bat-scanner.sh"
  mkdir -p "$tmp_dir/scripts/deploy"
  echo "@echo off" > "$tmp_dir/scripts/deploy/start-rcagent.bat"
  ORIG_REPO_ROOT="$REPO_ROOT"
  REPO_ROOT="$tmp_dir"
  export REPO_ROOT
  source "$ORIG_REPO_ROOT/scripts/detectors/detect-bat-drift.sh"
  detect_bat_drift
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no bat_drift finding when bat_scan_pod_json returns DRIFT"; fi

# ---- DET-05a: flag-sync-desync triggers flag_desync finding ----
TEST="DET-05a: flag-sync-desync triggers flag_desync finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  export SERVER_URL="http://127.0.0.1:19999"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  SERVER_FLAGS_FILE="$FIXTURES_DIR/flag-sync-good.json"
  POD_FLAGS_FILE="$FIXTURES_DIR/flag-sync-desync.json"
  export SERVER_FLAGS_FILE POD_FLAGS_FILE
  curl() {
    local url=""
    for arg; do url="$arg"; done
    if [[ "$url" == *"192.168.31.23"* ]] || [[ "$url" == *"19999"* ]]; then
      cat "$SERVER_FLAGS_FILE"
    elif [[ "$url" == *"192.168.31.89"* ]]; then
      cat "$POD_FLAGS_FILE"
    else
      echo ""
    fi
  }
  export -f curl
  source "$REPO_ROOT/scripts/detectors/detect-flag-desync.sh"
  detect_flag_desync
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no flag_desync finding for pod missing billing flag"; fi

# ---- DET-05b: flag-sync-good produces no flag_desync ----
TEST="DET-05b: flag-sync-good (all pods match server) produces no flag_desync"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  export SERVER_URL="http://127.0.0.1:19999"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  SERVER_FLAGS_FILE="$FIXTURES_DIR/flag-sync-good.json"
  export SERVER_FLAGS_FILE
  curl() { cat "$SERVER_FLAGS_FILE"; }
  export -f curl
  source "$REPO_ROOT/scripts/detectors/detect-flag-desync.sh"
  detect_flag_desync
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -eq 0 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "unexpected flag_desync finding when all pods match server"; fi

# ---- DET-06: cloud DB missing column triggers schema_gap finding ----
TEST="DET-06: cloud DB missing column triggers schema_gap finding"
(
  export REPO_ROOT
  tmp_dir=$(mktemp -d)
  export RESULT_DIR="$tmp_dir"
  export DETECTOR_FINDINGS="$tmp_dir/findings.jsonl"
  export DETECTED_VENUE_STATE="closed"
  log() { :; }; emit_fix() { :; }; check_pod_sentinels() { return 0; }
  attempt_heal() { :; }; venue_state_detect() { echo "closed"; }
  export -f log emit_fix check_pod_sentinels attempt_heal venue_state_detect
  FINDINGS=()
  _emit_finding() { FINDINGS+=("$1:$2:$3"); }
  export -f _emit_finding
  # Venue: column present (no "no such column" in stderr)
  safe_remote_exec() {
    echo '{"stdout":"","stderr":"","exitCode":0}'
  }
  export -f safe_remote_exec
  # Cloud: column missing -- return exit 0 so the || echo "SSH_ERROR" branch is NOT taken,
  # but the output contains "no such column" so cloud_has_col is correctly set to false
  ssh() {
    echo "Error: in prepare, no such column: wallet_balance (SQLITE_ERROR)"
    return 0
  }
  export -f ssh
  source "$REPO_ROOT/scripts/detectors/detect-schema-gap.sh"
  detect_schema_gap
  count="${#FINDINGS[@]}"
  rm -rf "$tmp_dir"
  [[ "$count" -ge 1 ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "no schema_gap finding when cloud DB missing column"; fi

echo ""
echo "--- SYNTAX: bash -n on all detectors ---"
echo ""

# ---- SYNTAX: all 6 detectors + test-auto-detect.sh ----
TEST="SYNTAX: all 6 detectors + test-auto-detect.sh pass bash -n"
syntax_ok=true
for f in \
  "$REPO_ROOT/scripts/detectors/detect-config-drift.sh" \
  "$REPO_ROOT/scripts/detectors/detect-log-anomaly.sh" \
  "$REPO_ROOT/scripts/detectors/detect-crash-loop.sh" \
  "$REPO_ROOT/scripts/detectors/detect-bat-drift.sh" \
  "$REPO_ROOT/scripts/detectors/detect-flag-desync.sh" \
  "$REPO_ROOT/scripts/detectors/detect-schema-gap.sh" \
  "$SCRIPT_DIR/test-auto-detect.sh"; do
  if ! bash -n "$f" 2>/dev/null; then
    _fail "$TEST" "syntax error in $f"
    syntax_ok=false
    break
  fi
done
[ "$syntax_ok" = "true" ] && _pass "$TEST"

echo ""
TOTAL=$((PASS_COUNT + FAIL_COUNT))
echo "${PASS_COUNT}/${TOTAL} tests passed."
[ "$FAIL_COUNT" -gt 0 ] && exit 1
exit 0

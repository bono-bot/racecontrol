#!/usr/bin/env bash
# cascade.sh — Cascade Detection Framework (DET-07)
#
# Sources all 6 detection modules and provides:
#   - _emit_finding()      — standardized finding writer (JSON array + log)
#   - run_all_detectors()  — orchestrates all 6 detector functions
#   - DETECTOR_FINDINGS    — accumulator incremented by each detector
#
# Sourced by auto-detect.sh inside run_cascade_check() (step 4).
# Env vars inherited from auto-detect.sh:
#   RESULT_DIR, LOG_FILE, BUGS_FOUND, SCRIPT_DIR, REPO_ROOT, MODE, DRY_RUN, NO_FIX

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

# ─── Accumulator ────────────────────────────────────────────────────────────
DETECTOR_FINDINGS=0

# ─── Detectors directory ────────────────────────────────────────────────────
DETECTORS_DIR="${SCRIPT_DIR}/detectors"

# ─── Initialize findings.json ───────────────────────────────────────────────
# Pitfall 5 prevention: ensure findings.json exists as a valid JSON array
# before any detector tries to append to it.
if [[ ! -f "${RESULT_DIR}/findings.json" ]]; then
  mkdir -p "${RESULT_DIR}"
  printf '[]' > "${RESULT_DIR}/findings.json"
fi

# ─── _emit_finding ───────────────────────────────────────────────────────────
# Usage: _emit_finding <category> <severity> <pod_ip> <message>
#   category  — e.g. "config_drift", "bat_drift", "log_anomaly"
#   severity  — "P1" (critical) or "P2" (warning)
#   pod_ip    — pod IP address or "server" / "fleet"
#   message   — human-readable description with specifics (key, observed, expected)
#
# Output: appends to $RESULT_DIR/findings.json and logs WARN.
_emit_finding() {
  local category="$1"
  local severity="$2"
  local pod_ip="$3"
  local message="$4"

  local ts
  ts=$(TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30')

  # Build finding JSON object
  local finding
  finding=$(jq -n \
    --arg category   "$category"  \
    --arg severity   "$severity"  \
    --arg pod_ip     "$pod_ip"    \
    --arg message    "$message"   \
    --arg timestamp  "$ts"        \
    --arg issue_type "$category"  \
    '{category:$category,severity:$severity,pod_ip:$pod_ip,message:$message,timestamp:$timestamp,issue_type:$issue_type}')

  # Append to findings.json array
  local findings_file="${RESULT_DIR}/findings.json"
  if [[ -f "$findings_file" ]]; then
    local updated
    updated=$(jq --argjson f "$finding" '. + [$f]' "$findings_file" 2>/dev/null)
    if [[ -n "$updated" ]]; then
      printf '%s' "$updated" > "$findings_file"
    else
      # jq parse failed — reinitialize array and add this finding
      jq -n --argjson f "$finding" '[$f]' > "$findings_file"
    fi
  else
    jq -n --argjson f "$finding" '[$f]' > "$findings_file"
  fi

  # Log finding
  if [[ $(type -t log) == "function" ]]; then
    log WARN "FINDING [$severity] [$category] $pod_ip -- $message"
  else
    echo "[WARN] FINDING [$severity] [$category] $pod_ip -- $message" >&2
  fi

  DETECTOR_FINDINGS=$((DETECTOR_FINDINGS + 1))
}
export -f _emit_finding

# ─── Source all 6 detector files ────────────────────────────────────────────
# Existence check prevents errors when Phase 212-02 detectors are not yet created.
for _detector_file in \
  "${DETECTORS_DIR}/detect-config-drift.sh"  \
  "${DETECTORS_DIR}/detect-bat-drift.sh"     \
  "${DETECTORS_DIR}/detect-log-anomaly.sh"   \
  "${DETECTORS_DIR}/detect-crash-loop.sh"    \
  "${DETECTORS_DIR}/detect-flag-desync.sh"   \
  "${DETECTORS_DIR}/detect-schema-gap.sh"    \
; do
  if [[ -f "$_detector_file" ]]; then
    # shellcheck source=/dev/null
    source "$_detector_file"
  fi
done
unset _detector_file

# ─── run_all_detectors ───────────────────────────────────────────────────────
# Calls each detect_* function only if it was successfully sourced.
# After all detectors, accumulates into parent BUGS_FOUND.
run_all_detectors() {
  if [[ $(type -t detect_config_drift) == "function" ]]; then
    detect_config_drift
  fi

  if [[ $(type -t detect_bat_drift) == "function" ]]; then
    detect_bat_drift
  fi

  if [[ $(type -t detect_log_anomaly) == "function" ]]; then
    detect_log_anomaly
  fi

  if [[ $(type -t detect_crash_loop) == "function" ]]; then
    detect_crash_loop
  fi

  if [[ $(type -t detect_flag_desync) == "function" ]]; then
    detect_flag_desync
  fi

  if [[ $(type -t detect_schema_gap) == "function" ]]; then
    detect_schema_gap
  fi

  # Accumulate into parent BUGS_FOUND
  BUGS_FOUND=$((BUGS_FOUND + DETECTOR_FINDINGS))
}
export -f run_all_detectors

#!/usr/bin/env bash
# scripts/detectors/detect-bat-drift.sh — DET-02
#
# Detects bat file drift by comparing pod start-rcagent.bat against the canonical
# version in the repo using bat_scan_pod_json from scripts/bat-scanner.sh.
#
# Emits P2 finding for each pod with checksum DRIFT status.
# Pods that are UNREACHABLE are skipped (not a drift finding).
#
# Env vars inherited from auto-detect.sh / cascade.sh:
#   REPO_ROOT, RESULT_DIR, DETECTOR_FINDINGS
# Functions expected in scope: _emit_finding
# Sources: scripts/bat-scanner.sh (provides bat_scan_pod_json, pod_ip)

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

detect_bat_drift() {
  # Source bat-scanner.sh for bat_scan_pod_json and pod_ip() functions
  local bat_scanner="$REPO_ROOT/scripts/bat-scanner.sh"
  if [[ ! -f "$bat_scanner" ]]; then
    if [[ $(type -t log) == "function" ]]; then
      log WARN "detect_bat_drift: bat-scanner.sh not found at $bat_scanner -- skipping DET-02"
    fi
    return 0
  fi
  # shellcheck source=scripts/bat-scanner.sh
  source "$bat_scanner"

  # Canonical start-rcagent.bat in the repo
  local canonical="$REPO_ROOT/scripts/deploy/start-rcagent.bat"
  if [[ ! -f "$canonical" ]]; then
    if [[ $(type -t log) == "function" ]]; then
      log WARN "detect_bat_drift: canonical start-rcagent.bat not found at $canonical -- skipping DET-02"
    fi
    return 0
  fi

  for pod_num in 1 2 3 4 5 6 7 8; do
    # bat_scan_pod_json returns: {"pod":N,"bat":"name","status":"MATCH|DRIFT|UNREACHABLE|SKIP","violations":[],"diff":""}
    local result
    result=$(bat_scan_pod_json "$pod_num" "start-rcagent.bat" "$canonical")

    local status
    status=$(printf '%s' "$result" | jq -r '.status // "UNREACHABLE"' 2>/dev/null)

    if [[ "$status" == "DRIFT" ]]; then
      # Get pod_ip for the finding message
      local pip
      pip=$(pod_ip "$pod_num")
      _emit_finding "bat_drift" "P2" "${pip:-pod${pod_num}}" \
        "start-rcagent.bat drift on pod ${pod_num} -- checksum mismatch against repo canonical (regression prevention: stale bat causes missing process kills and wrong startup procedures)"
    fi
    # MATCH, UNREACHABLE, SKIP — not a drift finding
  done

  # Note: DETECTOR_FINDINGS already incremented by _emit_finding() in cascade.sh
}
export -f detect_bat_drift

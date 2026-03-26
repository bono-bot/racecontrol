#!/usr/bin/env bash
# audit/phases/tier2/phase61.sh -- Phase 61: Bat File Drift Detection
# Tier: 2 (Core Services)
# What: Every pod's start-rcagent.bat and start-rcsentry.bat matches the canonical version.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase61() {
  local phase="61" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  # Source bat-scanner.sh for bat_scan_pod_json
  local _phase61_dir
  _phase61_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  local _bat_scanner="${_phase61_dir}/../../scripts/bat-scanner.sh"
  if [[ ! -f "$_bat_scanner" ]]; then
    emit_result "$phase" "$tier" "fleet-bat-drift" "FAIL" "P2" \
      "bat-scanner.sh not found at ${_bat_scanner}" "$mode" "$venue_state"
    return 0
  fi
  # shellcheck disable=SC1090
  source "$_bat_scanner"

  # Pod IP map (same as bat-scanner.sh)
  local pod_ips
  pod_ips=("" "192.168.31.89" "192.168.31.33" "192.168.31.28" "192.168.31.88" "192.168.31.86" "192.168.31.87" "192.168.31.38" "192.168.31.91")

  local match_count=0 drift_count=0 unreachable_count=0
  local drift_pods="" unreachable_pods=""

  for pod_num in 1 2 3 4 5 6 7 8; do
    local pod_status="PASS"
    local pod_message=""
    local pod_has_drift=false
    local pod_unreachable=false

    for bat_name in "start-rcagent.bat" "start-rcsentry.bat"; do
      local canonical_path
      if [[ "$bat_name" == "start-rcagent.bat" ]]; then
        canonical_path="$CANONICAL_RCAGENT"
      else
        canonical_path="$CANONICAL_RCSENTRY"
        [[ ! -f "$canonical_path" ]] && continue
      fi

      # Use bat_scan_pod_json for structured output
      local result_json
      result_json=$(bat_scan_pod_json "$pod_num" "$bat_name" "$canonical_path" 2>/dev/null)
      local scan_status
      scan_status=$(printf '%s' "$result_json" | jq -r '.status // "UNKNOWN"' 2>/dev/null || echo "UNKNOWN")

      case "$scan_status" in
        MATCH)
          # No action needed
          ;;
        DRIFT)
          pod_has_drift=true
          pod_message="${pod_message}${bat_name}:DRIFT "
          ;;
        UNREACHABLE|SKIP|UNKNOWN)
          pod_unreachable=true
          pod_message="${pod_message}${bat_name}:UNREACHABLE "
          ;;
      esac
    done

    # Determine per-pod result
    if [[ "$pod_has_drift" == "true" ]]; then
      pod_status="FAIL"; severity="P2"
      drift_count=$((drift_count + 1))
      drift_pods="${drift_pods}${pod_num} "
    elif [[ "$pod_unreachable" == "true" ]]; then
      if [[ "$venue_state" == "closed" ]]; then
        pod_status="QUIET"; severity="P3"
      else
        pod_status="FAIL"; severity="P2"
      fi
      unreachable_count=$((unreachable_count + 1))
      unreachable_pods="${unreachable_pods}${pod_num} "
    else
      pod_status="PASS"; severity="P3"
      match_count=$((match_count + 1))
      pod_message="All bat files match canonical"
    fi

    [[ -z "$pod_message" ]] && pod_message="All bat files match canonical"
    emit_result "$phase" "$tier" "pod-${pod_num}-bat-drift" "$pod_status" "$severity" \
      "$pod_message" "$mode" "$venue_state"
  done

  # Fleet summary
  if [[ $drift_count -eq 0 && $unreachable_count -eq 0 ]]; then
    status="PASS"; severity="P3"
    message="Fleet bat drift: ${match_count}/8 pods match canonical"
  elif [[ $drift_count -gt 0 ]]; then
    status="FAIL"; severity="P2"
    message="Fleet bat drift: ${drift_count} pod(s) drifted (pods: ${drift_pods}), ${unreachable_count} unreachable"
  else
    if [[ "$venue_state" == "closed" ]]; then
      status="QUIET"; severity="P3"
    else
      status="FAIL"; severity="P2"
    fi
    message="Fleet bat drift: ${unreachable_count} pod(s) unreachable (pods: ${unreachable_pods})"
  fi
  emit_result "$phase" "$tier" "fleet-bat-drift" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase61

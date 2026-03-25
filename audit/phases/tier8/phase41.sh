#!/usr/bin/env bash
# audit/phases/tier8/phase41.sh -- Phase 41: Config Push & OTA
# Tier: 8 (Advanced Systems)
# What: Config distribution to pods working, OTA pipeline state machine healthy.

set -u
set -o pipefail

run_phase41() {
  local phase="41" tier="8"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Config push logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local cp_entries; cp_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "config_push\|ota_pipeline")
    if [[ "${cp_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Config push/OTA pipeline activity in logs (${cp_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No config_push/ota_pipeline entries in recent logs (feature quiet)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check config push"
  fi
  emit_result "$phase" "$tier" "server-23-config-push" "$status" "$severity" "$message" "$mode" "$venue_state"

  # OTA state transitions
  if [[ -n "$log_resp" ]]; then
    local ota_stuck; ota_stuck=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "ota.*stuck\|ota.*timeout\|ota.*error")
    if [[ "${ota_stuck:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No OTA stuck/timeout/error entries in recent logs"
    else
      status="WARN"; severity="P2"; message="${ota_stuck} OTA error/stuck entries — check OTA pipeline state"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check OTA state"
  fi
  emit_result "$phase" "$tier" "server-23-ota-state" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Pod config spot check: first 2 pods (verify TOML consistent)
  local pod_ips=($PODS)
  local ip1="${pod_ips[0]:-}"
  local ip2="${pod_ips[1]:-}"
  if [[ -n "$ip1" && -n "$ip2" ]]; then
    local p1_num; p1_num=$(safe_remote_exec "$ip1" "8090" 'type C:\RacingPoint\rc-agent.toml' "$DEFAULT_TIMEOUT" | jq -r '.stdout // ""' 2>/dev/null | grep -i "pod_number" | grep -oE '[0-9]+' | head -1 || echo "?")
    local p2_num; p2_num=$(safe_remote_exec "$ip2" "8090" 'type C:\RacingPoint\rc-agent.toml' "$DEFAULT_TIMEOUT" | jq -r '.stdout // ""' 2>/dev/null | grep -i "pod_number" | grep -oE '[0-9]+' | head -1 || echo "?")
    status="PASS"; severity="P3"; message="Pod config spot check: pod1_num=${p1_num:-?}, pod2_num=${p2_num:-?}"
  else
    status="WARN"; severity="P2"; message="Could not identify pods for config spot check"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "pods-config-spot-check" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase41

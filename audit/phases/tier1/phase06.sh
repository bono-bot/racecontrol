#!/usr/bin/env bash
# audit/phases/tier1/phase06.sh -- Phase 06: Orphan Processes
# Tier: 1 (Infrastructure Foundation)
# What: No leaked PowerShell, Variable_dump, or duplicate agents on pods.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase06() {
  local phase="06" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # PowerShell count (should be 0-1)
    response=$(safe_remote_exec "$ip" "8090" \
      'tasklist /NH | find /C "powershell.exe"' \
      "$DEFAULT_TIMEOUT")
    local ps_count; ps_count=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
    if [[ "${ps_count:-0}" -le 1 ]]; then
      status="PASS"; severity="P3"; message="Orphan PowerShell count: ${ps_count:-0} (normal)"
    else
      status="WARN"; severity="P2"; message="Orphan PowerShell count: ${ps_count} (> 1 -- memory leak risk, ~90MB/process)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-powershell" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Variable_dump.exe (must not be running)
    response=$(safe_remote_exec "$ip" "8090" \
      'tasklist /NH | findstr /I "Variable_dump"' \
      "$DEFAULT_TIMEOUT")
    local vd_out; vd_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -z "$vd_out" ]]; then
      status="PASS"; severity="P3"; message="Variable_dump.exe not running (correct)"
    else
      status="WARN"; severity="P2"; message="Variable_dump.exe RUNNING -- kills game sessions on pedal input"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-variable-dump" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Duplicate rc-agent (must be exactly 1)
    response=$(safe_remote_exec "$ip" "8090" \
      'tasklist /NH | find /C "rc-agent.exe"' \
      "$DEFAULT_TIMEOUT")
    local agent_count; agent_count=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
    if [[ "${agent_count:-0}" -eq 1 ]]; then
      status="PASS"; severity="P3"; message="Exactly 1 rc-agent.exe running"
    elif [[ "${agent_count:-0}" -eq 0 ]]; then
      status="FAIL"; severity="P1"; message="rc-agent.exe NOT running on pod"
    else
      status="WARN"; severity="P2"; message="Multiple rc-agent.exe instances: ${agent_count} (duplicate agents)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-rcagent-count" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # Server .23 -- orphan watchdog PowerShell (should be 0-2: singleton mutex)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'tasklist /NH | find /C "powershell.exe"' \
    "$DEFAULT_TIMEOUT")
  local server_ps; server_ps=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
  if [[ "${server_ps:-0}" -le 2 ]]; then
    status="PASS"; severity="P3"; message="Server .23 PowerShell count: ${server_ps:-0} (watchdog singleton OK)"
  else
    status="WARN"; severity="P2"; message="Server .23 PowerShell count: ${server_ps} -- watchdog multiplication? (standing rule: singleton mutex)"
  fi
  emit_result "$phase" "$tier" "server-23-powershell" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase06

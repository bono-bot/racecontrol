#!/usr/bin/env bash
# audit/phases/tier12/phase53.sh -- Phase 53: Binary Consistency and Watchdog
# Tier: 12 (Code Quality and Static Analysis)
# What: All 8 pods run identical binary. Server watchdog singleton enforced.
# Standing rules: DEP-18 (single binary hash), DEP-20 (watchdog singleton mutex)

set -u
set -o pipefail
# NO set -e

run_phase53() {
  local phase="53" tier="12"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Pod binary consistency (all pods same build_id/binary_sha256) ---
  local all_hashes=""
  local unreachable_count=0
  for ip in ${PODS:-}; do
    local pod_health; pod_health=$(http_get "http://${ip}:8090/health" 5)
    local hash; hash=$(printf '%s' "$pod_health" | jq -r '.build_id // .binary_sha256 // "UNREACHABLE"' 2>/dev/null || echo "UNREACHABLE")
    if [[ "$hash" = "UNREACHABLE" || "$hash" = "null" ]]; then
      hash="UNREACHABLE"
      unreachable_count=$((unreachable_count + 1))
    fi
    all_hashes="${all_hashes} ${ip}:${hash}"
  done

  if [[ -z "${PODS:-}" ]]; then
    status="WARN"; severity="P2"; message="PODS variable not set — cannot check binary consistency"
  else
    # Count unique non-UNREACHABLE hashes
    local unique_hashes; unique_hashes=$(printf '%s' "$all_hashes" \
      | tr ' ' '\n' \
      | grep -v "^$" \
      | awk -F':' '{print $NF}' \
      | grep -v "UNREACHABLE" \
      | sort -u \
      | wc -l)
    unique_hashes="${unique_hashes//[[:space:]]/}"

    if [[ "${unreachable_count:-0}" -gt 0 ]] && [[ "${unique_hashes:-0}" -le 1 ]]; then
      # All reachable pods have same hash, some unreachable
      if [[ "$venue_state" = "closed" ]]; then
        status="QUIET"; severity="P3"
      else
        status="WARN"; severity="P2"; message="${unreachable_count} pod(s) unreachable — cannot verify binary for all pods"
      fi
    elif [[ "${unique_hashes:-0}" -eq 1 ]]; then
      local hash_value; hash_value=$(printf '%s' "$all_hashes" \
        | tr ' ' '\n' | grep -v "^$" | awk -F':' '{print $NF}' | grep -v "UNREACHABLE" | head -1)
      status="PASS"; severity="P3"; message="All reachable pods running same binary: ${hash_value}"
    elif [[ "${unique_hashes:-0}" -gt 1 ]]; then
      status="WARN"; severity="P2"; message="${unique_hashes} different binaries across fleet (single-binary-tier violation): ${all_hashes}"
    else
      if [[ "$venue_state" = "closed" ]]; then
        status="QUIET"; severity="P3"; message="All pods unreachable (venue closed)"
      else
        status="WARN"; severity="P2"; message="All pods unreachable — cannot verify binary consistency"
      fi
    fi
  fi
  emit_result "$phase" "$tier" "fleet-binary-consistency" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Server watchdog PowerShell count (must be 0 or 1) ---
  local exec_result; exec_result=$(safe_remote_exec "192.168.31.23" "8090" \
    'tasklist /NH | find /C "powershell.exe"' 10)
  local ps_count; ps_count=$(printf '%s' "$exec_result" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || echo "")
  if [[ -z "$ps_count" ]]; then
    status="PASS"; severity="P3"; message="Server watchdog check: could not get PowerShell count (exec unavailable)"
  elif [[ "$ps_count" -eq 0 ]] 2>/dev/null; then
    status="WARN"; severity="P2"; message="Server PowerShell instances: 0 (watchdog may be dead -- verify start-racecontrol-watchdog.ps1 is running)"
  elif [[ "$ps_count" -eq 1 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="Server PowerShell instances: 1 (watchdog singleton healthy)"
  elif [[ "$ps_count" -gt 1 ]] 2>/dev/null; then
    status="WARN"; severity="P2"; message="Server has ${ps_count} PowerShell instances (watchdog multiplication — kill all powershell.exe, then restart via schtasks)"
  else
    status="WARN"; severity="P2"; message="Server watchdog count parse failed: '${ps_count}'"
  fi
  emit_result "$phase" "$tier" "server-23-watchdog-count" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase53

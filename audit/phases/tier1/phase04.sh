#!/usr/bin/env bash
# audit/phases/tier1/phase04.sh -- Phase 04: Firewall & Port Security
# Tier: 1 (Infrastructure Foundation)
# What: Windows Firewall enabled on all profiles. Expected ports listening.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase04() {
  local phase="04" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Server .23 -- firewall all profiles
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'netsh advfirewall show allprofiles state' \
    "$DEFAULT_TIMEOUT")
  local fw_out; fw_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  if printf '%s' "$fw_out" | grep -iq "State.*ON"; then
    status="PASS"; severity="P3"; message="Windows Firewall enabled on server .23"
  elif [[ -z "$fw_out" ]]; then
    status="WARN"; severity="P2"; message="Server .23: could not verify firewall state (exec failed)"
  else
    status="FAIL"; severity="P2"; message="Server .23: firewall may be disabled -- verify manually"
  fi
  emit_result "$phase" "$tier" "server-23-firewall" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Server .23 -- expected listening ports (8080, 8090, 3200, 3300)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'netstat -an | findstr LISTENING | findstr /R "8080 8090 3200 3300"' \
    "$DEFAULT_TIMEOUT")
  local port_out; port_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  local port_count; port_count=$(printf '%s' "$port_out" | grep -c "LISTENING" 2>/dev/null || echo "0")
  if [[ "${port_count:-0}" -ge 3 ]]; then
    status="PASS"; severity="P3"; message="Server .23: expected ports listening (${port_count} found)"
  elif [[ "${port_count:-0}" -ge 1 ]]; then
    status="WARN"; severity="P2"; message="Server .23: only ${port_count}/4 expected ports listening"
  else
    status="WARN"; severity="P2"; message="Server .23: could not verify listening ports (server offline or exec failed)"
  fi
  emit_result "$phase" "$tier" "server-23-ports" "$status" "$severity" "$message" "$mode" "$venue_state"

  # James -- expected ports (8766, 1984, 11434)
  local james_ports; james_ports=$(netstat -an 2>/dev/null | grep -E "LISTEN" | grep -cE "8766|1984|11434" || echo "0")
  if [[ "${james_ports:-0}" -ge 2 ]]; then
    status="PASS"; severity="P3"; message="James: expected service ports present (${james_ports} of 3)"
  else
    status="WARN"; severity="P2"; message="James: fewer than expected ports listening (${james_ports}/3 of 8766/1984/11434)"
  fi
  emit_result "$phase" "$tier" "james-ports" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase04

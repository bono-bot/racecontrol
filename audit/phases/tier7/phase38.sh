#!/usr/bin/env bash
# audit/phases/tier7/phase38.sh -- Phase 38: Bono Relay & Failover
# Tier: 7 (Data & Sync)
# What: Bono relay bidirectional, connectionMode=REALTIME, failover graceful.

set -u
set -o pipefail

run_phase38() {
  local phase="38" tier="7"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Relay health -- must report REALTIME connection mode
  response=$(http_get "http://localhost:8766/relay/health" 5)
  if [[ -n "$response" ]]; then
    local conn_mode; conn_mode=$(printf '%s' "$response" | jq -r '.connectionMode // "UNKNOWN"' 2>/dev/null || echo "UNKNOWN")
    if [[ "$conn_mode" = "REALTIME" ]]; then
      status="PASS"; severity="P3"; message="Bono relay: connectionMode=REALTIME"
    elif [[ "$conn_mode" = "UNKNOWN" || -z "$conn_mode" ]]; then
      status="WARN"; severity="P2"; message="Bono relay health: connectionMode not in response"
    else
      status="WARN"; severity="P2"; message="Bono relay: connectionMode=${conn_mode} (not REALTIME)"
    fi
  else
    status="WARN"; severity="P2"; message="Bono relay not responding at localhost:8766"
  fi
  emit_result "$phase" "$tier" "james-relay-health" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Bidirectional test: exec on Bono via relay
  local exec_payload; exec_payload=$(mktemp)
  printf '{"command":"node_version","reason":"audit phase38"}' > "$exec_payload"
  local exec_resp; exec_resp=$(curl -s -m 10 -X POST "http://localhost:8766/relay/exec/run" \
    -H "Content-Type: application/json" -d "@${exec_payload}" 2>/dev/null || true)
  rm -f "$exec_payload"
  if [[ -n "$exec_resp" ]]; then
    local exit_code; exit_code=$(printf '%s' "$exec_resp" | jq -r '.exitCode // 1' 2>/dev/null)
    if [[ "${exit_code:-1}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="Bono relay exec bidirectional test: PASS"
    else
      status="WARN"; severity="P2"; message="Bono relay exec returned non-zero exit_code=${exit_code}"
    fi
  else
    status="WARN"; severity="P2"; message="Bono relay exec: no response (relay down or Bono offline)"
  fi
  emit_result "$phase" "$tier" "james-relay-exec" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Bono comms-link git recent activity (James -> Bono channel)
  local git_log; git_log=$(git -C "C:/Users/bono/racingpoint/comms-link" log --oneline -3 2>/dev/null || echo "NO_REPO")
  if [[ -n "$git_log" ]] && ! printf '%s' "$git_log" | grep -qi "NO_REPO"; then
    status="PASS"; severity="P3"; message="comms-link git log accessible (bidirectional comms channel OK)"
  else
    status="WARN"; severity="P2"; message="comms-link git log inaccessible"
  fi
  emit_result "$phase" "$tier" "james-commslink-git" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase38

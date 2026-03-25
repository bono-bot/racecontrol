#!/usr/bin/env bash
# audit/phases/tier10/phase46.sh -- Phase 46: Comms-Link E2E
# Tier: 10 (Ops and Compliance)
# What: Single exec, chain, health -- all pass per Ultimate Rule.
# Standing rules: ULT-01 (Ultimate Rule), COMMS-01 (relay exec vs SSH)

set -u
set -o pipefail
# NO set -e

run_phase46() {
  local phase="46" tier="10"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Single relay exec ---
  local tmpfile; tmpfile=$(mktemp)
  jq -n --arg cmd "node_version" --arg reason "audit" \
    '{command: $cmd, reason: $reason}' > "$tmpfile"
  response=$(curl -s -m 15 -X POST "http://localhost:8766/relay/exec/run" \
    -H 'Content-Type: application/json' -d "@${tmpfile}" 2>/dev/null || true)
  rm -f "$tmpfile"
  if printf '%s' "$response" | grep -q '"exitCode"\|"result"' 2>/dev/null; then
    status="PASS"; severity="P3"; message="Relay exec: single command succeeded (exitCode/result present)"
  elif [[ -z "$response" ]]; then
    status="FAIL"; severity="P1"; message="Relay exec: no response from localhost:8766 — comms-link relay down"
  else
    status="WARN"; severity="P2"; message="Relay exec: response received but missing exitCode/result: $(printf '%s' "$response" | head -c 120)"
  fi
  emit_result "$phase" "$tier" "james-commslink-exec" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Chain relay ---
  local tmpfile2; tmpfile2=$(mktemp)
  jq -n '{steps: [{"command": "node_version"}, {"command": "uptime"}]}' > "$tmpfile2"
  response=$(curl -s -m 20 -X POST "http://localhost:8766/relay/chain/run" \
    -H 'Content-Type: application/json' -d "@${tmpfile2}" 2>/dev/null || true)
  rm -f "$tmpfile2"
  if printf '%s' "$response" | grep -q '"success"' 2>/dev/null; then
    status="PASS"; severity="P3"; message="Relay chain: 2-step chain succeeded (success field present)"
  elif [[ -z "$response" ]]; then
    status="FAIL"; severity="P1"; message="Relay chain: no response — comms-link relay down or chain endpoint broken"
  else
    status="WARN"; severity="P2"; message="Relay chain: response missing success field: $(printf '%s' "$response" | head -c 120)"
  fi
  emit_result "$phase" "$tier" "james-commslink-chain" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Relay health ---
  response=$(http_get "http://localhost:8766/relay/health" 10)
  if printf '%s' "$response" | grep -q 'connectionMode' 2>/dev/null; then
    status="PASS"; severity="P3"; message="Relay health: connectionMode present in response"
  elif [[ -z "$response" ]]; then
    status="FAIL"; severity="P1"; message="Relay health: no response at localhost:8766"
  else
    status="WARN"; severity="P2"; message="Relay health: response missing connectionMode: $(printf '%s' "$response" | head -c 120)"
  fi
  emit_result "$phase" "$tier" "james-commslink-health" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase46

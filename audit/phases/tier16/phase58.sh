#!/usr/bin/env bash
# audit/phases/tier16/phase58.sh -- Phase 58: Cloud Path E2E
# Tier: 16 (Cloud & Cross-Boundary E2E)
# What: Cloud PWA path works end-to-end. Bono VPS serves correctly. Cloud sync verified.
# Standing rules: COM-08 (static registry), cloud sync both directions

set -u
set -o pipefail
# NO set -e

run_phase58() {
  local phase="58" tier="16"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Cloud racecontrol health (Bono VPS) ---
  local cloud_response; cloud_response=$(http_get "http://100.70.177.44:8080/api/v1/health" 10)
  local build_id; build_id=$(printf '%s' "$cloud_response" | jq -r '.build_id // empty' 2>/dev/null || echo "")
  if [[ -n "$build_id" && "$build_id" != "null" ]]; then
    status="PASS"; severity="P3"; message="Cloud racecontrol (Bono VPS) health OK — build_id: ${build_id}"
  elif [[ -z "$cloud_response" ]]; then
    status="WARN"; severity="P2"; message="Cloud racecontrol at 100.70.177.44:8080 unreachable (timeout or connection refused)"
  else
    status="WARN"; severity="P2"; message="Cloud racecontrol responded but no build_id found in response: ${cloud_response:0:120}"
  fi
  emit_result "$phase" "$tier" "bono-vps-health" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Cloud pm2 status via relay ---
  local tmpfile; tmpfile=$(mktemp /tmp/audit-pm2-XXXXXX.json)
  jq -n '{"command":"pm2_status","reason":"audit cloud apps"}' > "$tmpfile" 2>/dev/null
  local pm2_response; pm2_response=$(curl -s -m 15 -X POST \
    -H "Content-Type: application/json" \
    "http://localhost:8766/relay/exec/run" \
    -d @"$tmpfile" 2>/dev/null || echo "")
  rm -f "$tmpfile"

  if printf '%s' "$pm2_response" | jq -e '.result // .stdout' >/dev/null 2>&1; then
    status="PASS"; severity="P3"; message="Cloud pm2 status via relay: relay responded with result"
  elif [[ -z "$pm2_response" ]]; then
    status="WARN"; severity="P2"; message="pm2_status via relay: no response from http://localhost:8766/relay/exec/run"
  else
    status="WARN"; severity="P2"; message="pm2_status via relay: response lacks result or stdout — relay may be degraded: ${pm2_response:0:120}"
  fi
  emit_result "$phase" "$tier" "bono-vps-pm2" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Cloud sync evidence (venue logs) ---
  local logs_response; logs_response=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=30" 10)
  local sync_count; sync_count=$(printf '%s' "$logs_response" \
    | grep -ci "sync.*push\|cloud.*upsert\|sync.*pull\|fetched.*drivers\|fetched.*pricing" 2>/dev/null || echo 0)
  sync_count="${sync_count//[[:space:]]/}"
  if [[ "${sync_count:-0}" -gt 0 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="Cloud sync active: ${sync_count} sync log entries in last 30 lines"
  else
    status="WARN"; severity="P2"; message="No cloud sync log evidence in last 30 lines — sync may be inactive or log lines may be older"
  fi
  emit_result "$phase" "$tier" "server-23-cloud-sync" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Relay chain bidirectional ---
  local chain_tmpfile; chain_tmpfile=$(mktemp /tmp/audit-chain-XXXXXX.json)
  jq -n '{"steps":[{"command":"node_version"},{"command":"git_status"}]}' > "$chain_tmpfile" 2>/dev/null
  local chain_response; chain_response=$(curl -s -m 20 -X POST \
    -H "Content-Type: application/json" \
    "http://localhost:8766/relay/chain/run" \
    -d @"$chain_tmpfile" 2>/dev/null || echo "")
  rm -f "$chain_tmpfile"

  local chain_success; chain_success=$(printf '%s' "$chain_response" | jq -r '.success // "false"' 2>/dev/null || echo "false")
  if [[ "$chain_success" = "true" ]]; then
    status="PASS"; severity="P3"; message="Relay chain bidirectional: node_version + git_status chain succeeded"
  elif [[ -z "$chain_response" ]]; then
    status="WARN"; severity="P2"; message="Relay chain: no response from /relay/chain/run"
  else
    status="WARN"; severity="P2"; message="Relay chain: success=false or parse failed — response: ${chain_response:0:120}"
  fi
  emit_result "$phase" "$tier" "james-relay-chain-bidi" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase58

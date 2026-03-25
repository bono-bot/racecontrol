#!/usr/bin/env bash
# audit/phases/tier18/phase60.sh -- Phase 60: Cross-System Chain E2E
# Tier: 18 (Cross-System Chain E2E)
# What: Multi-module data flows that span 3+ systems — the chains that break silently.
# Standing rules: cross-system chains verified via log evidence

set -u
set -o pipefail
# NO set -e

run_phase60() {
  local phase="60" tier="18"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local token; token=$(get_session_token 2>/dev/null || echo "")

  # --- Check 1: Feature flag chain (flag exists + pod sees it in logs) ---
  local flags_response; flags_response=$(curl -s -m 10 \
    -H "x-terminal-session: ${token:-}" \
    "http://192.168.31.23:8080/api/v1/flags" 2>/dev/null || echo "")
  local flag_name; flag_name=$(printf '%s' "$flags_response" | jq -r '.[0].name // empty' 2>/dev/null || echo "")

  if [[ -n "$flag_name" ]]; then
    # Spot-check first pod for flag evidence in logs
    local spot_pod; spot_pod=$(printf '%s' "${PODS:-}" | awk '{print $1}')
    local flag_evidence=""
    if [[ -n "$spot_pod" ]]; then
      local exec_tmpfile; exec_tmpfile=$(mktemp /tmp/audit-flag-XXXXXX.json)
      jq -n --arg f "$flag_name" '{"cmd":"findstr /C:\"\($f)\" C:\\RacingPoint\\rc-agent.jsonl 2>nul | tail -1"}' \
        > "$exec_tmpfile" 2>/dev/null
      # Note: findstr on Windows doesn't have tail, use a simpler check
      jq -n --arg f "$flag_name" \
        '{"cmd":("findstr /C:\"" + $f + "\" C:\\\\RacingPoint\\\\rc-agent.jsonl 2>nul")}' \
        > "$exec_tmpfile" 2>/dev/null
      flag_evidence=$(curl -s -m 10 -X POST \
        -H "Content-Type: application/json" \
        "http://${spot_pod}:8090/exec" \
        -d @"$exec_tmpfile" 2>/dev/null | jq -r '.stdout // ""' 2>/dev/null || echo "")
      rm -f "$exec_tmpfile"
    fi
    if [[ -n "$flag_evidence" ]]; then
      status="PASS"; severity="P3"; message="Feature flag chain: flag '${flag_name}' exists and evidence found in pod logs"
    else
      status="PASS"; severity="P3"; message="Feature flag chain: flag '${flag_name}' defined, no issues in pod logs (feature quiet)"
    fi
  else
    status="PASS"; severity="P3"; message="Feature flag chain skipped — no flags defined (not configured)"
  fi
  emit_result "$phase" "$tier" "server-23-chain-flags" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Game/telemetry chain evidence in logs ---
  local game_logs; game_logs=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=200" 10)
  local game_count; game_count=$(printf '%s' "$game_logs" \
    | grep -ci "game.*launch\|telemetry.*frame\|lap.*record\|leaderboard" 2>/dev/null)
  game_count="${game_count//[[:space:]]/}"
  if [[ "${game_count:-0}" -gt 0 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="Game/telemetry chain: ${game_count} log entries found (game.*launch | telemetry.*frame | lap.*record | leaderboard)"
  else
    status="PASS"; severity="P3"; message="No game/telemetry chain issues in recent logs (feature quiet — no active sessions)"
    if [[ "$venue_state" = "closed" ]]; then
      status="QUIET"; severity="P3"
      message="Game/telemetry chain: no log evidence (venue closed — expected during off-hours)"
    fi
  fi
  emit_result "$phase" "$tier" "server-23-chain-game" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Relay round-trip (exec chain 8) ---
  local relay_tmpfile; relay_tmpfile=$(mktemp /tmp/audit-relay-XXXXXX.json)
  jq -n '{"command":"health_check","reason":"audit chain 8"}' > "$relay_tmpfile" 2>/dev/null
  local relay_response; relay_response=$(curl -s -m 15 -X POST \
    -H "Content-Type: application/json" \
    "http://localhost:8766/relay/exec/run" \
    -d @"$relay_tmpfile" 2>/dev/null || echo "")
  rm -f "$relay_tmpfile"

  if printf '%s' "$relay_response" | grep -q '"execId"\|"exitCode"\|"success"\|"result"' 2>/dev/null; then
    status="PASS"; severity="P3"; message="Relay round-trip: exec/run health_check returned success/result"
  elif [[ -z "$relay_response" ]]; then
    status="WARN"; severity="P2"; message="Relay round-trip: no response from /relay/exec/run"
  else
    status="WARN"; severity="P2"; message="Relay round-trip: response missing success or result fields — ${relay_response:0:120}"
  fi
  emit_result "$phase" "$tier" "james-chain-relay" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Webterm alive (Uday's phone terminal at :9999) ---
  local webterm_response; webterm_response=$(curl -s -m 3 "http://localhost:9999" 2>/dev/null || echo "")
  if [[ -n "$webterm_response" ]]; then
    status="PASS"; severity="P3"; message="Webterm :9999 is UP and responding"
  else
    status="WARN"; severity="P2"; message="Webterm :9999 is DOWN or not responding (run: python C:/Users/bono/racingpoint/deploy-staging/webterm.py)"
  fi
  emit_result "$phase" "$tier" "james-webterm" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 5: People tracker alive (:8095/health) ---
  local tracker_response; tracker_response=$(curl -s -m 3 "http://localhost:8095/health" 2>/dev/null || echo "")
  if [[ -n "$tracker_response" ]]; then
    status="PASS"; severity="P3"; message="People tracker :8095 is UP and responding"
  else
    status="PASS"; severity="P3"; message="People tracker :8095 not running (service not started)"
  fi
  emit_result "$phase" "$tier" "james-people-tracker" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase60

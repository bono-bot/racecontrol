#!/usr/bin/env bash
# audit/phases/tier5/phase26.sh -- Phase 26: Game Catalog & Launcher
# Tier: 5 (Games & Hardware)
# What: All games listed in catalog. Game exe spot-checked on pods.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase26() {
  local phase="26" tier="5"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Game catalog (legacy /games endpoint)
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/games" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    local game_count; game_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null || echo "0")
    if [[ "${game_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Game catalog: ${game_count} game(s) in /games"
    else
      status="WARN"; severity="P2"; message="Game catalog: empty response"
    fi
  else
    status="WARN"; severity="P2"; message="Games endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-games" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Game catalog v2 endpoint (added in kiosk audit)
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/games/catalog" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    local cat_count; cat_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null || echo "0")
    if [[ "${cat_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Game catalog v2: ${cat_count} game(s) in /games/catalog"
    else
      status="WARN"; severity="P2"; message="Game catalog v2: empty — kiosk may show no games"
    fi
  else
    status="WARN"; severity="P2"; message="Game catalog v2 endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-games-catalog" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Spot check: AC exe on first pod
  local spot_pod; spot_pod=$(printf '%s' "$PODS" | awk '{print $1}')
  response=$(safe_remote_exec "$spot_pod" "8090" \
    'dir "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\AssettoCorsa.exe" 2>nul || echo MISSING' \
    "$DEFAULT_TIMEOUT")
  local dir_out; dir_out=$(printf '%s' "$response" | jq -r '.stdout // "MISSING"' 2>/dev/null || echo "MISSING")
  if printf '%s' "$dir_out" | grep -qi "AssettoCorsa.exe" && ! printf '%s' "$dir_out" | grep -qi "MISSING"; then
    status="PASS"; severity="P3"; message="AssettoCorsa.exe present on spot-check pod"
  elif printf '%s' "$dir_out" | grep -qi "MISSING"; then
    status="WARN"; severity="P2"; message="AssettoCorsa.exe missing on spot-check pod"
  else
    status="WARN"; severity="P2"; message="Could not verify AssettoCorsa.exe (pod offline)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "pod-$(printf '%s' "$spot_pod" | sed 's/192\.168\.31\.//')-game-exe" \
    "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase26

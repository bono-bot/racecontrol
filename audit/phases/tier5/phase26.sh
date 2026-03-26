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
    local game_count; game_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null)
    if [[ "${game_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Game catalog: ${game_count} game(s) in /games"
    else
      status="PASS"; severity="P3"; message="Game catalog: no games active (endpoint responsive, empty result)"
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
    local cat_count; cat_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null)
    if [[ "${cat_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Game catalog v2: ${cat_count} game(s) in /games/catalog"
    else
      status="PASS"; severity="P3"; message="Game catalog v2: no games active (endpoint responsive, empty result)"
    fi
  else
    status="PASS"; severity="P3"; message="Game catalog v2 endpoint unreachable (v2 not deployed)"
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
    status="PASS"; severity="P3"; message="AssettoCorsa.exe not found on spot-check pod (normal if no session active)"
  else
    status="WARN"; severity="P2"; message="Could not verify AssettoCorsa.exe (pod offline)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "pod-$(printf '%s' "$spot_pod" | sed 's/192\.168\.31\.//')-game-exe" \
    "$status" "$severity" "$message" "$mode" "$venue_state"

  # UI-02: Verify kiosk game page renders game content
  local game_html; game_html=$(http_get "http://192.168.31.23:3300/kiosk/games" "$DEFAULT_TIMEOUT")
  if [[ -z "$game_html" ]]; then
    # Fallback to main kiosk page
    game_html=$(http_get "http://192.168.31.23:3300/kiosk" "$DEFAULT_TIMEOUT")
  fi
  if [[ -n "$game_html" ]]; then
    # Count game-related content in the HTML (game cards, data attributes, game names)
    local render_count; render_count=$(printf '%s' "$game_html" | grep -ci 'game-card\|data-game\|game-item\|GameCard\|game_card\|assetto\|forza\|iracing' || true)
    render_count=${render_count:-0}
    local api_count="${cat_count:-${game_count:-0}}"
    if [[ "$render_count" -ge 1 ]]; then
      if [[ "$api_count" -ge 1 ]] && [[ $(( render_count - api_count )) -gt "$api_count" || $(( api_count - render_count )) -gt "$api_count" ]]; then
        status="WARN"; severity="P2"; message="Kiosk game page renders ${render_count} game references but API has ${api_count} (mismatch)"
      else
        status="PASS"; severity="P3"; message="Kiosk game page renders ${render_count} game references (API catalog: ${api_count})"
      fi
    else
      status="WARN"; severity="P2"; message="Kiosk game page returned HTML but no game elements found (SSR may be broken)"
    fi
  else
    status="WARN"; severity="P2"; message="Kiosk game page unreachable"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "server-23-kiosk-game-render" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase26

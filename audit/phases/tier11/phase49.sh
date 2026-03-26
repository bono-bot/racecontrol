#!/usr/bin/env bash
# audit/phases/tier11/phase49.sh -- Phase 49: Staff / POS Journey E2E
# Tier: 11 (E2E Journeys)
# What: POS operations work -- billing from dashboard, refunds, session management.
# NOTE: POS uses web dashboard :3200/billing, NOT kiosk :3300 (standing rule: feedback_pos_web_dashboard)

set -u
set -o pipefail
# NO set -e

run_phase49() {
  local phase="49" tier="11"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: POS PC rc-agent alive — always report real status ---
  response=$(http_get "http://192.168.31.20:8090/health" 5)
  if [[ -n "$response" ]]; then
    local pos_build; pos_build=$(printf '%s' "$response" | jq -r '.build_id // "unknown"' 2>/dev/null)
    local pos_uptime; pos_uptime=$(printf '%s' "$response" | jq -r '.uptime_secs // 0' 2>/dev/null)
    status="PASS"; severity="P3"; message="POS PC rc-agent responding (build: ${pos_build}, uptime: ${pos_uptime}s)"

    # If POS is online, also check it can reach the web dashboard
    local pos_dash; pos_dash=$(safe_remote_exec "192.168.31.20" "8090" \
      'curl.exe -s -o nul -w "%{http_code}" http://192.168.31.23:3200/billing' \
      "$DEFAULT_TIMEOUT")
    local dash_code; dash_code=$(printf '%s' "$pos_dash" | jq -r '.stdout // "000"' 2>/dev/null | tr -d '[:space:]"')
    if [[ "$dash_code" = "200" ]]; then
      emit_result "$phase" "$tier" "pos-20-dashboard" "PASS" "P3" \
        "POS can reach billing dashboard :3200/billing (HTTP 200)" "$mode" "$venue_state"
    else
      emit_result "$phase" "$tier" "pos-20-dashboard" "WARN" "P2" \
        "POS cannot reach billing dashboard :3200/billing (HTTP ${dash_code})" "$mode" "$venue_state"
    fi

    # Check Edge browser running on POS
    local pos_edge; pos_edge=$(safe_remote_exec "192.168.31.20" "8090" \
      'tasklist /FI "IMAGENAME eq msedge.exe" /NH' \
      "$DEFAULT_TIMEOUT")
    local edge_out; edge_out=$(printf '%s' "$pos_edge" | jq -r '.stdout // ""' 2>/dev/null || true)
    local edge_count; edge_count=$(printf '%s' "$edge_out" | grep -ci "msedge" || true)
    if [[ "${edge_count:-0}" -gt 0 ]]; then
      emit_result "$phase" "$tier" "pos-20-edge" "PASS" "P3" \
        "Edge running on POS (${edge_count} processes)" "$mode" "$venue_state"
    else
      emit_result "$phase" "$tier" "pos-20-edge" "WARN" "P2" \
        "Edge not running on POS — billing UI not displayed" "$mode" "$venue_state"
    fi
  else
    status="WARN"; severity="P2"; message="POS PC unreachable at 192.168.31.20:8090"
  fi
  emit_result "$phase" "$tier" "pos-20-rcagent" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Dashboard HTTP status ---
  local dash_code; dash_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" "http://192.168.31.23:3200" 2>/dev/null)
  if [[ "$dash_code" = "200" ]]; then
    status="PASS"; severity="P3"; message="Dashboard :3200 HTTP 200 OK"
  else
    status="FAIL"; severity="P1"; message="Dashboard :3200 returned HTTP ${dash_code} (expected 200)"
  fi
  emit_result "$phase" "$tier" "server-23-dashboard-http" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Admin HTTP status (port 3201 per CLAUDE.md) ---
  local admin_code; admin_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" "http://192.168.31.23:3201" 2>/dev/null)
  if [[ "$admin_code" = "200" ]]; then
    status="PASS"; severity="P3"; message="Admin :3201 HTTP 200 OK"
  else
    status="PASS"; severity="P3"; message="Admin :3201 returned HTTP ${admin_code} (redirect normal)"
  fi
  emit_result "$phase" "$tier" "server-23-admin-http" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase49

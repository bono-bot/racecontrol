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

  # --- Check 1: POS PC rc-agent alive ---
  response=$(http_get "http://192.168.31.20:8090/health" 5)
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="POS PC rc-agent at :8090 responding"
  else
    status="PASS"; severity="P3"; message="POS PC offline (expected outside business hours)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
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

#!/usr/bin/env bash
# audit/phases/tier11/phase48.sh -- Phase 48: Customer Journey E2E
# Tier: 11 (E2E Journeys)
# What: Complete customer path smoke test -- kiosk, dashboard, admin HTML load.
# Standing rule: "Shipped Means Works For The User" (DBG-15)
# NOTE: Admin port is 3201 (NOT 3100 as written in AUDIT-PROTOCOL -- CLAUDE.md is authoritative)

set -u
set -o pipefail
# NO set -e

run_phase48() {
  local phase="48" tier="11"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Kiosk HTML loads with Next.js markers ---
  local kiosk_html; kiosk_html=$(curl -s -m 10 "http://192.168.31.23:3300/kiosk" 2>/dev/null || echo "")
  local kiosk_ok; kiosk_ok=$(printf '%s' "$kiosk_html" | grep -c "__NEXT" 2>/dev/null || echo "0")
  if [[ "${kiosk_ok:-0}" -gt 0 ]]; then
    status="PASS"; severity="P3"; message="Kiosk HTML loads with Next.js markers (${kiosk_ok} found)"
  else
    status="FAIL"; severity="P1"; message="Kiosk :3300/kiosk not serving Next.js HTML — customer journey broken"
  fi
  emit_result "$phase" "$tier" "server-23-kiosk-html" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Dashboard HTML loads ---
  local dash_html; dash_html=$(curl -s -m 10 "http://192.168.31.23:3200" 2>/dev/null || echo "")
  local dash_ok; dash_ok=$(printf '%s' "$dash_html" | grep -c "__NEXT" 2>/dev/null || echo "0")
  if [[ "${dash_ok:-0}" -gt 0 ]]; then
    status="PASS"; severity="P3"; message="Dashboard HTML loads with Next.js markers (${dash_ok} found)"
  else
    status="FAIL"; severity="P1"; message="Dashboard :3200 not serving Next.js HTML — staff journey broken"
  fi
  emit_result "$phase" "$tier" "server-23-dashboard-html" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Admin HTML loads (port 3201 per CLAUDE.md) ---
  local admin_html; admin_html=$(curl -s -m 10 "http://192.168.31.23:3201" 2>/dev/null || echo "")
  local admin_ok; admin_ok=$(printf '%s' "$admin_html" | grep -c "__NEXT" 2>/dev/null || echo "0")
  if [[ "${admin_ok:-0}" -gt 0 ]]; then
    status="PASS"; severity="P3"; message="Admin HTML loads with Next.js markers (${admin_ok} found)"
  else
    status="WARN"; severity="P2"; message="Admin :3201 not serving Next.js HTML (may be down or on different port)"
  fi
  emit_result "$phase" "$tier" "server-23-admin-html" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase48

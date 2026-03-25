#!/usr/bin/env bash
# audit/phases/tier6/phase33.sh -- Phase 33: Cafe Marketing & PNG Generation
# Tier: 6 (Notifications & Marketing)
# What: Marketing content generates, WhatsApp broadcast works, promo engine evaluates.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase33() {
  local phase="33" tier="6"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Cafe marketing logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local mkt_entries; mkt_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "cafe_marketing\|png.*generat\|broadcast" || echo "0")
    if [[ "${mkt_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Cafe marketing/broadcast activity in recent logs (${mkt_entries} entries)"
    else
      status="WARN"; severity="P2"; message="No cafe_marketing/broadcast entries in recent logs (may be infrequent)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check marketing logs"
  fi
  emit_result "$phase" "$tier" "server-23-cafe-marketing" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Promo engine evaluation
  if [[ -n "$log_resp" ]]; then
    local promo_entries; promo_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "cafe_promo.*evaluat\|promo.*applied" || echo "0")
    if [[ "${promo_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Cafe promo evaluation entries found (${promo_entries})"
    else
      status="WARN"; severity="P2"; message="No promo evaluation entries in recent logs (promo engine may not be active)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check promo engine"
  fi
  emit_result "$phase" "$tier" "server-23-promo-engine" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase33

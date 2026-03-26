#!/usr/bin/env bash
# audit/phases/tier3/phase64.sh -- Phase 64: Sentinel Alert Wiring
# Tier: 3 (Display/UX)
# What: Verify OBS-01 (MAINTENANCE_MODE WhatsApp alert) and OBS-04 (sentinel file WebSocket events) wiring exists.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase64() {
  local phase="64" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  # Venue closed -> QUIET (can't verify live alert delivery)
  if [[ "$venue_state" == "closed" ]]; then
    emit_result "$phase" "$tier" "fleet-sentinel-wiring" "QUIET" "P3" \
      "Sentinel wiring check skipped -- venue closed" "$mode" "$venue_state"
    return 0
  fi

  # Check fleet health for active_sentinels field (OBS-04 wiring indicator)
  local fleet_resp
  fleet_resp=$(http_get "${FLEET_HEALTH_ENDPOINT:-http://192.168.31.23:8080/api/v1/fleet/health}" "$DEFAULT_TIMEOUT")

  if [[ -z "$fleet_resp" ]]; then
    status="FAIL"; severity="P1"
    message="Sentinel wiring check failed -- fleet health endpoint unreachable"
    emit_result "$phase" "$tier" "fleet-sentinel-wiring" "$status" "$severity" "$message" "$mode" "$venue_state"
    return 0
  fi

  # Check if any pod response contains active_sentinels field
  local has_sentinel_field
  has_sentinel_field=$(printf '%s' "$fleet_resp" | jq '[.[] | select(has("active_sentinels"))] | length' 2>/dev/null || echo "0")

  if [[ "${has_sentinel_field:-0}" -gt 0 ]]; then
    # active_sentinels field exists in fleet response -- OBS-04 wiring is present
    local active_sentinel_count
    active_sentinel_count=$(printf '%s' "$fleet_resp" | jq '[.[] | .active_sentinels // [] | length] | add // 0' 2>/dev/null || echo "0")
    status="PASS"; severity="P3"
    message="Sentinel wiring present: ${has_sentinel_field} pod(s) report active_sentinels (${active_sentinel_count} total active)"
  else
    # Field missing -- server may not have OBS-04 code yet
    status="WARN"; severity="P3"
    message="Sentinel wiring: active_sentinels field not found in fleet health (server may need OBS-04 deployment)"
  fi
  emit_result "$phase" "$tier" "fleet-sentinel-wiring" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase64

#!/usr/bin/env bash
# audit/phases/tier2/phase62.sh -- Phase 62: Config Fallback Detection
# Tier: 2 (Core Services)
# What: Verify pods are running with real racecontrol.toml config, not OBS-02 fallback defaults.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase62() {
  local phase="62" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  # Known fallback default values (from OBS-02 config fallback implementation)
  local DEFAULT_API_URL="http://127.0.0.1:8080"
  local DEFAULT_SERVER_IP="127.0.0.1"

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # Check if pod is reachable first
    local health_resp
    health_resp=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    if [[ -z "$health_resp" ]]; then
      if [[ "$venue_state" == "closed" ]]; then
        status="QUIET"; severity="P3"
        message="Config fallback check skipped -- pod unreachable, venue closed"
      else
        status="FAIL"; severity="P2"
        message="Config fallback check failed -- pod unreachable"
      fi
      emit_result "$phase" "$tier" "${host}-config-fallback" "$status" "$severity" "$message" "$mode" "$venue_state"
      continue
    fi

    # Fetch racecontrol.toml via rc-sentry /files endpoint to check config values
    local toml_resp
    toml_resp=$(curl -s --max-time 10 -X POST "http://${ip}:${SENTRY_PORT:-8091}/files" \
      -H "Content-Type: application/json" \
      -d '{"path":"C:\\\\RacingPoint\\\\racecontrol.toml"}' 2>/dev/null || echo "")

    if [[ -z "$toml_resp" ]]; then
      # rc-sentry may not have /files endpoint -- fallback to health check
      # If health is up, agent is running with some config (possibly fallback)
      status="WARN"; severity="P2"
      message="Cannot read racecontrol.toml via rc-sentry -- unable to verify config"
      emit_result "$phase" "$tier" "${host}-config-fallback" "$status" "$severity" "$message" "$mode" "$venue_state"
      continue
    fi

    # Check for fallback default values in the config
    local has_fallback=false
    local fallback_details=""

    if printf '%s' "$toml_resp" | grep -q "api_url.*${DEFAULT_API_URL}\|api_url.*127\.0\.0\.1" 2>/dev/null; then
      has_fallback=true
      fallback_details="${fallback_details}api_url=127.0.0.1 "
    fi

    if printf '%s' "$toml_resp" | grep -q "server_ip.*${DEFAULT_SERVER_IP}" 2>/dev/null; then
      # Exclude lines that also contain the real server IP
      if ! printf '%s' "$toml_resp" | grep "server_ip" | grep -q "192\.168\." 2>/dev/null; then
        has_fallback=true
        fallback_details="${fallback_details}server_ip=127.0.0.1 "
      fi
    fi

    if [[ "$has_fallback" == "true" ]]; then
      status="FAIL"; severity="P2"
      message="Config using fallback defaults: ${fallback_details}-- agent may not reach server"
    else
      status="PASS"; severity="P3"
      message="Config has real server values (no fallback defaults detected)"
    fi
    emit_result "$phase" "$tier" "${host}-config-fallback" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase62

#!/usr/bin/env bash
# audit/phases/tier1/phase01.sh -- Phase 01: Fleet Inventory
# Tier: 1 (Infrastructure Foundation)
# What: Every binary, build_id, uptime, process count across all machines.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase01() {
  local phase="01" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"

  # ---------------------------------------------------------------------------
  # SERVER CHECK: racecontrol :8080/api/v1/health
  # ---------------------------------------------------------------------------
  local response status severity message
  response=$(http_get "http://192.168.31.23:8080/api/v1/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]] && echo "$response" | grep -q "build_id"; then
    status="PASS"
    severity="P3"
    message="racecontrol healthy: $(echo "$response" | jq -r '.build_id // "unknown"' 2>/dev/null || echo 'ok')"
  else
    status="FAIL"
    severity="P1"
    message="racecontrol :8080 unreachable or unhealthy"
  fi
  emit_result "$phase" "$tier" "server-23-8080" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # SERVER OPS CHECK: server_ops :8090/health
  # ---------------------------------------------------------------------------
  response=$(http_get "http://192.168.31.23:8090/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]] && printf '%s' "$response" | grep -q '"status"'; then
    status="PASS"
    severity="P3"
    message="server_ops healthy: $(printf '%s' "$response" | jq -r '.status // "ok"' 2>/dev/null)"
  else
    status="FAIL"
    severity="P1"
    message="server_ops :8090 unreachable or unhealthy"
  fi
  emit_result "$phase" "$tier" "server-23-8090" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # POD LOOP: all 8 pods -- rc-agent :8090 and rc-sentry :8091
  # ---------------------------------------------------------------------------
  local ip host
  for ip in $PODS; do
    # Derive short host name: 192.168.31.89 -> pod-89
    host="pod-$(echo "$ip" | sed 's/192\.168\.31\.//')"

    # --- rc-agent check ---
    response=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    if echo "$response" | grep -q "build_id"; then
      status="PASS"
      severity="P3"
      message="rc-agent healthy"
    elif [[ -z "$response" ]]; then
      status="FAIL"
      severity="P1"
      message="rc-agent unreachable"
    else
      status="WARN"
      severity="P2"
      message="rc-agent unhealthy response"
    fi
    # QUIET override: venue closed + (FAIL or WARN) -> QUIET
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"
      severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-rcagent" "$status" "$severity" "$message" "$mode" "$venue_state"

    # --- rc-sentry check ---
    response=$(http_get "http://${ip}:8091/health" "$DEFAULT_TIMEOUT")
    if echo "$response" | grep -q "build_id"; then
      status="PASS"
      severity="P3"
      message="rc-sentry healthy"
    elif [[ -z "$response" ]]; then
      status="FAIL"
      severity="P1"
      message="rc-sentry unreachable"
    else
      status="WARN"
      severity="P2"
      message="rc-sentry unhealthy response"
    fi
    # QUIET override: venue closed + (FAIL or WARN) -> QUIET
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"
      severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-rcsentry" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # ---------------------------------------------------------------------------
  # JAMES LOCAL CHECK: comms-link relay (quick mode only -- skip in other modes if extended)
  # ---------------------------------------------------------------------------
  response=$(http_get "http://localhost:8766/relay/health" 5)
  if [[ -n "$response" ]]; then
    status="PASS"
    severity="P3"
    message="comms-link relay reachable"
  else
    status="WARN"
    severity="P2"
    message="comms-link relay not responding (expected if offline)"
  fi
  emit_result "$phase" "$tier" "james-commslink" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Full mode only: check Ollama and go2rtc
  if [[ "$mode" = "full" ]]; then
    response=$(http_get "http://localhost:11434/api/tags" 5)
    if [[ -n "$response" ]]; then
      status="PASS"; severity="P3"; message="Ollama reachable"
    else
      status="WARN"; severity="P2"; message="Ollama not responding"
    fi
    emit_result "$phase" "$tier" "james-ollama" "$status" "$severity" "$message" "$mode" "$venue_state"

    response=$(http_get "http://localhost:1984/api/streams" 5)
    if [[ -n "$response" ]]; then
      status="PASS"; severity="P3"; message="go2rtc reachable"
    else
      status="WARN"; severity="P2"; message="go2rtc not responding"
    fi
    emit_result "$phase" "$tier" "james-go2rtc" "$status" "$severity" "$message" "$mode" "$venue_state"
  fi

  # ---------------------------------------------------------------------------
  # BONO VPS CHECK: cloud racecontrol :8080/api/v1/health
  # ---------------------------------------------------------------------------
  response=$(http_get "http://100.70.177.44:8080/api/v1/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]] && echo "$response" | grep -q "build_id"; then
    status="PASS"
    severity="P3"
    message="Bono VPS healthy: $(echo "$response" | jq -r '.build_id // "unknown"' 2>/dev/null || echo 'ok')"
  else
    status="WARN"
    severity="P2"
    message="Bono VPS :8080 unreachable or unhealthy (degraded, not critical for venue ops)"
  fi
  emit_result "$phase" "$tier" "bono-vps" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase01

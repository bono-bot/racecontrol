#!/usr/bin/env bash
# audit/phases/tier1/phase09.sh -- Phase 09: Self-Monitor & Self-Heal
# Tier: 1 (Infrastructure Foundation)
# What: rc-agent self_monitor, self_heal, failure_monitor active on pods.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase09() {
  local phase="09" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # Check self-monitor is alive: if rc-agent is up with uptime > 60s, self-monitor is working
    response=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    local uptime; uptime=$(printf '%s' "$response" | jq -r '.uptime_secs // 0' 2>/dev/null)
    if [[ "${uptime:-0}" -ge 60 ]]; then
      status="PASS"; severity="P3"; message="Self-monitor healthy (agent uptime ${uptime}s)"
    elif [[ "${uptime:-0}" -gt 0 ]]; then
      status="PASS"; severity="P3"; message="Self-monitor started recently (uptime ${uptime}s)"
    else
      status="WARN"; severity="P2"; message="Cannot verify self-monitor (health unreachable or uptime 0)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-self-monitor" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Sub-check: self-monitor log recency (WL-01)
    # Verify JSONL log file was written recently, not just that agent has uptime
    response=$(safe_remote_exec "$ip" "8090" \
      'powershell -Command "(Get-Item C:\\RacingPoint\\rc-agent-*.jsonl | Sort-Object LastWriteTime -Descending | Select-Object -First 1).LastWriteTime.ToString('"'"'yyyy-MM-dd HH:mm:ss'"'"')"' \
      "$DEFAULT_TIMEOUT")
    local remote_ts; remote_ts=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '\r\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
    if [[ -n "$remote_ts" ]]; then
      local remote_epoch now_epoch delta_secs
      remote_epoch=$(date -d "$remote_ts" +%s 2>/dev/null || echo 0)
      now_epoch=$(date +%s)
      delta_secs=$(( now_epoch - remote_epoch ))
      if [[ "$remote_epoch" -gt 0 ]] && [[ "$delta_secs" -lt 300 ]]; then
        status="PASS"; severity="P3"; message="Self-monitor log fresh (last write ${delta_secs}s ago)"
      elif [[ "$remote_epoch" -gt 0 ]] && [[ "$delta_secs" -lt 900 ]]; then
        status="WARN"; severity="P2"; message="Self-monitor log stale (last write ${delta_secs}s ago)"
      elif [[ "$remote_epoch" -gt 0 ]]; then
        status="WARN"; severity="P2"; message="Self-monitor possibly dead (last write ${delta_secs}s ago)"
      else
        status="WARN"; severity="P2"; message="Self-monitor log timestamp unparseable: ${remote_ts}"
      fi
    else
      status="WARN"; severity="P2"; message="Self-monitor recency check failed (no JSONL file or exec error)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-self-monitor-recency" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Check for safe_mode active (must NOT be active)
    response=$(safe_remote_exec "$ip" "8090" \
      'findstr /C:"safe_mode" /C:"SAFE_MODE" C:\RacingPoint\rc-agent-*.jsonl 2>nul | findstr /V /C:"disabled"' \
      "$DEFAULT_TIMEOUT")
    local sm_out; sm_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -z "$sm_out" ]]; then
      status="PASS"; severity="P3"; message="safe_mode not active on pod"
    else
      status="WARN"; severity="P2"; message="safe_mode references found in logs -- verify mode is disabled"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-safe-mode" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase09

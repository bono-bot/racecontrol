#!/usr/bin/env bash
# audit/phases/tier9/phase43.sh -- Phase 43: Camera Pipeline
# Tier: 9 (Cameras & AI)
# What: go2rtc streams all 13 cameras. NVR reachable. Streams serving.
# NOTE: go2rtc runs on James :1984 (NOT server :8096 -- standing rule: verify against running system)

set -u
set -o pipefail

run_phase43() {
  local phase="43" tier="9"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # go2rtc stream count on James :1984
  response=$(http_get "http://localhost:1984/api/streams" 5)
  if [[ -n "$response" ]]; then
    local stream_count; stream_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null || echo "0")
    if [[ "${stream_count:-0}" -ge 13 ]]; then
      status="PASS"; severity="P3"; message="go2rtc: ${stream_count} streams active (>= 13 cameras)"
    elif [[ "${stream_count:-0}" -ge 1 ]]; then
      status="WARN"; severity="P2"; message="go2rtc: only ${stream_count}/13 streams active"
    else
      status="FAIL"; severity="P2"; message="go2rtc: 0 streams active — no cameras serving"
    fi
  else
    status="WARN"; severity="P2"; message="go2rtc not responding at localhost:1984 (expected on James .27)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-go2rtc-streams" "$status" "$severity" "$message" "$mode" "$venue_state"

  # NVR reachable (192.168.31.18)
  local nvr_code; nvr_code=$(curl -s -m 5 -o /dev/null -w "%{http_code}" "http://192.168.31.18" 2>/dev/null || echo "000")
  if [[ "$nvr_code" != "000" && -n "$nvr_code" ]]; then
    status="PASS"; severity="P3"; message="NVR .18 reachable (HTTP ${nvr_code})"
  else
    status="WARN"; severity="P2"; message="NVR .18 not responding (may be offline or blocked by firewall)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "nvr-18-reachable" "$status" "$severity" "$message" "$mode" "$venue_state"

  # go2rtc process running on James
  local go2rtc_proc; go2rtc_proc=$(tasklist 2>/dev/null | grep -i "go2rtc" || true)
  if [[ -n "$go2rtc_proc" ]]; then
    status="PASS"; severity="P3"; message="go2rtc.exe process running"
  else
    status="WARN"; severity="P2"; message="go2rtc process not found in tasklist (may need to start go2rtc.exe)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-go2rtc-process" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase43

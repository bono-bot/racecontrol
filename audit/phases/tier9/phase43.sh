#!/usr/bin/env bash
# audit/phases/tier9/phase43.sh -- Phase 43: Camera Pipeline
# Tier: 9 (Cameras & AI)
# What: go2rtc running, cameras dashboard serving, NVR reachable, per-camera health.
# go2rtc API on James :1984, cameras dashboard on James :8096

set -u
set -o pipefail
# NO set -e

run_phase43() {
  local phase="43" tier="9"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: go2rtc process running on James ---
  local go2rtc_proc; go2rtc_proc=$(tasklist 2>/dev/null | grep -i "go2rtc" || true)
  if [[ -n "$go2rtc_proc" ]]; then
    status="PASS"; severity="P3"; message="go2rtc.exe process running"
  else
    status="FAIL"; severity="P1"; message="go2rtc process not found — camera system DOWN"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" != "PASS" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-go2rtc-process" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Cameras dashboard page at :8096 ---
  local dash_code; dash_code=$(curl -s -m 5 -o /dev/null -w "%{http_code}" "http://localhost:8096/cameras/live" 2>/dev/null)
  if [[ "$dash_code" = "200" ]]; then
    status="PASS"; severity="P3"; message="Cameras dashboard :8096/cameras/live serving (HTTP 200)"
  elif [[ "$dash_code" = "000" ]]; then
    status="FAIL"; severity="P1"; message="Cameras dashboard :8096 unreachable (go2rtc web UI down)"
  else
    status="WARN"; severity="P2"; message="Cameras dashboard :8096 returned HTTP ${dash_code}"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" != "PASS" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-cameras-dashboard" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: go2rtc API stream count ---
  response=$(http_get "http://localhost:1984/api/streams" 5)
  local stream_count=0
  if [[ -n "$response" ]]; then
    stream_count=$(printf '%s' "$response" | jq 'keys | length' 2>/dev/null)
    if [[ "${stream_count:-0}" -ge 13 ]]; then
      status="PASS"; severity="P3"; message="go2rtc: ${stream_count} streams configured (>= 13 cameras)"
    elif [[ "${stream_count:-0}" -ge 1 ]]; then
      status="WARN"; severity="P2"; message="go2rtc: only ${stream_count}/13 streams configured"
    else
      status="FAIL"; severity="P2"; message="go2rtc: 0 streams configured — no cameras set up"
    fi
  else
    status="WARN"; severity="P2"; message="go2rtc API not responding at localhost:1984"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-go2rtc-streams" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: NVR reachable (192.168.31.18) ---
  local nvr_code; nvr_code=$(curl -s -m 5 -o /dev/null -w "%{http_code}" "http://192.168.31.18" 2>/dev/null)
  if [[ "$nvr_code" != "000" && -n "$nvr_code" ]]; then
    status="PASS"; severity="P3"; message="NVR .18 reachable (HTTP ${nvr_code})"
  else
    status="WARN"; severity="P2"; message="NVR .18 not responding (may be offline or blocked by firewall)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "nvr-18-reachable" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 5: Per-camera RTSP health (sample probe) ---
  # go2rtc is on-demand: streams without viewers have no active RTSP connection.
  # Probing ALL cameras at once overwhelms the NVR's RTSP connection limit (~8-16 max).
  # Strategy: count already-active streams, then probe 3 sample cameras (NVR ch1, ch9, entrance)
  # with gaps to avoid NVR connection exhaustion.
  if [[ -n "$response" && "${stream_count:-0}" -ge 1 ]]; then
    # Count base cameras (exclude _h264 transcoded variants)
    local cam_total; cam_total=$(printf '%s' "$response" | jq '[keys[] | select(endswith("_h264") | not)] | length' 2>/dev/null)
    # Count streams with active RTSP producers (bytes flowing)
    local cam_active; cam_active=$(printf '%s' "$response" | jq '[to_entries[] | select(.key | endswith("_h264") | not) | select(.value.producers // [] | map(select(.bytes_recv != null and .bytes_recv > 0)) | length > 0)] | length' 2>/dev/null)

    # Probe 3 sample cameras: NVR ch1, NVR ch9, standalone entrance
    # These represent different RTSP sources (NVR channels + standalone cam)
    local sample_cams="ch1 ch9 entrance"
    local probed=0 probe_ok=0 probe_fail_list=""
    for cam in $sample_cams; do
      # Skip if already active
      local has_active; has_active=$(printf '%s' "$response" | jq -r \
        --arg c "$cam" '.[$c].producers // [] | map(select(.bytes_recv != null and .bytes_recv > 0)) | length' 2>/dev/null)
      if [[ "${has_active:-0}" -ge 1 ]]; then
        probe_ok=$((probe_ok + 1))
        probed=$((probed + 1))
        continue
      fi

      # Probe via frame.jpeg — forces RTSP connect + grab one frame
      # 10s timeout: RTSP handshake + first keyframe can take 3-8s
      local probe_code; probe_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
        "http://localhost:1984/api/frame.jpeg?src=${cam}" 2>/dev/null)
      probed=$((probed + 1))
      if [[ "$probe_code" = "200" ]]; then
        probe_ok=$((probe_ok + 1))
      else
        if [[ -n "$probe_fail_list" ]]; then
          probe_fail_list="${probe_fail_list}, ${cam}"
        else
          probe_fail_list="${cam}"
        fi
      fi
      # 2s gap between probes to avoid NVR connection flooding
      sleep 2
    done

    if [[ "$probe_ok" -eq "$probed" ]]; then
      status="PASS"; severity="P3"; message="${cam_total} streams configured, ${cam_active} active, ${probed}/${probed} sample probes OK"
    elif [[ "$probe_ok" -ge 1 ]]; then
      status="WARN"; severity="P2"; message="Camera probe partial: ${probe_ok}/${probed} OK. Failed: ${probe_fail_list}"
    else
      status="FAIL"; severity="P1"; message="All ${probed} camera probes failed: ${probe_fail_list} — NVR or RTSP may be down"
    fi
  else
    status="WARN"; severity="P2"; message="Cannot probe cameras �� go2rtc API unavailable"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" != "PASS" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-cameras-health" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 6: Standalone camera IPs reachable (entrance .8, reception .15, .154) ---
  local standalone_cams="192.168.31.8 192.168.31.15 192.168.31.154"
  for cam_ip in $standalone_cams; do
    local cam_code; cam_code=$(curl -s -m 3 -o /dev/null -w "%{http_code}" "http://${cam_ip}" 2>/dev/null)
    local cam_name; cam_name=$(printf '%s' "$cam_ip" | sed 's/192\.168\.31\./cam-/')
    if [[ "$cam_code" != "000" && -n "$cam_code" ]]; then
      status="PASS"; severity="P3"; message="Camera ${cam_ip} reachable (HTTP ${cam_code})"
    else
      status="WARN"; severity="P2"; message="Camera ${cam_ip} not responding"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${cam_name}-reachable" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase43

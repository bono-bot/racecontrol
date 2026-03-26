#!/usr/bin/env bash
# audit/lib/fixes.sh -- Auto-fix engine for Racing Point fleet audit
#
# Off by default: AUTO_FIX=true required (FIX-01)
# Fail-safe: is_pod_idle() returns 1 on any API error (FIX-02)
# Whitelist-only: APPROVED_FIXES whitelist enforced (FIX-08)
# Audit trail: emit_fix() logs every action to fixes.jsonl (FIX-07)

APPROVED_FIXES=("clear_stale_sentinels" "kill_orphan_powershell" "restart_rc_agent" "wol_pod" "clear_old_maintenance_mode" "replace_stale_bat")
export APPROVED_FIXES

_is_approved_fix() {
  local fix_name="$1"
  local entry
  for entry in "${APPROVED_FIXES[@]}"; do
    if [[ "$entry" == "$fix_name" ]]; then return 0; fi
  done
  return 1
}
export -f _is_approved_fix

_ip_to_pod_number() {
  local ip="$1"
  case "${ip##*.}" in
    89) echo "1" ;; 33) echo "2" ;; 28) echo "3" ;; 88) echo "4" ;;
    86) echo "5" ;; 87) echo "6" ;; 38) echo "7" ;; 91) echo "8" ;;
    *)  echo "" ;;
  esac
}
export -f _ip_to_pod_number

_FLEET_HEALTH_CACHE=""

is_pod_idle() {
  local pod_ip="$1"
  local pod_num
  pod_num=$(_ip_to_pod_number "$pod_ip")
  if [[ -z "$pod_num" ]]; then return 1; fi
  if [[ -z "$_FLEET_HEALTH_CACHE" ]]; then
    _FLEET_HEALTH_CACHE=$(curl -s -m 8 "${FLEET_HEALTH_ENDPOINT:-http://192.168.31.23:8080/api/v1/fleet/health}" 2>/dev/null || echo "")
  fi
  if [[ -z "$_FLEET_HEALTH_CACHE" ]]; then return 1; fi
  if ! printf "%s" "$_FLEET_HEALTH_CACHE" | jq -e "." >/dev/null 2>&1; then return 1; fi
  local active_billing
  active_billing=$(printf "%s" "$_FLEET_HEALTH_CACHE" | jq -r --argjson pn "$pod_num" '[.[] | select(.pod_number == $pn)] | first | ((.active_billing_session // false) or (.billing_active // false))' 2>/dev/null || echo "error")
  if [[ "$active_billing" == "error" || "$active_billing" == "null" || -z "$active_billing" ]]; then return 1; fi
  if [[ "$active_billing" == "true" ]]; then return 1; fi
  return 0
}
export -f is_pod_idle

check_pod_sentinels() {
  local pod_ip="$1"
  local result
  result=$(safe_remote_exec "$pod_ip" 8090 \
    "if exist C:\RacingPoint\OTA_DEPLOYING echo OTA_ACTIVE & if exist C:\RacingPoint\MAINTENANCE_MODE echo MM_ACTIVE" 10)
  if printf "%s" "$result" | grep -qE "OTA_ACTIVE|MM_ACTIVE"; then return 1; fi
  return 0
}
export -f check_pod_sentinels

clear_stale_sentinels() {
  local pod_ip="$1"
  if ! _is_approved_fix "clear_stale_sentinels"; then return 1; fi
  local check_result
  check_result=$(safe_remote_exec "$pod_ip" 8090 "if exist C:\RacingPoint\MAINTENANCE_MODE echo MM & if exist C:\RacingPoint\GRACEFUL_RELAUNCH echo GR & if exist C:\RacingPoint\rcagent-restart-sentinel.txt echo RS" 10)
  local before_state="" found_any=false
  if printf "%s" "$check_result" | grep -q "MM"; then before_state="${before_state}MAINTENANCE_MODE,"; found_any=true; fi
  if printf "%s" "$check_result" | grep -q "GR"; then before_state="${before_state}GRACEFUL_RELAUNCH,"; found_any=true; fi
  if printf "%s" "$check_result" | grep -q "RS"; then before_state="${before_state}rcagent-restart-sentinel.txt,"; found_any=true; fi
  if [[ "$found_any" == "false" ]]; then return 0; fi
  before_state="${before_state%,}"
  safe_remote_exec "$pod_ip" 8090 "del /Q C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\GRACEFUL_RELAUNCH C:\RacingPoint\rcagent-restart-sentinel.txt 2>nul" 10 >/dev/null
  local verify_result
  verify_result=$(safe_remote_exec "$pod_ip" 8090 "if exist C:\RacingPoint\MAINTENANCE_MODE echo MM & if exist C:\RacingPoint\GRACEFUL_RELAUNCH echo GR & if exist C:\RacingPoint\rcagent-restart-sentinel.txt echo RS" 10)
  local after_state="cleared"
  if printf "%s" "$verify_result" | grep -qE "MM|GR|RS"; then after_state="partial_clear_check_logs"; fi
  emit_fix "sentinel" "$pod_ip" "clear_stale_sentinels" "$before_state" "$after_state"
  return 0
}
export -f clear_stale_sentinels

kill_orphan_powershell() {
  local pod_ip="$1"
  if ! _is_approved_fix "kill_orphan_powershell"; then return 1; fi
  local count_result
  count_result=$(safe_remote_exec "$pod_ip" 8090 "tasklist /FI \"IMAGENAME eq powershell.exe\" /NH 2>/dev/null | findstr /I powershell | find /C /V \"\"" 10)
  local count
  count=$(printf "%s" "$count_result" | jq -r ".stdout // .output // .result // \"\"" 2>/dev/null | tr -d "[:space:]" | grep -oE "^[0-9]+")
  count="${count:-0}"
  if [[ "$count" -le 1 ]]; then return 0; fi
  local before_state="powershell_count=${count}"
  safe_remote_exec "$pod_ip" 8090 "taskkill /F /IM powershell.exe 2>nul" 10 >/dev/null
  emit_fix "orphan_ps" "$pod_ip" "kill_orphan_powershell" "$before_state" "killed_all,watchdog_will_restart"
  return 0
}
export -f kill_orphan_powershell

restart_rc_agent() {
  local pod_ip="$1"
  if ! _is_approved_fix "restart_rc_agent"; then return 1; fi
  local agent_health
  agent_health=$(http_get "http://${pod_ip}:8090/health" 5)
  if [[ -n "$agent_health" ]]; then return 0; fi
  local sentry_health
  sentry_health=$(http_get "http://${pod_ip}:8091/health" 5)
  if [[ -z "$sentry_health" ]]; then return 0; fi
  safe_remote_exec "$pod_ip" 8091 "schtasks /Run /TN StartRCAgent" 10 >/dev/null
  emit_fix "rc_agent" "$pod_ip" "restart_rc_agent" "rc-agent_down,rc-sentry_up" "schtasks_triggered"
  return 0
}
export -f restart_rc_agent

# ---------------------------------------------------------------------------
# Helper — _pod_mac_address (pod_ip)
# Maps last IP octet to MAC address for Wake-on-LAN magic packets.
# ---------------------------------------------------------------------------
_pod_mac_address() {
  local ip="$1"
  case "${ip##*.}" in
    89) echo "30-56-0F-05-45-88" ;;
    33) echo "30-56-0F-05-46-53" ;;
    28) echo "30-56-0F-05-44-B3" ;;
    88) echo "30-56-0F-05-45-25" ;;
    86) echo "30-56-0F-05-44-B7" ;;
    87) echo "30-56-0F-05-45-6E" ;;
    38) echo "30-56-0F-05-44-B4" ;;
    91) echo "30-56-0F-05-46-C5" ;;
    *)  echo "" ;;
  esac
}
export -f _pod_mac_address

# ---------------------------------------------------------------------------
# Fix — wol_pod (pod_ip)
# Hypothesis: Pod is powered off (ping fails) -- WoL magic packet should wake it.
# Guard: WOL_ENABLED defaults to false until manual test on at least 2 pods.
# Gate: Does NOT send WoL if pod is already online (avoids spurious WoL).
# ---------------------------------------------------------------------------
wol_pod() {
  local pod_ip="$1"
  if ! _is_approved_fix "wol_pod"; then return 1; fi

  # Guard: WOL_ENABLED must be explicitly "true" — default false until manual test
  if [[ "${WOL_ENABLED:-false}" != "true" ]]; then
    emit_fix "wol" "$pod_ip" "wol_pod" "wol_disabled" "skipped_wol_not_enabled"
    return 0
  fi

  local MAC
  MAC=$(_pod_mac_address "$pod_ip")
  if [[ -z "$MAC" ]]; then
    emit_fix "wol" "$pod_ip" "wol_pod" "pod_offline" "unknown_mac_no_wol_sent"
    return 1
  fi

  # Test hypothesis: if ping succeeds, pod is ON — skip WoL (Pitfall 3)
  local ping_result
  ping_result=$(safe_remote_exec "192.168.31.23" 8090 "ping -n 1 -w 2000 ${pod_ip}" 10)
  local ping_output
  ping_output=$(printf '%s' "$ping_result" | jq -r '.stdout // .output // .result // ""' 2>/dev/null | tr '[:upper:]' '[:lower:]')
  if printf '%s' "$ping_output" | grep -q "reply from"; then
    # Pod already online — hypothesis incorrect, no WoL needed
    emit_fix "wol" "$pod_ip" "wol_pod" "pod_online_no_wol_needed" "skipped_already_online"
    return 0
  fi

  # Send magic packet via server .23 using PowerShell UDP broadcast
  # Convert MAC from format 30-56-0F-05-45-88 to bytes for magic packet
  local wol_cmd
  wol_cmd="\$mac='${MAC}';\$bytes=[byte[]](,0xFF*6)+([byte[]](('0x'+\$mac.Replace('-',','0x').Replace('-',',0x').Split(',')) | ForEach-Object {[Convert]::ToByte(\$_,16)})*(16));\$udp=New-Object System.Net.Sockets.UdpClient;\$udp.Connect('255.255.255.255',9);\$udp.Send(\$bytes,\$bytes.Length);\$udp.Close();Write-Host 'WoL sent'"
  # Use a simpler, more reliable PowerShell WoL one-liner via safe_remote_exec
  local mac_hex
  mac_hex=$(printf '%s' "$MAC" | tr '-' ':')
  local ps_wol
  ps_wol="powershell -Command \"\$m='${MAC}'.Replace('-','');[byte[]]\$b=(,0xFF*6)+(,([Convert]::ToByte(\$m.Substring(0,2),16),[Convert]::ToByte(\$m.Substring(2,2),16),[Convert]::ToByte(\$m.Substring(4,2),16),[Convert]::ToByte(\$m.Substring(6,2),16),[Convert]::ToByte(\$m.Substring(8,2),16),[Convert]::ToByte(\$m.Substring(10,2),16))*16);\$u=New-Object Net.Sockets.UdpClient;\$u.Connect('255.255.255.255',9);\$u.Send(\$b,102);\$u.Close();Write-Host 'WoL_SENT'\""
  safe_remote_exec "192.168.31.23" 8090 "$ps_wol" 15 >/dev/null

  emit_fix "wol" "$pod_ip" "wol_pod" "pod_offline" "wol_sent,mac=${MAC}"
  return 0
}
export -f wol_pod

# ---------------------------------------------------------------------------
# Fix — clear_old_maintenance_mode (pod_ip)
# Hypothesis: MAINTENANCE_MODE sentinel is stale (>30 min old) and blocking all restarts.
# Guard: Only clears when venue is CLOSED -- during open hours MM may be intentional.
# ---------------------------------------------------------------------------
clear_old_maintenance_mode() {
  local pod_ip="$1"
  if ! _is_approved_fix "clear_old_maintenance_mode"; then return 1; fi

  # Guard: Only clear during closed hours — open-hours MAINTENANCE_MODE may be intentional staff action
  local venue_state
  venue_state=$(venue_state_detect 2>/dev/null || echo "open")
  if [[ "$venue_state" == "open" ]]; then
    emit_fix "maintenance_mode" "$pod_ip" "clear_old_maintenance_mode" "venue_open" "skipped_venue_open"
    return 0
  fi

  # Test hypothesis: check if MAINTENANCE_MODE file exists and its age via forfiles (Pitfall 5)
  local forfiles_result
  forfiles_result=$(safe_remote_exec "$pod_ip" 8090 \
    "forfiles /P C:\\RacingPoint /M MAINTENANCE_MODE /C \"cmd /c echo @fdate @ftime\"" 15)
  local forfiles_output
  forfiles_output=$(printf '%s' "$forfiles_result" | jq -r '.stdout // .output // .result // ""' 2>/dev/null | tr -d '\r')

  # If forfiles returns an error (file not found), nothing to clear
  local forfiles_exit
  forfiles_exit=$(printf '%s' "$forfiles_result" | jq -r '.exitCode // .exit_code // ""' 2>/dev/null | tr -d '[:space:]')
  if [[ "$forfiles_exit" != "0" ]] || [[ -z "$forfiles_output" ]]; then
    # File doesn't exist — return 0 (nothing to clear)
    return 0
  fi

  # Parse date output from forfiles: format is typically "MM/DD/YYYY HH:MM:SS AM/PM"
  # Check if the file is older than 30 minutes by comparing timestamps
  local file_datetime
  file_datetime=$(printf '%s' "$forfiles_output" | grep -oE '[0-9]+/[0-9]+/[0-9]+ [0-9]+:[0-9]+:[0-9]+ (AM|PM)' | head -1)

  if [[ -n "$file_datetime" ]]; then
    # Convert to epoch using date parsing (Windows date format M/D/YYYY H:MM:SS AM/PM)
    local file_epoch now_epoch elapsed_mins
    file_epoch=$(date -d "$file_datetime" +%s 2>/dev/null || echo "0")
    now_epoch=$(date +%s)
    elapsed_mins=$(( (now_epoch - file_epoch) / 60 ))

    if [[ "$file_epoch" -gt 0 ]] && [[ "$elapsed_mins" -lt 30 ]]; then
      # File is recent — may be intentional, skip
      emit_fix "maintenance_mode" "$pod_ip" "clear_old_maintenance_mode" "mm_age_${elapsed_mins}min" "skipped_too_recent"
      return 0
    fi
    local before_state="mm_age_${elapsed_mins}min"
  else
    # Cannot determine age — still proceed with clear since venue is closed
    local before_state="mm_age_unknown"
  fi

  # Clear the sentinel
  safe_remote_exec "$pod_ip" 8090 "del /Q C:\\RacingPoint\\MAINTENANCE_MODE 2>nul" 10 >/dev/null

  # Verify: check if file is gone
  local verify_result
  verify_result=$(safe_remote_exec "$pod_ip" 8090 \
    "if exist C:\\RacingPoint\\MAINTENANCE_MODE echo MM_STILL_PRESENT" 10)
  local verify_output
  verify_output=$(printf '%s' "$verify_result" | jq -r '.stdout // .output // .result // ""' 2>/dev/null)
  local after_state="cleared"
  if printf '%s' "$verify_output" | grep -q "MM_STILL_PRESENT"; then
    after_state="still_present"
  fi

  emit_fix "maintenance_mode" "$pod_ip" "clear_old_maintenance_mode" "${before_state:-mm_stale}" "$after_state"
  return 0
}
export -f clear_old_maintenance_mode

# ---------------------------------------------------------------------------
# Fix — replace_stale_bat (pod_ip)
# Hypothesis: Pod start-rcagent.bat diverges from canonical version -- stale bat causes settings regression.
# Guard: Checks staging HTTP server at port 9998 before attempting download.
# ---------------------------------------------------------------------------
replace_stale_bat() {
  local pod_ip="$1"
  if ! _is_approved_fix "replace_stale_bat"; then return 1; fi

  # Guard: Check staging HTTP server is online (Pitfall 6)
  local staging_check
  staging_check=$(curl -s -o /dev/null -w '%{http_code}' \
    "http://192.168.31.27:9998/start-rcagent.bat" 2>/dev/null | tr -d '\r')
  if [[ "$staging_check" != "200" ]]; then
    emit_fix "bat" "$pod_ip" "replace_stale_bat" "bat_stale" "bat_staging_server_offline"
    return 0
  fi

  # Apply fix: download canonical bat from staging server to pod
  safe_remote_exec "$pod_ip" 8090 \
    "curl.exe -s -o C:\\RacingPoint\\start-rcagent.bat http://192.168.31.27:9998/start-rcagent.bat" 20 >/dev/null

  # Verify: check file exists and has non-zero size
  local verify_result
  verify_result=$(safe_remote_exec "$pod_ip" 8090 \
    "if exist C:\\RacingPoint\\start-rcagent.bat (for %I in (C:\\RacingPoint\\start-rcagent.bat) do echo SIZE=%~zI) else echo FILE_MISSING" 10)
  local verify_output
  verify_output=$(printf '%s' "$verify_result" | jq -r '.stdout // .output // .result // ""' 2>/dev/null | tr -d '\r')

  local after_state="bat_download_failed"
  if printf '%s' "$verify_output" | grep -qE "SIZE=[1-9]"; then
    after_state="bat_replaced"
  elif printf '%s' "$verify_output" | grep -q "FILE_MISSING"; then
    after_state="bat_download_failed"
  fi

  emit_fix "bat" "$pod_ip" "replace_stale_bat" "bat_stale" "$after_state"
  return 0
}
export -f replace_stale_bat

run_auto_fixes() {
  if [[ "${AUTO_FIX:-false}" != "true" ]]; then return 0; fi
  echo "--- Auto-Fix Engine ---"
  _FLEET_HEALTH_CACHE=""
  local fixes_applied=0 pods_skipped=0 pod_ip
  for pod_ip in ${PODS:-}; do
    if ! is_pod_idle "$pod_ip"; then
      emit_fix "autofix" "$pod_ip" "SKIP_ACTIVE_SESSION" "billing_active" "skipped"
      pods_skipped=$((pods_skipped + 1))
      continue
    fi
    if ! check_pod_sentinels "$pod_ip"; then
      emit_fix "autofix" "$pod_ip" "SKIP_OTA_DEPLOYING" "ota_deploying" "skipped"
      continue
    fi
    local fix_name
    for fix_name in "${APPROVED_FIXES[@]}"; do
      if "$fix_name" "$pod_ip"; then fixes_applied=$((fixes_applied + 1)); fi
    done
  done
  echo "Auto-Fix summary: fixes_applied=${fixes_applied}, pods_skipped=${pods_skipped}"
  echo "--- Auto-Fix Complete ---"
}
export -f run_auto_fixes

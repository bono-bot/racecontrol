#!/usr/bin/env bash
# audit/lib/fixes.sh -- Auto-fix engine for Racing Point fleet audit
#
# Off by default: AUTO_FIX=true required (FIX-01)
# Fail-safe: is_pod_idle() returns 1 on any API error (FIX-02)
# Whitelist-only: APPROVED_FIXES whitelist enforced (FIX-08)
# Audit trail: emit_fix() logs every action to fixes.jsonl (FIX-07)

APPROVED_FIXES=("clear_stale_sentinels" "kill_orphan_powershell" "restart_rc_agent")
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

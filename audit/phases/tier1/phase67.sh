#!/usr/bin/env bash
# audit/phases/tier1/phase67.sh -- Phase 67: Meta-Monitor Liveness
# Tier: 1 (Infrastructure Foundation)
# What: Verifies that self-healing and self-debugging systems are ACTUALLY RUNNING,
#        not just that their code exists. Checks process liveness, scheduled task
#        registration, and output recency.
#
# WHY THIS EXISTS:
#   Previous audits (phases 10, 66) verified:
#     - watchdog-state.json content (phase 10) — proxy, not process
#     - detector/engine script existence (phase 66) — code, not runtime
#   This missed rc-watchdog being dead for 30+ minutes with no alert,
#   and both scheduled tasks (CommsLink-DaemonWatchdog, AutoDetect-Daily)
#   being unregistered — meaning no automated healing or detection was running.
#   The audit checked the MAP, not the TERRITORY.
#
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status.

set -u
set -o pipefail
# NO set -e

run_phase67() {
  local phase="67" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  # ---------------------------------------------------------------------------
  # CHECK 1: rc-watchdog.exe PROCESS is running on James (.27)
  # Phase 10 checks watchdog-state.json (output). This checks the process itself.
  # A dead process with a stale state file = false PASS on phase 10.
  # ---------------------------------------------------------------------------
  local watchdog_running
  watchdog_running=$(tasklist /FI "IMAGENAME eq rc-watchdog.exe" /FO CSV 2>/dev/null | grep -c "rc-watchdog" || echo "0")
  if [[ "${watchdog_running:-0}" -ge 1 ]]; then
    status="PASS"; severity="P3"
    message="rc-watchdog.exe process running (${watchdog_running} instance(s))"
  else
    status="FAIL"; severity="P1"
    message="rc-watchdog.exe NOT running — all self-healing is disabled. Start: C:/Users/bono/racingpoint/deploy-staging/rc-watchdog.exe"
  fi
  emit_result "$phase" "$tier" "watchdog-process" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 2: rc-watchdog log recency (last write within 5 min = 2.5 cycles)
  # The process could be alive but hung. Verify it's producing output.
  # ---------------------------------------------------------------------------
  local log_dir="C:/Users/bono/.claude"
  local today_log="${log_dir}/rc-watchdog.log.$(date -u '+%Y-%m-%d')"
  if [[ -f "$today_log" ]]; then
    local last_check
    last_check=$(grep "check run complete" "$today_log" 2>/dev/null | tail -1 | awk '{print $1}')
    if [[ -n "$last_check" ]]; then
      local last_epoch now_epoch delta_secs
      last_epoch=$(date -d "$last_check" +%s 2>/dev/null || echo "0")
      now_epoch=$(date -u +%s)
      delta_secs=$(( now_epoch - last_epoch ))
      if [[ "$last_epoch" -gt 0 ]] && [[ "$delta_secs" -lt 300 ]]; then
        status="PASS"; severity="P3"
        message="rc-watchdog last cycle ${delta_secs}s ago (fresh)"
      elif [[ "$last_epoch" -gt 0 ]] && [[ "$delta_secs" -lt 600 ]]; then
        status="WARN"; severity="P2"
        message="rc-watchdog last cycle ${delta_secs}s ago (slightly stale, expect every 120s)"
      else
        status="FAIL"; severity="P1"
        message="rc-watchdog last cycle ${delta_secs}s ago — process may be hung or dead"
      fi
    else
      status="WARN"; severity="P2"
      message="rc-watchdog log exists but no 'check run complete' entries found today"
    fi
  else
    status="FAIL"; severity="P1"
    message="rc-watchdog log not found for today — daemon may not have started"
  fi
  emit_result "$phase" "$tier" "watchdog-recency" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 3: CommsLink-DaemonWatchdog scheduled task registered
  # The watchdog process can die (crash, OOM, reboot). Without a scheduled task
  # to restart it, a dead watchdog stays dead indefinitely.
  # ---------------------------------------------------------------------------
  local task_query
  task_query=$(schtasks.exe //Query //TN "CommsLink-DaemonWatchdog" //FO LIST 2>&1 || echo "NOT_FOUND")
  if echo "$task_query" | grep -q "TaskName"; then
    local task_status
    task_status=$(echo "$task_query" | grep "Status:" | awk '{print $NF}')
    status="PASS"; severity="P3"
    message="CommsLink-DaemonWatchdog task registered (status: ${task_status:-unknown})"
  else
    status="FAIL"; severity="P1"
    message="CommsLink-DaemonWatchdog task NOT registered — rc-watchdog will not survive reboot. Run register-james-watchdog.bat as admin"
  fi
  emit_result "$phase" "$tier" "watchdog-task" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 4: AutoDetect-Daily scheduled task registered
  # The auto-detect pipeline finds bugs autonomously. Without the scheduled task,
  # it only runs when manually invoked — defeating the purpose.
  # ---------------------------------------------------------------------------
  task_query=$(schtasks.exe //Query //TN "AutoDetect-Daily" //FO LIST 2>&1 || echo "NOT_FOUND")
  if echo "$task_query" | grep -q "TaskName"; then
    local next_run
    next_run=$(echo "$task_query" | grep "Next Run Time:" | sed 's/Next Run Time:[[:space:]]*//')
    status="PASS"; severity="P3"
    message="AutoDetect-Daily task registered (next: ${next_run:-unknown})"
  else
    status="FAIL"; severity="P1"
    message="AutoDetect-Daily task NOT registered — autonomous bug detection is disabled. Run register-auto-detect-task.bat as admin"
  fi
  emit_result "$phase" "$tier" "autodetect-task" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 5: HKCU/HKLM Run key for rc-watchdog (boot persistence)
  # Scheduled task handles crash recovery. Run key handles boot start.
  # Both are needed for full coverage.
  # ---------------------------------------------------------------------------
  local hkcu_key hklm_key
  hkcu_key=$(reg query "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run" //v RCWatchdog 2>/dev/null || echo "")
  hklm_key=$(reg query "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run" //v RCWatchdog 2>/dev/null || echo "")
  if [[ -n "$hkcu_key" ]] || [[ -n "$hklm_key" ]]; then
    status="PASS"; severity="P3"
    message="RCWatchdog Run key present (boot-start persistence)"
  else
    status="WARN"; severity="P2"
    message="RCWatchdog not in HKCU or HKLM Run — will not auto-start on boot"
  fi
  emit_result "$phase" "$tier" "watchdog-boot-key" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 6: auto-detect suggestions.jsonl recency
  # If the auto-detect pipeline is registered but producing stale output,
  # it's broken silently. Check if suggestions were written in last 24h.
  # ---------------------------------------------------------------------------
  local suggestions="$SCRIPT_DIR/../results/suggestions.jsonl"
  if [[ -f "$suggestions" ]]; then
    local last_ts
    last_ts=$(tail -1 "$suggestions" | jq -r '.run_ts // ""' 2>/dev/null)
    if [[ -n "$last_ts" ]]; then
      # Parse IST timestamp
      local last_epoch now_epoch delta_hours
      last_epoch=$(date -d "$(echo "$last_ts" | sed 's/ IST//')" +%s 2>/dev/null || echo "0")
      now_epoch=$(date +%s)
      if [[ "$last_epoch" -gt 0 ]]; then
        delta_hours=$(( (now_epoch - last_epoch) / 3600 ))
        if [[ "$delta_hours" -lt 26 ]]; then
          status="PASS"; severity="P3"
          message="auto-detect suggestions fresh (last: $last_ts, ${delta_hours}h ago)"
        else
          status="WARN"; severity="P2"
          message="auto-detect suggestions stale (last: $last_ts, ${delta_hours}h ago)"
        fi
      else
        status="WARN"; severity="P2"
        message="auto-detect suggestions timestamp unparseable: $last_ts"
      fi
    else
      status="WARN"; severity="P2"
      message="auto-detect suggestions.jsonl exists but last entry has no timestamp"
    fi
  else
    status="WARN"; severity="P2"
    message="auto-detect suggestions.jsonl not found — pipeline may not have run yet"
  fi
  emit_result "$phase" "$tier" "autodetect-recency" "$status" "$severity" "$message" "$mode" "$venue_state"

  # ---------------------------------------------------------------------------
  # CHECK 7: auto-detect-config.json toggle consistency
  # auto_fix_enabled=true but self_patch_enabled=false is EXPECTED (safe default).
  # auto_fix_enabled=false means NO healing happens even if bugs are found.
  # ---------------------------------------------------------------------------
  local config_file="${SCRIPT_DIR}/../results/auto-detect-config.json"
  if [[ -f "$config_file" ]]; then
    local auto_fix self_patch
    auto_fix=$(jq -r '.auto_fix_enabled' "$config_file" 2>/dev/null)
    self_patch=$(jq -r '.self_patch_enabled' "$config_file" 2>/dev/null)
    if [[ "$auto_fix" == "true" ]]; then
      status="PASS"; severity="P3"
      message="auto_fix_enabled=true, self_patch_enabled=${self_patch} — healing active"
    else
      status="WARN"; severity="P2"
      message="auto_fix_enabled=${auto_fix} — detect-only mode, no automated healing"
    fi
  else
    status="FAIL"; severity="P2"
    message="auto-detect-config.json missing — healing toggle state unknown"
  fi
  emit_result "$phase" "$tier" "healing-toggles" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase67

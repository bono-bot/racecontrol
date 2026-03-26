#!/usr/bin/env bash
# audit/phases/tier3/phase68.sh -- Phase 68: Kiosk Game Launch Timer
# Tier: 3 (Display & UX)
# What: Verifies kiosk renders launch countdown timer during waiting_for_game state.
#   Checks that the LaunchingView and LiveSessionPanel both show elapsed/countdown
#   timer elements, not just a bare spinner. Also verifies progress bar has correct
#   color classes (must NOT be always-red).
#
# Why this exists: 2026-03-26 — kiosk showed only "Game Loading..." spinner with
#   no timer during game launch. Staff couldn't tell if game had been loading 5s or
#   175s out of 180s timeout. Progress bar was also always red (both branches of
#   ternary evaluated to bg-rp-red). Both bugs shipped undetected because no audit
#   checked the actual UI rendering of the launch state.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

# grep -c returns exit 1 when count=0; this wrapper returns 0 instead of triggering || echo
_count() { grep -c "$@" 2>/dev/null || true; }

run_phase68() {
  local phase="68" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local timeout="${DEFAULT_TIMEOUT:-10}"
  local status severity message

  # --- Check 1: LaunchingView has launch countdown timer ---
  local kiosk_src="C:/Users/bono/racingpoint/racecontrol/kiosk/src/components/PodKioskView.tsx"
  if [[ -f "$kiosk_src" ]]; then
    local has_elapsed; has_elapsed=$(_count 'elapsed_seconds' "$kiosk_src")
    local has_timeout; has_timeout=$(_count 'LAUNCH_TIMEOUT_SECS' "$kiosk_src")
    local has_progress; has_progress=$(_count 'progress.*%' "$kiosk_src")

    if [[ "$has_elapsed" -ge 1 ]] && [[ "$has_timeout" -ge 1 ]] && [[ "$has_progress" -ge 1 ]]; then
      status="PASS"; severity="P3"
      message="LaunchingView has launch countdown timer (elapsed=${has_elapsed}, timeout=${has_timeout}, progress=${has_progress} refs)"
    elif [[ "$has_elapsed" -ge 1 ]]; then
      status="WARN"; severity="P2"
      message="LaunchingView references elapsed_seconds but missing countdown UI (timeout=${has_timeout}, progress=${has_progress})"
    else
      status="FAIL"; severity="P1"
      message="LaunchingView has NO launch countdown timer — customer/staff blind to 180s timeout progress"
    fi
  else
    status="WARN"; severity="P2"
    message="PodKioskView.tsx not found at expected path"
  fi
  emit_result "$phase" "$tier" "kiosk-launch-timer-view" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: LiveSessionPanel has launch timer banner ---
  local panel_src="C:/Users/bono/racingpoint/racecontrol/kiosk/src/components/LiveSessionPanel.tsx"
  if [[ -f "$panel_src" ]]; then
    local panel_elapsed; panel_elapsed=$(_count 'elapsed_seconds\|LaunchTimerBanner\|LAUNCH_TIMEOUT' "$panel_src")
    local panel_countdown; panel_countdown=$(_count 'remaining.*60\|localElapsed' "$panel_src")

    if [[ "$panel_elapsed" -ge 2 ]] && [[ "$panel_countdown" -ge 1 ]]; then
      status="PASS"; severity="P3"
      message="LiveSessionPanel has launch timer banner (${panel_elapsed} timer refs, ${panel_countdown} countdown refs)"
    elif [[ "$panel_elapsed" -ge 1 ]]; then
      status="WARN"; severity="P2"
      message="LiveSessionPanel has partial launch timer (elapsed=${panel_elapsed}, countdown=${panel_countdown})"
    else
      status="FAIL"; severity="P1"
      message="LiveSessionPanel has NO launch timer — dashboard blind to game loading progress"
    fi
  else
    status="WARN"; severity="P2"
    message="LiveSessionPanel.tsx not found at expected path"
  fi
  emit_result "$phase" "$tier" "kiosk-launch-timer-panel" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Progress bar color is NOT always-red ---
  # The bug: `isLow ? "bg-rp-red" : "bg-rp-red"` — both branches same color
  if [[ -f "$kiosk_src" ]] && [[ -f "$panel_src" ]]; then
    local always_red_view; always_red_view=$(_count 'isLow.*bg-rp-red.*bg-rp-red' "$kiosk_src")
    local always_red_panel; always_red_panel=$(_count 'bg-rp-red.*:.*bg-rp-red' "$panel_src")
    local healthy_color_view; healthy_color_view=$(_count 'bg-emerald\|bg-green\|bg-blue' "$kiosk_src")
    local healthy_color_panel; healthy_color_panel=$(_count 'bg-emerald\|bg-green\|bg-blue' "$panel_src")

    if [[ "$always_red_view" -ge 1 ]] || [[ "$always_red_panel" -ge 1 ]]; then
      status="FAIL"; severity="P1"
      message="Progress bar always-red bug detected (view=${always_red_view}, panel=${always_red_panel} identical ternary branches)"
    elif [[ "$healthy_color_view" -ge 1 ]] && [[ "$healthy_color_panel" -ge 1 ]]; then
      status="PASS"; severity="P3"
      message="Progress bar has healthy color states (view=${healthy_color_view}, panel=${healthy_color_panel} non-red color refs)"
    else
      status="WARN"; severity="P2"
      message="Progress bar may lack healthy color states (view=${healthy_color_view}, panel=${healthy_color_panel})"
    fi
  else
    status="WARN"; severity="P2"
    message="Cannot check progress bar colors — source files missing"
  fi
  emit_result "$phase" "$tier" "kiosk-progress-bar-color" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Runtime verification — kiosk page loads with static assets ---
  local kiosk_html; kiosk_html=$(http_get "http://192.168.31.23:3300/kiosk/control" "$timeout")
  if [[ -z "$kiosk_html" ]]; then
    kiosk_html=$(http_get "http://192.168.31.23:3300/kiosk" "$timeout")
  fi
  if [[ -n "$kiosk_html" ]]; then
    local has_next; has_next=$(printf '%s' "$kiosk_html" | _count '_next/static')
    if [[ "${has_next:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"
      message="Kiosk control page loads with Next.js static assets (${has_next} refs)"
    else
      status="WARN"; severity="P2"
      message="Kiosk control page loaded but no _next/static references found"
    fi
  else
    status="WARN"; severity="P2"
    message="Kiosk control page unreachable for runtime check"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "kiosk-launch-timer-runtime" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase68

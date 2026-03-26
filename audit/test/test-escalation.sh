#!/usr/bin/env bash
# audit/test/test-escalation.sh -- Offline test suite for escalation ladder (TEST-03)
# 6 tests: TIER-GATE TIER-SENTINEL TIER-ORDER TIER-RETRY-ONLY TIER-SKIP-WOL TIER-SYNTAX
# Uses file-based call tracking because tier functions run in $() subshells
set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

PASS_COUNT=0
FAIL_COUNT=0
_pass() { local name="$1"; echo "PASS: $name"; PASS_COUNT=$((PASS_COUNT + 1)); }
_fail() { local name="$1"; local reason="${2:-}"; echo "FAIL: $name${reason:+ -- $reason}"; FAIL_COUNT=$((FAIL_COUNT + 1)); }

echo "=== audit/test/test-escalation.sh ==="
echo ""
echo "--- TEST-03: Escalation Ladder (Tier Ordering) ---"
echo ""

# ---- TIER-GATE ----
TEST="TIER-GATE: NO_FIX=true -- auto_fix_enabled disabled -- TIER_CALLS empty"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export NO_FIX="true"
  export AUTO_DETECT_CONFIG="$tmp_dir/config.json"
  printf '{"auto_fix_enabled":true,"wol_enabled":false}
' > "$tmp_dir/config.json"
  export RELAY_URL="http://localhost:8766"
  _FLEET_HEALTH_CACHE=""; export _FLEET_HEALTH_CACHE
  CALLS_FILE="$tmp_dir/calls.txt"; export CALLS_FILE; touch "$CALLS_FILE"
  log() { :; }; export -f log
  emit_fix() { :; }; export -f emit_fix
  is_pod_idle() { return 0; }; export -f is_pod_idle
  check_pod_sentinels() { return 0; }; export -f check_pod_sentinels
  safe_remote_exec() { echo "{}"; }; export -f safe_remote_exec
  venue_state_detect() { echo "closed"; }; export -f venue_state_detect
  send_whatsapp() { :; }; export -f send_whatsapp
  _is_cooldown_active() { return 1; }; export -f _is_cooldown_active
  _record_alert() { :; }; export -f _record_alert
  ist_now() { echo "2026-03-26 08:00 IST"; }; export -f ist_now
  _notify_whatsapp_uday() { :; }; export -f _notify_whatsapp_uday
  _notify_bono_ws() { :; }; export -f _notify_bono_ws
  wol_pod() { :; }; export -f wol_pod
  source "$REPO_ROOT/scripts/healing/escalation-engine.sh" 2>/dev/null
  attempt_retry() { echo "retry" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_restart() { echo "restart" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_wol() { echo "wol" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_cloud_failover() { echo "cloud_failover" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  escalate_human() { echo "human" >> "$CALLS_FILE"; }
  export -f attempt_retry attempt_restart attempt_wol attempt_cloud_failover escalate_human
  escalate_pod "192.168.31.89" "rc_agent_down" "P1" 2>/dev/null || true
  actual=$(tr "
" " " < "$CALLS_FILE" | sed "s/ $//")
  rm -rf "$tmp_dir"
  [[ -z "$actual" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "TIER_CALLS not empty with NO_FIX=true"; fi

# ---- TIER-SENTINEL ----
TEST="TIER-SENTINEL: check_pod_sentinels returns 1 -- tiers 1-4 blocked -- only human called"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export NO_FIX="false"
  export AUTO_DETECT_CONFIG="$tmp_dir/config.json"
  printf '{"auto_fix_enabled":true,"wol_enabled":false}
' > "$tmp_dir/config.json"
  export RELAY_URL="http://localhost:8766"
  _FLEET_HEALTH_CACHE=""; export _FLEET_HEALTH_CACHE
  CALLS_FILE="$tmp_dir/calls.txt"; export CALLS_FILE; touch "$CALLS_FILE"
  log() { :; }; export -f log
  emit_fix() { :; }; export -f emit_fix
  is_pod_idle() { return 0; }; export -f is_pod_idle
  check_pod_sentinels() { return 1; }; export -f check_pod_sentinels
  safe_remote_exec() { echo "{}"; }; export -f safe_remote_exec
  venue_state_detect() { echo "closed"; }; export -f venue_state_detect
  send_whatsapp() { :; }; export -f send_whatsapp
  _is_cooldown_active() { return 1; }; export -f _is_cooldown_active
  _record_alert() { :; }; export -f _record_alert
  ist_now() { echo "2026-03-26 08:00 IST"; }; export -f ist_now
  _notify_whatsapp_uday() { :; }; export -f _notify_whatsapp_uday
  _notify_bono_ws() { :; }; export -f _notify_bono_ws
  wol_pod() { :; }; export -f wol_pod
  source "$REPO_ROOT/scripts/healing/escalation-engine.sh" 2>/dev/null
  attempt_retry() { echo "retry" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_restart() { echo "restart" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_wol() { echo "wol" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_cloud_failover() { echo "cloud_failover" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  escalate_human() { echo "human" >> "$CALLS_FILE"; }
  export -f attempt_retry attempt_restart attempt_wol attempt_cloud_failover escalate_human
  escalate_pod "192.168.31.89" "rc_agent_down" "P1" 2>/dev/null || true
  actual=$(tr "
" " " < "$CALLS_FILE" | sed "s/ $//")
  rm -rf "$tmp_dir"
  [[ "$actual" == "human" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "expected only human in TIER_CALLS with sentinel block"; fi

# ---- TIER-ORDER ----
TEST="TIER-ORDER: pod never recovers -- TIER_CALLS == retry restart wol cloud_failover human exact"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export NO_FIX="false"
  export AUTO_DETECT_CONFIG="$tmp_dir/config.json"
  printf '{"auto_fix_enabled":true,"wol_enabled":false}
' > "$tmp_dir/config.json"
  export RELAY_URL="http://localhost:8766"
  _FLEET_HEALTH_CACHE=""; export _FLEET_HEALTH_CACHE
  CALLS_FILE="$tmp_dir/calls.txt"; export CALLS_FILE; touch "$CALLS_FILE"
  log() { :; }; export -f log
  emit_fix() { :; }; export -f emit_fix
  is_pod_idle() { return 0; }; export -f is_pod_idle
  check_pod_sentinels() { return 0; }; export -f check_pod_sentinels
  safe_remote_exec() { echo "{}"; }; export -f safe_remote_exec
  venue_state_detect() { echo "closed"; }; export -f venue_state_detect
  send_whatsapp() { :; }; export -f send_whatsapp
  _is_cooldown_active() { return 1; }; export -f _is_cooldown_active
  _record_alert() { :; }; export -f _record_alert
  ist_now() { echo "2026-03-26 08:00 IST"; }; export -f ist_now
  _notify_whatsapp_uday() { :; }; export -f _notify_whatsapp_uday
  _notify_bono_ws() { :; }; export -f _notify_bono_ws
  wol_pod() { :; }; export -f wol_pod
  source "$REPO_ROOT/scripts/healing/escalation-engine.sh" 2>/dev/null
  attempt_retry() { echo "retry" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_restart() { echo "restart" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_wol() { echo "wol" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_cloud_failover() { echo "cloud_failover" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  escalate_human() { echo "human" >> "$CALLS_FILE"; }
  export -f attempt_retry attempt_restart attempt_wol attempt_cloud_failover escalate_human
  escalate_pod "192.168.31.89" "rc_agent_down" "P1" 2>/dev/null || true
  actual=$(tr "
" " " < "$CALLS_FILE" | sed "s/ $//")
  expected="retry restart wol cloud_failover human"
  rm -rf "$tmp_dir"
  [[ "$actual" == "$expected" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "tier sequence mismatch -- expected: retry restart wol cloud_failover human"; fi

# ---- TIER-RETRY-ONLY ----
TEST="TIER-RETRY-ONLY: attempt_retry returns RESOLVED -- TIER_CALLS == retry only"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export NO_FIX="false"
  export AUTO_DETECT_CONFIG="$tmp_dir/config.json"
  printf '{"auto_fix_enabled":true,"wol_enabled":false}
' > "$tmp_dir/config.json"
  export RELAY_URL="http://localhost:8766"
  _FLEET_HEALTH_CACHE=""; export _FLEET_HEALTH_CACHE
  CALLS_FILE="$tmp_dir/calls.txt"; export CALLS_FILE; touch "$CALLS_FILE"
  log() { :; }; export -f log
  emit_fix() { :; }; export -f emit_fix
  is_pod_idle() { return 0; }; export -f is_pod_idle
  check_pod_sentinels() { return 0; }; export -f check_pod_sentinels
  safe_remote_exec() { echo "{}"; }; export -f safe_remote_exec
  venue_state_detect() { echo "closed"; }; export -f venue_state_detect
  send_whatsapp() { :; }; export -f send_whatsapp
  _is_cooldown_active() { return 1; }; export -f _is_cooldown_active
  _record_alert() { :; }; export -f _record_alert
  ist_now() { echo "2026-03-26 08:00 IST"; }; export -f ist_now
  _notify_whatsapp_uday() { :; }; export -f _notify_whatsapp_uday
  _notify_bono_ws() { :; }; export -f _notify_bono_ws
  wol_pod() { :; }; export -f wol_pod
  source "$REPO_ROOT/scripts/healing/escalation-engine.sh" 2>/dev/null
  attempt_retry() { echo "retry" >> "$CALLS_FILE"; echo "RESOLVED"; return 0; }
  attempt_restart() { echo "restart" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_wol() { echo "wol" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_cloud_failover() { echo "cloud_failover" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  escalate_human() { echo "human" >> "$CALLS_FILE"; }
  export -f attempt_retry attempt_restart attempt_wol attempt_cloud_failover escalate_human
  escalate_pod "192.168.31.89" "rc_agent_down" "P1" 2>/dev/null || true
  actual=$(tr "
" " " < "$CALLS_FILE" | sed "s/ $//")
  rm -rf "$tmp_dir"
  [[ "$actual" == "retry" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "ladder did not stop at retry tier"; fi

# ---- TIER-SKIP-WOL ----
TEST="TIER-SKIP-WOL: wol_enabled=false -- wol returns UNRESOLVED -- ladder continues to human"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export NO_FIX="false"
  export AUTO_DETECT_CONFIG="$tmp_dir/config.json"
  printf '{"auto_fix_enabled":true,"wol_enabled":false}
' > "$tmp_dir/config.json"
  export RELAY_URL="http://localhost:8766"
  _FLEET_HEALTH_CACHE=""; export _FLEET_HEALTH_CACHE
  CALLS_FILE="$tmp_dir/calls.txt"; export CALLS_FILE; touch "$CALLS_FILE"
  log() { :; }; export -f log
  emit_fix() { :; }; export -f emit_fix
  is_pod_idle() { return 0; }; export -f is_pod_idle
  check_pod_sentinels() { return 0; }; export -f check_pod_sentinels
  safe_remote_exec() { echo "{}"; }; export -f safe_remote_exec
  venue_state_detect() { echo "closed"; }; export -f venue_state_detect
  send_whatsapp() { :; }; export -f send_whatsapp
  _is_cooldown_active() { return 1; }; export -f _is_cooldown_active
  _record_alert() { :; }; export -f _record_alert
  ist_now() { echo "2026-03-26 08:00 IST"; }; export -f ist_now
  _notify_whatsapp_uday() { :; }; export -f _notify_whatsapp_uday
  _notify_bono_ws() { :; }; export -f _notify_bono_ws
  wol_pod() { :; }; export -f wol_pod
  source "$REPO_ROOT/scripts/healing/escalation-engine.sh" 2>/dev/null
  attempt_retry() { echo "retry" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_restart() { echo "restart" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_wol() { echo "wol" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  attempt_cloud_failover() { echo "cloud_failover" >> "$CALLS_FILE"; echo "UNRESOLVED"; return 1; }
  escalate_human() { echo "human" >> "$CALLS_FILE"; }
  export -f attempt_retry attempt_restart attempt_wol attempt_cloud_failover escalate_human
  escalate_pod "192.168.31.89" "rc_agent_down" "P1" 2>/dev/null || true
  actual=$(tr "
" " " < "$CALLS_FILE" | sed "s/ $//")
  expected="retry restart wol cloud_failover human"
  rm -rf "$tmp_dir"
  [[ "$actual" == "$expected" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "wol UNRESOLVED did not continue to cloud_failover and human"; fi

# ---- TIER-SYNTAX ----
TEST="TIER-SYNTAX: bash -n on escalation-engine.sh"
if bash -n "$REPO_ROOT/scripts/healing/escalation-engine.sh" 2>/dev/null; then
  _pass "$TEST"
else
  _fail "$TEST" "syntax error in escalation-engine.sh"
fi

echo ""
TOTAL=$((PASS_COUNT + FAIL_COUNT))
echo "${PASS_COUNT}/${TOTAL} tests passed."
[ "$FAIL_COUNT" -gt 0 ] && exit 1
exit 0

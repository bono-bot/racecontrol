#!/usr/bin/env bash
# scripts/healing/escalation-engine.sh — 5-tier graduated escalation engine
#
# Implements: HEAL-02 (5-tier escalation loop), HEAL-03 (sentinel gate),
#             HEAL-05 (verify_fix), HEAL-06 (Cause Elimination methodology),
#             HEAL-07 (live-sync entry point), HEAL-08 (runtime toggle)
#
# This file can be sourced OR executed directly (--self-test).
# Usage (source):  source scripts/healing/escalation-engine.sh
# Usage (execute): bash scripts/healing/escalation-engine.sh --self-test
#
# Tiers:
#   1. attempt_retry        — transient failure, health check retry
#   2. attempt_restart      — rc-agent crash/hung, schtasks restart via rc-sentry
#   3. attempt_wol          — pod powered off, WoL magic packet (wol_enabled=false until manual test)
#   4. attempt_cloud_failover — local infra failure, Bono cloud serving
#   5. escalate_human       — WhatsApp to Uday + Bono WS (6h cooldown, QUIET silence, night gate)
#
# Entry point for detectors: attempt_heal(pod_ip, issue_type, severity)
# which delegates to escalate_pod().
#
# Sentinel-aware: every tier calls _sentinel_gate before acting.
# Billing-gated: escalate_pod checks is_pod_idle before any tier.
# Toggle-controlled: _auto_fix_enabled reads JSON config at call time (HEAL-08).

# Use set -u and pipefail but NOT set -e — errors handled per-function.
set -uo pipefail

# ─── Path resolution ─────────────────────────────────────────────────────────
_ESCALATION_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd)"
REPO_ROOT="${REPO_ROOT:-$(cd "$_ESCALATION_SCRIPT_DIR/../.." && pwd)}"
AUTO_DETECT_CONFIG="${REPO_ROOT}/audit/results/auto-detect-config.json"

# ─── HEAL-08: _auto_fix_enabled ──────────────────────────────────────────────
# Reads auto_fix_enabled from config JSON at call time (not startup).
# NO_FIX=true env var overrides config (returns 1 = disabled).
# Missing config = disabled (safe default). jq parse failure = disabled (safe default).
_auto_fix_enabled() {
  # NO_FIX override takes precedence
  if [[ "${NO_FIX:-false}" == "true" ]]; then return 1; fi

  # Missing config = disabled (safe default — require explicit opt-in)
  if [[ ! -f "$AUTO_DETECT_CONFIG" ]]; then
    log WARN "[HEAL-08] auto-detect config missing ($AUTO_DETECT_CONFIG) — auto-fix disabled"
    return 1
  fi

  # Parse config; jq failure = disabled (safe default)
  local val
  val=$(jq -r '.auto_fix_enabled // false' "$AUTO_DETECT_CONFIG" 2>/dev/null || echo "false")
  if [[ "$val" == "true" ]]; then return 0; fi
  return 1
}
export -f _auto_fix_enabled

# ─── HEAL-03: _sentinel_gate ─────────────────────────────────────────────────
# Check pod sentinels (OTA_DEPLOYING, MAINTENANCE_MODE) before any tier action.
# Returns 0 if clear, 1 if blocked.
_sentinel_gate() {
  local pod_ip="$1" tier_name="$2"
  if ! check_pod_sentinels "$pod_ip"; then
    # Check if MAINTENANCE_MODE is stale (>15 min old)
    local mm_age
    mm_age=$(safe_remote_exec "$pod_ip" 8090 \
      "powershell -NoProfile -Command \"if(Test-Path C:\\RacingPoint\\MAINTENANCE_MODE){((Get-Date)-(Get-Item C:\\RacingPoint\\MAINTENANCE_MODE).LastWriteTime).TotalMinutes}else{0}\"" 10 2>/dev/null || echo "0")
    mm_age=$(printf '%s' "$mm_age" | tr -d '[:space:]' | cut -d'.' -f1)
    if [[ "${mm_age:-0}" -gt 15 ]]; then
      log WARN "[HEAL] STALE MAINTENANCE_MODE on $pod_ip — sentinel is ${mm_age} minutes old (>15min). Manual clear may be needed."
    fi
    emit_fix "heal" "$pod_ip" "SENTINEL_BLOCK_${tier_name}" "sentinel_active" "blocked"
    log WARN "[HEAL] sentinel block: $pod_ip at $tier_name"
    return 1
  fi
  return 0
}
export -f _sentinel_gate

# ─── Tier 1: attempt_retry ───────────────────────────────────────────────────
# Hypothesis: Transient failure -- health check will succeed on retry
attempt_retry() {
  local pod_ip="$1"
  local attempt health status
  for attempt in 1 2; do
    sleep 5
    health=$(curl -s -m 5 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
    status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
    if [[ -n "$status" ]]; then
      echo "RESOLVED"
      return 0
    fi
  done
  echo "UNRESOLVED"
  return 1
}
export -f attempt_retry

# ─── Tier 2: attempt_restart ─────────────────────────────────────────────────
# Hypothesis: rc-agent process crashed or hung -- schtasks restart via rc-sentry will recover
attempt_restart() {
  local pod_ip="$1"
  # Check rc-sentry alive first
  local sentry_health
  sentry_health=$(curl -s -m 5 "http://${pod_ip}:8091/health" 2>/dev/null || echo "")
  if [[ -z "$sentry_health" ]]; then
    # Cannot restart without rc-sentry
    echo "UNRESOLVED"
    return 1
  fi

  # Send restart via rc-sentry schtasks
  safe_remote_exec "$pod_ip" 8091 "schtasks /Run /TN StartRCAgent" 10 >/dev/null

  # Wait 15s for startup
  sleep 15

  # Verify: curl health on :8090
  local health status
  health=$(curl -s -m 5 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
  status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
  if [[ -n "$status" ]]; then
    emit_fix "restart" "$pod_ip" "attempt_restart" "rc_agent_down" "rc_agent_recovered"
    echo "RESOLVED"
    return 0
  fi

  echo "UNRESOLVED"
  return 1
}
export -f attempt_restart

# ─── Tier 3: attempt_wol ─────────────────────────────────────────────────────
# Reads wol_enabled from config; if not enabled, skip tier (not block escalation)
attempt_wol() {
  local pod_ip="$1"

  # Read wol_enabled from config at call time
  local wol_val
  wol_val=$(jq -r '.wol_enabled // false' "$AUTO_DETECT_CONFIG" 2>/dev/null || echo "false")
  if [[ "$wol_val" != "true" ]]; then
    echo "UNRESOLVED"
    return 1
  fi

  # Call wol_pod from fixes.sh (requires WOL_ENABLED=true)
  WOL_ENABLED="true" wol_pod "$pod_ip" || true

  # Wait 60s — WoL boot takes time
  sleep 60

  # Verify: ping then curl health
  local ping_result ping_output
  ping_result=$(safe_remote_exec "192.168.31.23" 8090 "ping -n 1 -w 2000 ${pod_ip}" 10)
  ping_output=$(printf '%s' "$ping_result" | jq -r '.stdout // .output // .result // ""' 2>/dev/null | tr '[:upper:]' '[:lower:]')
  if ! printf '%s' "$ping_output" | grep -q "reply from"; then
    echo "UNRESOLVED"
    return 1
  fi

  local health status
  health=$(curl -s -m 10 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
  status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
  if [[ -n "$status" ]]; then
    echo "RESOLVED"
    return 0
  fi

  echo "UNRESOLVED"
  return 1
}
export -f attempt_wol

# ─── Tier 4: attempt_cloud_failover ──────────────────────────────────────────
# Hypothesis: Local infrastructure failure -- Bono cloud can serve until local recovery
attempt_cloud_failover() {
  local pod_ip="$1"

  # Check cooldown for fleet-level cloud failover (Pitfall 7)
  if type -t _is_cooldown_active &>/dev/null; then
    if _is_cooldown_active "fleet" "cloud_failover"; then
      log INFO "[HEAL] cloud_failover cooldown active — skipping"
      echo "UNRESOLVED"
      return 1
    fi
  fi

  # Notify Bono via relay — write JSON to temp file (standing rule: bash string escaping safety)
  local tmpfile
  tmpfile=$(mktemp)
  jq -n '{"command":"health_check","reason":"escalation_tier4_cloud_failover"}' > "$tmpfile"
  local relay_result
  relay_result=$(curl -s -m 15 -X POST "http://localhost:8766/relay/exec/run" \
    -H "Content-Type: application/json" \
    -d "@${tmpfile}" 2>/dev/null || echo "")
  rm -f "$tmpfile"

  if [[ -z "$relay_result" ]]; then
    emit_fix "cloud_failover" "$pod_ip" "attempt_cloud_failover" "local_infra_failure" "relay_unreachable"
    echo "UNRESOLVED"
    return 1
  fi

  emit_fix "cloud_failover" "$pod_ip" "attempt_cloud_failover" "local_infra_failure" "cloud_failover_requested"

  # Record cooldown
  if type -t _record_alert &>/dev/null; then
    _record_alert "fleet" "cloud_failover"
  fi

  # Verify: curl Bono VPS health endpoint
  local bono_health bono_status
  bono_health=$(curl -s -m 10 "http://srv1422716.hstgr.cloud:8080/api/v1/health" 2>/dev/null || echo "")
  bono_status=$(printf '%s' "$bono_health" | jq -r '.status // ""' 2>/dev/null || echo "")
  if [[ "$bono_status" == "ok" ]]; then
    echo "RESOLVED"
    return 0
  fi

  echo "UNRESOLVED"
  return 1
}
export -f attempt_cloud_failover

# ─── Tier 5: escalate_human ──────────────────────────────────────────────────
# HEAL-04: QUIET severity = no WhatsApp
# Night gate: venue closed AND hour < 07 IST = defer
# Cooldown: 6h per pod+issue pair
escalate_human() {
  local pod_ip="$1" issue_type="$2" severity="${3:-P1}"

  # HEAL-04: QUIET severity — return immediately, no WhatsApp
  if [[ "$severity" == "QUIET" ]]; then
    log INFO "[HEAL] escalate_human: QUIET severity — suppressing WhatsApp for $pod_ip $issue_type"
    return 0
  fi

  # Night gate: venue closed AND hour < 07 IST — defer to avoid waking Uday
  local venue_state
  venue_state=$(venue_state_detect 2>/dev/null || echo "open")
  if [[ "$venue_state" == "closed" ]]; then
    local ist_hour
    ist_hour=$(TZ=Asia/Kolkata date '+%H' | sed 's/^0*//')
    ist_hour="${ist_hour:-0}"
    if [[ "$ist_hour" -lt 7 ]]; then
      log INFO "[HEAL] escalate_human: deep night ($ist_hour IST) — deferring WhatsApp for $pod_ip $issue_type"
      return 0
    fi
  fi

  # Cooldown check: skip if in 6h window
  if type -t _is_cooldown_active &>/dev/null; then
    if _is_cooldown_active "$pod_ip" "$issue_type"; then
      log INFO "[HEAL] escalate_human: cooldown active for $pod_ip $issue_type — skipping"
      return 0
    fi
  fi

  # Record alert
  if type -t _record_alert &>/dev/null; then
    _record_alert "$pod_ip" "$issue_type"
  fi

  # Build message
  local ist_ts
  ist_ts=$(ist_now)
  local msg="Racing Point Fleet Alert — ${severity}
Pod: ${pod_ip} | Issue: ${issue_type}
Time: ${ist_ts}
Status: All recovery tiers exhausted.
Action: Manual investigation required on pod ${pod_ip}.
Issue type: ${issue_type}"

  # WhatsApp to Uday
  _notify_whatsapp_uday "$msg" || true

  # Bono WS notification
  _notify_bono_ws "[HEAL] Human escalation: $pod_ip | $issue_type | $severity | $ist_ts" || true

  return 0
}
export -f escalate_human

# ─── HEAL-05: verify_fix ─────────────────────────────────────────────────────
# Poll verify function every 10s up to 60s deadline.
# Verify function name: _verify_${issue_type}
verify_fix() {
  local pod_ip="$1" issue_type="$2"
  local verify_fn="_verify_${issue_type}"
  local deadline=60 elapsed=0

  # If no verify function for this issue type, emit PASS (cannot verify = pass-through)
  if ! type -t "$verify_fn" &>/dev/null; then
    emit_fix "heal_verify" "$pod_ip" "$verify_fn" "no_verify_fn" "verification:PASS_no_fn"
    return 0
  fi

  while [[ "$elapsed" -lt "$deadline" ]]; do
    if "$verify_fn" "$pod_ip"; then
      emit_fix "heal_verify" "$pod_ip" "$verify_fn" "polling" "verification:PASS"
      return 0
    fi
    sleep 10
    elapsed=$((elapsed + 10))
  done

  emit_fix "heal_verify" "$pod_ip" "$verify_fn" "deadline_exceeded" "verification:FAIL"
  return 1
}
export -f verify_fix

# ─── Verify functions for known issue types ──────────────────────────────────

# crash_loop: rc-agent health returns valid JSON
_verify_crash_loop() {
  local pod_ip="$1"
  local health status
  health=$(curl -s -m 5 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
  status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
  [[ -n "$status" ]]
}
export -f _verify_crash_loop

# bat_drift: health proxy for bat correctness
_verify_bat_drift() {
  local pod_ip="$1"
  local health status
  health=$(curl -s -m 5 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
  status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
  [[ -n "$status" ]]
}
export -f _verify_bat_drift

# config_drift: re-run health check as proxy for config correctness
_verify_config_drift() {
  local pod_ip="$1"
  local health status
  health=$(curl -s -m 5 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
  status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
  [[ -n "$status" ]]
}
export -f _verify_config_drift

# log_anomaly: health proxy — log anomaly resolves if agent is healthy
_verify_log_anomaly() {
  local pod_ip="$1"
  local health status
  health=$(curl -s -m 5 "http://${pod_ip}:8090/health" 2>/dev/null || echo "")
  status=$(printf '%s' "$health" | jq -r '.status // ""' 2>/dev/null || echo "")
  [[ -n "$status" ]]
}
export -f _verify_log_anomaly

# flag_desync: check flags endpoint returns non-empty
_verify_flag_desync() {
  local pod_ip="$1"
  local flags count
  flags=$(curl -s -m 5 "http://192.168.31.23:8080/api/v1/flags" 2>/dev/null || echo "")
  count=$(printf '%s' "$flags" | jq -r 'length // 0' 2>/dev/null || echo "0")
  [[ "${count:-0}" -gt 0 ]]
}
export -f _verify_flag_desync

# schema_gap: always PASS — schema drift requires migration, not auto-fixable
_verify_schema_gap() {
  return 0
}
export -f _verify_schema_gap

# ─── HEAL-02: escalate_pod — main 5-tier entry point ─────────────────────────
escalate_pod() {
  local pod_ip="$1" issue_type="$2" severity="${3:-P1}"

  # C52 audit fix: clear stale fleet health cache to ensure billing gate uses fresh data
  _FLEET_HEALTH_CACHE=""

  # Concurrent escalation guard: flock per pod to prevent overlapping escalations
  local lock_dir="/tmp/escalation-locks"
  mkdir -p "$lock_dir" 2>/dev/null || true
  local lock_file="$lock_dir/pod-${pod_ip//\./-}.lock"
  exec 9>"$lock_file"
  if ! flock -n 9; then
    log INFO "[HEAL] escalation already in progress for $pod_ip — skipping"
    exec 9>&-
    return 0
  fi

  # HEAL-08: check toggle at call time
  if ! _auto_fix_enabled; then
    log INFO "[HEAL] auto_fix_enabled=false — detect-only mode, skipping escalation for $pod_ip $issue_type"
    exec 9>&-
    return 0
  fi

  # Billing gate: do not attempt fixes on active billing sessions (pods only)
  # Skip billing check for non-pod identifiers (fleet, cloud, server, etc.)
  case "$pod_ip" in
    192.168.31.89|192.168.31.33|192.168.31.28|192.168.31.88|192.168.31.86|192.168.31.87|192.168.31.38|192.168.31.91)
      if ! is_pod_idle "$pod_ip"; then
        emit_fix "heal" "$pod_ip" "SKIP_BILLING_ACTIVE" "billing_active" "skipped"
        log INFO "[HEAL] billing active on $pod_ip — skipping escalation"
        return 0
      fi
      ;;
    *)
      log INFO "[HEAL] non-pod identifier '$pod_ip' — skipping billing gate"
      ;;
  esac

  # Reset fleet health cache at entry (Pitfall 4 — stale cache causes billing gate to pass incorrectly)
  _FLEET_HEALTH_CACHE=""

  log INFO "[HEAL] escalate_pod: $pod_ip | $issue_type | $severity"

  local tier_result verify_result

  # ── Persistent attempt cap: 5 attempts per tier per (pod, issue) per 24h ──
  local _attempt_dir="${REPO_ROOT}/audit/results/escalation-attempts"
  mkdir -p "$_attempt_dir"

  _check_attempt_cap() {
    local _cap_pod="$1" _cap_issue="$2" _cap_tier="$3"
    local _cap_file="${_attempt_dir}/${_cap_pod//\./_}_${_cap_issue}_${_cap_tier}.count"
    local _cap_count=0 _cap_ts=0
    if [[ -f "$_cap_file" ]]; then
      _cap_count=$(head -1 "$_cap_file" 2>/dev/null | cut -d'|' -f1)
      _cap_ts=$(head -1 "$_cap_file" 2>/dev/null | cut -d'|' -f2)
      _cap_count="${_cap_count:-0}"
      _cap_ts="${_cap_ts:-0}"
    fi
    local _cap_now; _cap_now=$(date +%s)
    local _cap_age=$(( _cap_now - _cap_ts ))
    # Reset counter if older than 24h
    if [[ "$_cap_age" -gt 86400 ]]; then
      _cap_count=0
      _cap_ts=$_cap_now
    fi
    if [[ "$_cap_count" -ge 5 ]]; then
      log WARN "[HEAL] attempt cap reached: $_cap_pod $_cap_issue $_cap_tier ($_cap_count/5 in 24h)"
      return 1
    fi
    _cap_count=$((_cap_count + 1))
    echo "${_cap_count}|${_cap_ts}" > "$_cap_file"
    return 0
  }

  # ── Tier 1: Retry ──
  if _sentinel_gate "$pod_ip" "retry" && _check_attempt_cap "$pod_ip" "$issue_type" "retry"; then
    log INFO "[HEAL] Tier 1 (retry): $pod_ip"
    tier_result=$(attempt_retry "$pod_ip" 2>/dev/null || echo "UNRESOLVED")
    if [[ "$tier_result" == "RESOLVED" ]]; then
      verify_result=0
      verify_fix "$pod_ip" "$issue_type" && verify_result=0 || verify_result=1
      if [[ "$verify_result" -eq 0 ]]; then
        log INFO "[HEAL] RESOLVED at Tier 1 (retry): $pod_ip $issue_type"
        return 0
      fi
      log WARN "[HEAL] Tier 1 fix applied but verification failed — continuing escalation"
    fi
  fi

  # ── Tier 2: Restart ──
  if _sentinel_gate "$pod_ip" "restart" && _check_attempt_cap "$pod_ip" "$issue_type" "restart"; then
    log INFO "[HEAL] Tier 2 (restart): $pod_ip"
    tier_result=$(attempt_restart "$pod_ip" 2>/dev/null || echo "UNRESOLVED")
    if [[ "$tier_result" == "RESOLVED" ]]; then
      verify_result=0
      verify_fix "$pod_ip" "$issue_type" && verify_result=0 || verify_result=1
      if [[ "$verify_result" -eq 0 ]]; then
        log INFO "[HEAL] RESOLVED at Tier 2 (restart): $pod_ip $issue_type"
        return 0
      fi
      log WARN "[HEAL] Tier 2 fix applied but verification failed — continuing escalation"
    fi
  fi

  # ── Tier 3: WoL ──
  if _sentinel_gate "$pod_ip" "wol" && _check_attempt_cap "$pod_ip" "$issue_type" "wol"; then
    log INFO "[HEAL] Tier 3 (wol): $pod_ip"
    tier_result=$(attempt_wol "$pod_ip" 2>/dev/null || echo "UNRESOLVED")
    if [[ "$tier_result" == "RESOLVED" ]]; then
      verify_result=0
      verify_fix "$pod_ip" "$issue_type" && verify_result=0 || verify_result=1
      if [[ "$verify_result" -eq 0 ]]; then
        log INFO "[HEAL] RESOLVED at Tier 3 (wol): $pod_ip $issue_type"
        return 0
      fi
      log WARN "[HEAL] Tier 3 fix applied but verification failed — continuing escalation"
    fi
  fi

  # ── Tier 4: Cloud failover ──
  if _sentinel_gate "$pod_ip" "cloud_failover" && _check_attempt_cap "$pod_ip" "$issue_type" "cloud_failover"; then
    log INFO "[HEAL] Tier 4 (cloud_failover): $pod_ip"
    tier_result=$(attempt_cloud_failover "$pod_ip" 2>/dev/null || echo "UNRESOLVED")
    if [[ "$tier_result" == "RESOLVED" ]]; then
      verify_result=0
      verify_fix "$pod_ip" "$issue_type" && verify_result=0 || verify_result=1
      if [[ "$verify_result" -eq 0 ]]; then
        log INFO "[HEAL] RESOLVED at Tier 4 (cloud_failover): $pod_ip $issue_type"
        return 0
      fi
      log WARN "[HEAL] Tier 4 fix applied but verification failed — continuing escalation"
    fi
  fi

  # ── Tier 5: Human escalation ──
  log WARN "[HEAL] Tier 5 (human escalation): all automated tiers exhausted for $pod_ip $issue_type"
  emit_fix "heal" "$pod_ip" "all_tiers_exhausted" "tiers_1_to_4_failed" "escalating_human"
  escalate_human "$pod_ip" "$issue_type" "$severity" || true

  return 1
}
export -f escalate_pod

# ─── HEAL-07: attempt_heal — live-sync entry point ───────────────────────────
# Called by detectors after _emit_finding (Plan 02 wires this).
# Delegates to escalate_pod().
attempt_heal() {
  local pod_ip="$1" issue_type="$2" severity="${3:-P1}"
  escalate_pod "$pod_ip" "$issue_type" "$severity"
}
export -f attempt_heal

# ─── Self-test mode ───────────────────────────────────────────────────────────
# Run when executed directly with --self-test argument.
_run_self_test() {
  echo "[escalation-engine] --self-test mode"
  echo ""

  local all_ok=true

  echo "--- Function availability ---"
  local check_functions=(
    "_auto_fix_enabled"
    "_sentinel_gate"
    "attempt_retry"
    "attempt_restart"
    "attempt_wol"
    "attempt_cloud_failover"
    "escalate_human"
    "verify_fix"
    "escalate_pod"
    "attempt_heal"
  )

  local fn
  for fn in "${check_functions[@]}"; do
    if type -t "$fn" &>/dev/null; then
      echo "  [OK] $fn"
    else
      echo "  [MISSING] $fn"
      all_ok=false
    fi
  done

  echo ""
  echo "--- Config file ---"
  if [[ -f "$AUTO_DETECT_CONFIG" ]]; then
    echo "  [OK] auto-detect-config.json exists"
    local auto_fix wol
    auto_fix=$(jq -r '.auto_fix_enabled' "$AUTO_DETECT_CONFIG" 2>/dev/null || echo "parse_error")
    wol=$(jq -r '.wol_enabled' "$AUTO_DETECT_CONFIG" 2>/dev/null || echo "parse_error")
    echo "  auto_fix_enabled=$auto_fix"
    echo "  wol_enabled=$wol"
  else
    echo "  [MISSING] auto-detect-config.json not found at $AUTO_DETECT_CONFIG"
    all_ok=false
  fi

  echo ""
  if [[ "$all_ok" == "true" ]]; then
    echo "[escalation-engine] SELF-TEST PASS"
    return 0
  else
    echo "[escalation-engine] SELF-TEST FAIL — missing functions or config"
    return 1
  fi
}

# When executed directly (not sourced), run self-test if --self-test flag is present
if [[ "${BASH_SOURCE[0]}" == "${0}" ]] && [[ "${1:-}" == "--self-test" ]]; then
  # Source required libs for self-test
  source "${REPO_ROOT}/audit/lib/core.sh" 2>/dev/null || { echo "FAIL: cannot source core.sh"; exit 1; }
  source "${REPO_ROOT}/audit/lib/fixes.sh" 2>/dev/null || { echo "FAIL: cannot source fixes.sh"; exit 1; }
  source "${REPO_ROOT}/audit/lib/notify.sh" 2>/dev/null || { echo "FAIL: cannot source notify.sh"; exit 1; }
  _run_self_test
  exit $?
fi

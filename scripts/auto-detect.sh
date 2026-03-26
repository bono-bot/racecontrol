#!/usr/bin/env bash
# auto-detect.sh — Autonomous Bug Detection & Self-Healing Pipeline
#
# Chains: Audit Protocol → Quality Gate → E2E → Cascade Check → Auto-Fix → Verify → Notify
# Runs unattended. All debugging features: Cause Elimination, Standing Rules, Debug First Time Right.
#
# Usage:
#   AUDIT_PIN=261121 bash scripts/auto-detect.sh [--mode quick|standard|full] [--dry-run] [--no-fix] [--no-notify]
#
# Exit codes:
#   0 — all clear, no bugs found (or all auto-fixed)
#   1 — bugs found, some unfixable (report sent)
#   2 — fatal prerequisite error

set -euo pipefail

# ─── Configuration ────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
export REPO_ROOT
COMMS_LINK_DIR="$(cd "$REPO_ROOT/../comms-link" && pwd)"
COMMS_PSK="${COMMS_PSK:-85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0}"
COMMS_URL="${COMMS_URL:-ws://srv1422716.hstgr.cloud:8765}"
RELAY_URL="${RELAY_URL:-http://localhost:8766}"
SERVER_URL="${SERVER_URL:-http://192.168.31.23:8080}"
AUDIT_PIN="${AUDIT_PIN:-}"
MODE="${MODE:-standard}"
DRY_RUN=false
NO_FIX=false
NO_NOTIFY=false
TIMESTAMP=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')
TIMESTAMP_FILE=$(TZ=Asia/Kolkata date '+%Y-%m-%d_%H-%M')
RESULT_DIR="$REPO_ROOT/audit/results/auto-detect-${TIMESTAMP_FILE}"
LOG_FILE="$RESULT_DIR/auto-detect.log"

# ─── Parse Args ───────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode) MODE="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    --no-fix) NO_FIX=true; shift ;;
    --no-notify) NO_NOTIFY=true; shift ;;
    *) echo "Unknown arg: $1"; exit 2 ;;
  esac
done

# Export env vars needed by escalation engine (sourced later)
export NO_FIX

# ─── Venue-State-Aware Mode Selection (SCHED-05) ─────────────────────────────
# shellcheck source=audit/lib/core.sh
source "$REPO_ROOT/audit/lib/core.sh"
# Source fixes.sh for APPROVED_FIXES, is_pod_idle, check_pod_sentinels
# shellcheck source=audit/lib/fixes.sh
source "$REPO_ROOT/audit/lib/fixes.sh"
# Source notify.sh for WhatsApp, Bono WS, INBOX channels
# shellcheck source=audit/lib/notify.sh
source "$REPO_ROOT/audit/lib/notify.sh"
# HEAL-07: Source escalation engine for live-sync healing
if [[ -f "$SCRIPT_DIR/healing/escalation-engine.sh" ]]; then
  # shellcheck source=/dev/null
  source "$SCRIPT_DIR/healing/escalation-engine.sh"
fi
# COORD-01/04: Source coordination state module
COORD_SOURCE="$SCRIPT_DIR/coordination/coord-state.sh"
if [[ -f "$COORD_SOURCE" ]]; then
  # shellcheck source=scripts/coordination/coord-state.sh
  source "$COORD_SOURCE"
fi
FLEET_HEALTH_ENDPOINT="${SERVER_URL}/api/v1/fleet/health"
export FLEET_HEALTH_ENDPOINT

if [[ $(type -t venue_state_detect) == "function" ]]; then
  DETECTED_VENUE_STATE=$(venue_state_detect 2>/dev/null || echo "closed")
else
  DETECTED_VENUE_STATE="closed"
fi

if [[ "$DETECTED_VENUE_STATE" == "open" ]] && [[ "$MODE" != "quick" ]]; then
  echo "[INFO] Venue OPEN -- overriding mode to quick (SCHED-05)"
  MODE="quick"
fi

# ─── Prerequisites ────────────────────────────────────────────────────────────
if [[ -z "$AUDIT_PIN" ]]; then
  echo "FATAL: AUDIT_PIN env var required"
  exit 2
fi

for cmd in jq curl node; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "FATAL: $cmd not found in PATH"
    exit 2
  fi
done

mkdir -p "$RESULT_DIR"

# ─── PID File Run Guard (SCHED-03) ────────────────────────────────────────────
PID_FILE="/tmp/auto-detect.pid"

_acquire_run_lock() {
  if [[ -f "$PID_FILE" ]]; then
    local existing_pid
    existing_pid=$(cat "$PID_FILE" 2>/dev/null | tr -d '[:space:]')
    if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
      echo "[$(TZ=Asia/Kolkata date '+%H:%M:%S')] [INFO] auto-detect already running (PID $existing_pid). Exiting."
      exit 0
    fi
    rm -f "$PID_FILE"
  fi
  echo $$ > "$PID_FILE"
}

# shellcheck disable=SC2064
trap "rm -f $PID_FILE" EXIT
_acquire_run_lock
# COORD-01: Write AUTO_DETECT_ACTIVE coordination lock
if [[ $(type -t write_active_lock) == "function" ]]; then
  write_active_lock
fi
# Extend trap to also clear coord lock on exit (replaces the initial trap above)
# shellcheck disable=SC2064
trap "rm -f $PID_FILE; clear_active_lock 2>/dev/null || true" EXIT

# ─── Logging ──────────────────────────────────────────────────────────────────
log() {
  local level="$1"; shift
  local msg="[$(TZ=Asia/Kolkata date '+%H:%M:%S')] [$level] $*"
  echo "$msg" | tee -a "$LOG_FILE"
}

# ─── Escalation Cooldown (SCHED-04) ──────────────────────────────────────────
COOLDOWN_FILE="$REPO_ROOT/audit/results/auto-detect-cooldown.json"
ESCALATION_COOLDOWN_SECS=21600  # 6 hours

_is_cooldown_active() {
  local pod="$1" issue="$2"
  local key="${pod}:${issue}"
  if [[ ! -f "$COOLDOWN_FILE" ]]; then return 1; fi
  local last_ts now_ts elapsed
  last_ts=$(jq -r --arg key "$key" '.[$key] // 0' "$COOLDOWN_FILE" 2>/dev/null || echo "0")
  now_ts=$(date +%s)
  elapsed=$(( now_ts - last_ts ))
  if [[ "$elapsed" -lt "$ESCALATION_COOLDOWN_SECS" ]]; then return 0; fi
  return 1
}

_record_alert() {
  local pod="$1" issue="$2"
  local key="${pod}:${issue}"
  local now_ts existing
  now_ts=$(date +%s)
  existing="{}"
  [[ -f "$COOLDOWN_FILE" ]] && existing=$(cat "$COOLDOWN_FILE" 2>/dev/null || echo "{}")
  printf "%s" "$existing" | jq --arg key "$key" --argjson ts "$now_ts" \
    '.[$key] = $ts' > "${COOLDOWN_FILE}.tmp" && mv "${COOLDOWN_FILE}.tmp" "$COOLDOWN_FILE"
}

# ─── Step Results Tracking ────────────────────────────────────────────────────
declare -A STEP_RESULTS
BUGS_FOUND=0
BUGS_FIXED=0
BUGS_UNFIXED=0
CASCADE_UPDATES=0

record_step() {
  local step="$1" status="$2" detail="$3"
  STEP_RESULTS["$step"]="$status"
  local json="{\"step\":\"$step\",\"status\":\"$status\",\"detail\":\"$detail\",\"timestamp\":\"$(TZ=Asia/Kolkata date -Iseconds)\"}"
  echo "$json" >> "$RESULT_DIR/steps.jsonl"
  if [[ "$status" == "FAIL" ]]; then
    BUGS_FOUND=$((BUGS_FOUND + 1))
  fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# STEP 1: AUDIT PROTOCOL
# ═══════════════════════════════════════════════════════════════════════════════
run_audit() {
  log INFO "=== STEP 1: Audit Protocol (mode=$MODE) ==="

  if [[ "$DRY_RUN" == "true" ]]; then
    log INFO "[DRY-RUN] Would run: AUDIT_PIN=*** bash audit/audit.sh --mode $MODE --auto-fix --commit"
    record_step "audit" "SKIP" "dry-run"
    return 0
  fi

  local audit_flags="--mode $MODE"
  if [[ "$NO_FIX" != "true" ]]; then
    audit_flags="$audit_flags --auto-fix"
  fi

  cd "$REPO_ROOT"
  local audit_output
  audit_output=$(AUDIT_PIN="$AUDIT_PIN" bash audit/audit.sh $audit_flags 2>&1) || true
  echo "$audit_output" >> "$LOG_FILE"

  # Parse audit results
  local latest_dir
  latest_dir=$(echo "$audit_output" | grep -o 'Results in: .*' | sed 's/Results in: //' | tail -1 || true)
  if [[ -z "$latest_dir" ]]; then
    latest_dir=$(ls -td "$REPO_ROOT"/audit/results/2026-* 2>/dev/null | head -1 || true)
  fi

  if [[ -n "$latest_dir" ]] && [[ -f "$latest_dir/audit-summary.json" ]]; then
    local fail_count warn_count pass_count
    fail_count=$(jq -r '.counts.fail // 0' "$latest_dir/audit-summary.json" 2>/dev/null || echo "0")
    warn_count=$(jq -r '.counts.warn // 0' "$latest_dir/audit-summary.json" 2>/dev/null || echo "0")
    pass_count=$(jq -r '.counts.pass // 0' "$latest_dir/audit-summary.json" 2>/dev/null || echo "0")

    log INFO "Audit results: PASS=$pass_count, WARN=$warn_count, FAIL=$fail_count"

    if [[ "$fail_count" -gt 0 ]]; then
      record_step "audit" "FAIL" "fail=$fail_count,warn=$warn_count,pass=$pass_count"
      # Copy audit report to our result dir
      cp "$latest_dir/audit-summary.json" "$RESULT_DIR/" 2>/dev/null || true
      cp "$latest_dir/audit-report.md" "$RESULT_DIR/" 2>/dev/null || true
    elif [[ "$warn_count" -gt 0 ]]; then
      record_step "audit" "WARN" "warn=$warn_count,pass=$pass_count"
    else
      record_step "audit" "PASS" "pass=$pass_count"
    fi
  else
    log WARN "Could not parse audit results"
    record_step "audit" "WARN" "results_unparseable"
  fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# STEP 2: QUALITY GATE (comms-link test suite)
# ═══════════════════════════════════════════════════════════════════════════════
run_quality_gate() {
  log INFO "=== STEP 2: Quality Gate (comms-link test suite) ==="

  if [[ "$DRY_RUN" == "true" ]]; then
    log INFO "[DRY-RUN] Would run: COMMS_PSK=*** bash test/run-all.sh"
    record_step "quality_gate" "SKIP" "dry-run"
    return 0
  fi

  cd "$COMMS_LINK_DIR"
  local gate_output gate_exit=0
  gate_output=$(COMMS_PSK="$COMMS_PSK" COMMS_URL="$COMMS_URL" bash test/run-all.sh 2>&1) || gate_exit=$?
  echo "$gate_output" >> "$LOG_FILE"

  if [[ $gate_exit -eq 0 ]]; then
    log INFO "Quality Gate: ALL PASS"
    record_step "quality_gate" "PASS" "all_suites_pass"
  else
    log ERROR "Quality Gate: FAILED (exit=$gate_exit)"
    record_step "quality_gate" "FAIL" "exit_code=$gate_exit"
  fi
  cd "$REPO_ROOT"
}

# ═══════════════════════════════════════════════════════════════════════════════
# STEP 3: E2E HEALTH VERIFICATION
# ═══════════════════════════════════════════════════════════════════════════════
run_e2e_health() {
  log INFO "=== STEP 3: E2E Health Verification ==="

  if [[ "$DRY_RUN" == "true" ]]; then
    record_step "e2e_health" "SKIP" "dry-run"
    return 0
  fi

  local all_ok=true

  # 3a: Server health
  local server_health
  server_health=$(curl -s --max-time 10 "$SERVER_URL/api/v1/health" 2>/dev/null || echo "")
  local server_status
  server_status=$(echo "$server_health" | jq -r '.status // ""' 2>/dev/null || echo "")
  if [[ "$server_status" == "ok" ]]; then
    log INFO "Server: OK (build=$(echo "$server_health" | jq -r '.build_id // "?"'))"
  else
    log ERROR "Server: UNREACHABLE or unhealthy"
    all_ok=false
  fi

  # 3b: Bono VPS health
  local bono_health
  bono_health=$(curl -s --max-time 10 "http://srv1422716.hstgr.cloud:8080/api/v1/health" 2>/dev/null || echo "")
  local bono_status
  bono_status=$(echo "$bono_health" | jq -r '.status // ""' 2>/dev/null || echo "")
  if [[ "$bono_status" == "ok" ]]; then
    log INFO "Bono VPS: OK"
  else
    log WARN "Bono VPS: unreachable"
  fi

  # 3c: Comms-link relay
  local relay_health
  relay_health=$(curl -s --max-time 5 "$RELAY_URL/relay/health" 2>/dev/null || echo "")
  local relay_connected
  relay_connected=$(echo "$relay_health" | jq -r '.connected // false' 2>/dev/null || echo "false")
  if [[ "$relay_connected" == "true" ]]; then
    log INFO "Relay: CONNECTED"
  else
    log ERROR "Relay: DISCONNECTED"
    all_ok=false
  fi

  # 3d: Exec round-trip (standing rule: live E2E verification)
  local exec_result
  exec_result=$(curl -s --max-time 15 -X POST "$RELAY_URL/relay/exec/run" \
    -H "Content-Type: application/json" \
    -d '{"command":"node_version","reason":"auto-detect E2E"}' 2>/dev/null || echo "")
  local exec_exit
  exec_exit=$(echo "$exec_result" | jq -r '.exitCode // -1' 2>/dev/null || echo "-1")
  if [[ "$exec_exit" == "0" ]]; then
    log INFO "Exec round-trip: OK"
  else
    log ERROR "Exec round-trip: FAILED (exit=$exec_exit)"
    all_ok=false
  fi

  # 3e: Chain round-trip
  local chain_result
  chain_result=$(curl -s --max-time 30 -X POST "$RELAY_URL/relay/chain/run" \
    -H "Content-Type: application/json" \
    -d '{"steps":[{"command":"node_version"},{"command":"health_check"}]}' 2>/dev/null || echo "")
  local chain_status
  chain_status=$(echo "$chain_result" | jq -r '.status // ""' 2>/dev/null || echo "")
  local chain_status_lower
  chain_status_lower=$(echo "$chain_status" | tr '[:upper:]' '[:lower:]')
  if [[ "$chain_status_lower" == "ok" ]]; then
    log INFO "Chain round-trip: OK"
  else
    log WARN "Chain round-trip: status=$chain_status"
  fi

  # 3f: Next.js apps health
  local nextjs_ok=true
  for app_check in "web:3200:/api/health" "admin:3201:/api/health" "kiosk:3300:/kiosk/api/health"; do
    IFS=':' read -r app_name app_port app_path <<< "$app_check"
    local app_health
    app_health=$(curl -s --max-time 5 "http://192.168.31.23:${app_port}${app_path}" 2>/dev/null || echo "")
    local app_status
    app_status=$(echo "$app_health" | jq -r '.status // ""' 2>/dev/null || echo "")
    if [[ "$app_status" == "ok" ]]; then
      log INFO "Next.js $app_name: OK"
    else
      log WARN "Next.js $app_name: $app_status (port $app_port)"
      nextjs_ok=false
    fi
  done

  if [[ "$all_ok" == "true" ]]; then
    record_step "e2e_health" "PASS" "server+relay+exec+chain all OK"
  else
    record_step "e2e_health" "FAIL" "one or more critical services down"
  fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# STEP 4: CASCADE CHECK — Cross-System Bug Detection
# ═══════════════════════════════════════════════════════════════════════════════
run_cascade_check() {
  log INFO "=== STEP 4: Cascade Check ==="

  if [[ "$DRY_RUN" == "true" ]]; then
    record_step "cascade" "SKIP" "dry-run"
    return 0
  fi

  local cascade_issues=0

  # DET-07: Source cascade detection framework and run all detector modules
  if [[ -f "$SCRIPT_DIR/cascade.sh" ]]; then
    # shellcheck source=scripts/cascade.sh
    source "$SCRIPT_DIR/cascade.sh"
  fi
  if [[ $(type -t run_all_detectors) == "function" ]]; then
    run_all_detectors
  fi
  # Add detector findings to cascade_issues total
  cascade_issues=$((cascade_issues + ${DETECTOR_FINDINGS:-0}))

  # 4a: Check server build matches HEAD
  local server_build
  server_build=$(curl -s --max-time 5 "$SERVER_URL/api/v1/health" 2>/dev/null | jq -r '.build_id // ""' 2>/dev/null || echo "")
  local head_short
  head_short=$(cd "$REPO_ROOT" && git rev-parse --short HEAD 2>/dev/null || echo "")
  if [[ -n "$server_build" ]] && [[ -n "$head_short" ]] && [[ "$server_build" != "$head_short" ]]; then
    # Check if there are actual code changes between builds
    local code_diff
    code_diff=$(cd "$REPO_ROOT" && git log --oneline "${server_build}..HEAD" -- crates/ 2>/dev/null | head -5 || echo "")
    if [[ -n "$code_diff" ]]; then
      log WARN "BUILD DRIFT: server=$server_build, HEAD=$head_short — code changes detected"
      cascade_issues=$((cascade_issues + 1))
    else
      log INFO "Build diff is docs-only (server=$server_build, HEAD=$head_short)"
    fi
  else
    log INFO "Build check: server=$server_build, HEAD=$head_short"
  fi

  # 4b: Check pod build consistency (all pods should run same build)
  local fleet_health
  fleet_health=$(curl -s --max-time 10 "$SERVER_URL/api/v1/fleet/health" 2>/dev/null || echo "[]")
  local pod_builds
  pod_builds=$(echo "$fleet_health" | jq -r '.[].build_id // empty' 2>/dev/null | sort -u || echo "")
  local unique_builds
  unique_builds=$(echo "$pod_builds" | wc -l | tr -d ' ')
  if [[ "$unique_builds" -gt 1 ]] && [[ -n "$pod_builds" ]]; then
    log WARN "POD BUILD INCONSISTENCY: $unique_builds different builds across fleet"
    log WARN "  Builds: $(echo "$pod_builds" | tr '\n' ' ')"
    cascade_issues=$((cascade_issues + 1))
  elif [[ -n "$pod_builds" ]]; then
    log INFO "Pod builds: consistent ($(echo "$pod_builds" | head -1))"
  fi

  # 4c: Check cloud build matches venue
  local cloud_build
  cloud_build=$(curl -s --max-time 10 "http://srv1422716.hstgr.cloud:8080/api/v1/health" 2>/dev/null | jq -r '.build_id // ""' 2>/dev/null || echo "")
  if [[ -n "$cloud_build" ]] && [[ -n "$server_build" ]] && [[ "$cloud_build" != "$server_build" ]]; then
    log WARN "CLOUD-VENUE BUILD MISMATCH: cloud=$cloud_build, venue=$server_build"
    cascade_issues=$((cascade_issues + 1))
  else
    log INFO "Cloud-venue: matched ($cloud_build)"
  fi

  # 4d: Check comms-link is synced (James ↔ Bono)
  local james_hash
  james_hash=$(cd "$COMMS_LINK_DIR" && git rev-parse --short HEAD 2>/dev/null || echo "")
  local bono_hash_result
  bono_hash_result=$(curl -s --max-time 15 -X POST "$RELAY_URL/relay/exec/run" \
    -H "Content-Type: application/json" \
    -d '{"command":"git_log","reason":"auto-detect cascade check"}' 2>/dev/null || echo "")
  local bono_hash
  bono_hash=$(echo "$bono_hash_result" | jq -r '.stdout // ""' 2>/dev/null | head -1 | awk '{print $1}' || echo "")
  if [[ -n "$james_hash" ]] && [[ -n "$bono_hash" ]] && [[ "$james_hash" != "$bono_hash" ]]; then
    log WARN "COMMS-LINK DESYNC: james=$james_hash, bono=$bono_hash"
    cascade_issues=$((cascade_issues + 1))
    # Auto-fix: tell Bono to pull
    if [[ "$NO_FIX" != "true" ]]; then
      log INFO "  Auto-fix: telling Bono to git pull..."
      curl -s --max-time 15 -X POST "$RELAY_URL/relay/exec/run" \
        -H "Content-Type: application/json" \
        -d '{"command":"git_pull","reason":"auto-detect: comms-link desync fix"}' >/dev/null 2>&1 || true
      CASCADE_UPDATES=$((CASCADE_UPDATES + 1))
      BUGS_FIXED=$((BUGS_FIXED + 1))
    fi
  else
    log INFO "Comms-link: synced ($james_hash)"
  fi

  # 4e: Standing rules compliance — check git dirty state
  local racecontrol_dirty
  racecontrol_dirty=$(cd "$REPO_ROOT" && git status --porcelain | wc -l | tr -d ' ')
  if [[ "$racecontrol_dirty" -gt 0 ]]; then
    log WARN "UNCOMMITTED CHANGES: $racecontrol_dirty files in racecontrol"
  fi

  local commslink_dirty
  commslink_dirty=$(cd "$COMMS_LINK_DIR" && git status --porcelain | wc -l | tr -d ' ')
  if [[ "$commslink_dirty" -gt 0 ]]; then
    log WARN "UNCOMMITTED CHANGES: $commslink_dirty files in comms-link"
  fi

  if [[ "$cascade_issues" -eq 0 ]]; then
    record_step "cascade" "PASS" "all systems consistent"
  else
    record_step "cascade" "WARN" "issues=$cascade_issues"
    BUGS_FOUND=$((BUGS_FOUND + cascade_issues))
  fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# STEP 5: STANDING RULES VERIFICATION
# ═══════════════════════════════════════════════════════════════════════════════
run_standing_rules_check() {
  log INFO "=== STEP 5: Standing Rules Verification ==="

  if [[ "$DRY_RUN" == "true" ]]; then
    record_step "standing_rules" "SKIP" "dry-run"
    return 0
  fi

  local violations=0

  # SR-01: Auto-push check — are there local commits not pushed?
  local unpushed
  unpushed=$(cd "$REPO_ROOT" && git log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$unpushed" -gt 0 ]]; then
    log WARN "SR-VIOLATION: $unpushed unpushed commits in racecontrol"
    violations=$((violations + 1))
    if [[ "$NO_FIX" != "true" ]]; then
      log INFO "  Auto-fix: pushing..."
      (cd "$REPO_ROOT" && git push 2>/dev/null) || log WARN "  Push failed"
      BUGS_FIXED=$((BUGS_FIXED + 1))
    fi
  fi

  local unpushed_cl
  unpushed_cl=$(cd "$COMMS_LINK_DIR" && git log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$unpushed_cl" -gt 0 ]]; then
    log WARN "SR-VIOLATION: $unpushed_cl unpushed commits in comms-link"
    violations=$((violations + 1))
    if [[ "$NO_FIX" != "true" ]]; then
      log INFO "  Auto-fix: pushing..."
      (cd "$COMMS_LINK_DIR" && git push 2>/dev/null) || log WARN "  Push failed"
      BUGS_FIXED=$((BUGS_FIXED + 1))
    fi
  fi

  # SR-02: Relay watchdog running?
  local relay_ok
  relay_ok=$(curl -s --max-time 3 "$RELAY_URL/relay/health" 2>/dev/null | jq -r '.connected // false' 2>/dev/null || echo "false")
  if [[ "$relay_ok" != "true" ]]; then
    log WARN "SR-VIOLATION: comms-link relay not healthy"
    violations=$((violations + 1))
  fi

  if [[ "$violations" -eq 0 ]]; then
    record_step "standing_rules" "PASS" "all standing rules compliant"
  else
    record_step "standing_rules" "WARN" "violations=$violations"
  fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# STEP 6: GENERATE REPORT & NOTIFY
# ═══════════════════════════════════════════════════════════════════════════════
generate_report_and_notify() {
  log INFO "=== STEP 6: Report & Notify ==="

  # Calculate totals
  BUGS_UNFIXED=$((BUGS_FOUND - BUGS_FIXED))
  if [[ "$BUGS_UNFIXED" -lt 0 ]]; then BUGS_UNFIXED=0; fi

  # Determine overall verdict
  local verdict="PASS"
  if [[ "$BUGS_UNFIXED" -gt 0 ]]; then verdict="FAIL"; fi
  if [[ "$BUGS_FOUND" -gt 0 ]] && [[ "$BUGS_UNFIXED" -eq 0 ]]; then verdict="FIXED"; fi

  # Generate summary JSON
  local summary_json="$RESULT_DIR/summary.json"
  cat > "$summary_json" <<ENDJSON
{
  "timestamp": "$TIMESTAMP",
  "mode": "$MODE",
  "verdict": "$verdict",
  "bugs_found": $BUGS_FOUND,
  "bugs_fixed": $BUGS_FIXED,
  "bugs_unfixed": $BUGS_UNFIXED,
  "cascade_updates": $CASCADE_UPDATES,
  "steps": {
    "audit": "${STEP_RESULTS[audit]:-SKIP}",
    "quality_gate": "${STEP_RESULTS[quality_gate]:-SKIP}",
    "e2e_health": "${STEP_RESULTS[e2e_health]:-SKIP}",
    "cascade": "${STEP_RESULTS[cascade]:-SKIP}",
    "standing_rules": "${STEP_RESULTS[standing_rules]:-SKIP}"
  }
}
ENDJSON

  log INFO "─── AUTONOMOUS BUG DETECTION REPORT ───"
  log INFO "Timestamp: $TIMESTAMP"
  log INFO "Mode: $MODE"
  log INFO "Verdict: $verdict"
  log INFO "Bugs found: $BUGS_FOUND | Fixed: $BUGS_FIXED | Unfixed: $BUGS_UNFIXED"
  log INFO "Cascade updates: $CASCADE_UPDATES"
  log INFO ""
  log INFO "Steps:"
  for step in audit quality_gate e2e_health cascade standing_rules; do
    log INFO "  $step: ${STEP_RESULTS[$step]:-SKIP}"
  done
  log INFO "────────────────────────────────────────"

  # Notify Bono via WS + INBOX.md
  if [[ "$NO_NOTIFY" != "true" ]] && [[ "$DRY_RUN" != "true" ]]; then
    local notify_msg="AUTO-DETECT $verdict — $TIMESTAMP

Bugs: found=$BUGS_FOUND, fixed=$BUGS_FIXED, unfixed=$BUGS_UNFIXED
Cascade: $CASCADE_UPDATES updates applied
Steps: audit=${STEP_RESULTS[audit]:-SKIP}, gate=${STEP_RESULTS[quality_gate]:-SKIP}, e2e=${STEP_RESULTS[e2e_health]:-SKIP}, cascade=${STEP_RESULTS[cascade]:-SKIP}, rules=${STEP_RESULTS[standing_rules]:-SKIP}

Standing rules: If James is down, run: bash /root/racecontrol/scripts/bono-auto-detect.sh
Full report: $RESULT_DIR/auto-detect.log"

    # WS notification
    cd "$COMMS_LINK_DIR"
    COMMS_PSK="$COMMS_PSK" COMMS_URL="$COMMS_URL" node send-message.js "$notify_msg" 2>/dev/null || log WARN "WS notify failed"
    cd "$REPO_ROOT"

    log INFO "Bono notified via WS"

    # WhatsApp escalation via healing engine (HEAL-04: silence conditions)
    # HEAL-04: QUIET severity = no WhatsApp; venue closed + <7AM IST = defer; 6h cooldown per pod+issue
    if [[ -f "$RESULT_DIR/findings.json" ]] && [[ $(type -t escalate_human) == "function" ]]; then
      # HEAL-04: WhatsApp only when (a) all tiers exhausted or (b) 3+ pods affected
      local affected_pods
      affected_pods=$(jq -r '[.[].pod_ip] | unique | length' "$RESULT_DIR/findings.json" 2>/dev/null || echo "0")

      if [[ "$BUGS_UNFIXED" -gt 0 ]] || [[ "$affected_pods" -ge 3 ]]; then
        jq -r '.[] | "\(.pod_ip) \(.issue_type) \(.severity)"' "$RESULT_DIR/findings.json" 2>/dev/null | \
        while read -r pod_ip issue_type severity; do
          if [[ -n "$pod_ip" ]] && [[ -n "$issue_type" ]]; then
            escalate_human "$pod_ip" "$issue_type" "${severity:-P1}"
          fi
        done
      fi
    fi
  fi

  # COORD-04: Write completion marker for Bono skip-if-recent logic
  if [[ $(type -t write_completion_marker) == "function" ]]; then
    write_completion_marker "$verdict" "$BUGS_FOUND" "$BUGS_FIXED"
    log INFO "Completion marker written: verdict=$verdict"
  fi

  # Return appropriate exit code
  if [[ "$BUGS_UNFIXED" -gt 0 ]]; then
    return 1
  fi
  return 0
}

# ═══════════════════════════════════════════════════════════════════════════════
# MAIN — Execute Pipeline
# ═══════════════════════════════════════════════════════════════════════════════
main() {
  log INFO "PID lock acquired (PID $$)"
  log INFO "Venue state: $DETECTED_VENUE_STATE | Effective mode: $MODE"
  log INFO "=== AUTONOMOUS BUG DETECTION PIPELINE ==="
  log INFO "Mode: $MODE | $(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')"

  run_audit
  run_quality_gate
  run_e2e_health
  run_cascade_check
  run_standing_rules_check
  generate_report_and_notify
}

main "$@"

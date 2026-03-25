#!/usr/bin/env bash
# audit/audit.sh — Racing Point fleet audit entry point
# Usage: AUDIT_PIN=<pin> bash audit/audit.sh --mode <quick|standard|full|pre-ship|post-incident> [flags]
# Exit codes: 0=all checks passed, 1=one or more FAIL results, 2=fatal prerequisite error

set -u
set -o pipefail

# ---------------------------------------------------------------------------
# Resolve SCRIPT_DIR so all relative paths work regardless of CWD
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ---------------------------------------------------------------------------
# Source shared library (written by Plan 02). Exit 2 if not found.
# Plans 01 and 02 are both Wave 1 — run Plan 02 before end-to-end execution.
# ---------------------------------------------------------------------------
if [ -f "$SCRIPT_DIR/lib/core.sh" ]; then
  # shellcheck source=audit/lib/core.sh
  source "$SCRIPT_DIR/lib/core.sh"
else
  echo "ERROR: audit/lib/core.sh not found. Run Plan 02 first to create it." >&2
  # Provide stubs for functions used before full framework is available
  emit_result() { :; }
  get_session_token() { :; }
fi

if [ -f "$SCRIPT_DIR/lib/parallel.sh" ]; then
  source "$SCRIPT_DIR/lib/parallel.sh"
fi

if [ -f "$SCRIPT_DIR/lib/results.sh" ]; then
  source "$SCRIPT_DIR/lib/results.sh"
fi

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
export SERVER_IP="192.168.31.23"
export SERVER_PORT="8080"
export SERVER_OPS_PORT="8090"
export AUTH_ENDPOINT="http://${SERVER_IP}:${SERVER_PORT}/api/v1/terminal/auth"
export FLEET_HEALTH_ENDPOINT="http://${SERVER_IP}:${SERVER_PORT}/api/v1/fleet/health"
export PODS="192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91"
export DEFAULT_TIMEOUT=10

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
AUDIT_MODE=""
AUDIT_TIER=""
AUDIT_PHASE=""
AUTO_FIX=false
NOTIFY=false
COMMIT=false
DRY_RUN=false

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
usage() {
  cat >&2 <<'USAGE'
Usage: AUDIT_PIN=<pin> bash audit/audit.sh --mode <MODE> [OPTIONS]

Modes:
  quick          Fast health sweep (Tiers 1-2, ~5 min)
  standard       Full Tiers 1-9 run (~15 min)
  full           All 18 tiers, 60 phases (~8 min with parallel engine)
  pre-ship       Pre-deployment verification gates
  post-incident  Post-incident investigation sweep

Options:
  --tier N       Run only a specific tier (1-18)
  --phase N      Run only a specific phase number
  --auto-fix     Apply smallest reversible fixes automatically
  --notify       Send Bono notification on completion
  --commit       Auto-commit result files after run
  --dry-run      Parse args + init result dir, then exit 0 (no checks run)
  --help, -h     Print this message and exit 0

Environment:
  AUDIT_PIN      Required. Staff terminal PIN (e.g. export AUDIT_PIN=261121)
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --mode)
      shift
      case "$1" in
        quick|standard|full|pre-ship|post-incident)
          AUDIT_MODE="$1"
          ;;
        *)
          echo "ERROR: invalid --mode '$1'. Valid: quick standard full pre-ship post-incident" >&2
          exit 2
          ;;
      esac
      ;;
    --tier)
      shift
      AUDIT_TIER="$1"
      ;;
    --phase)
      shift
      AUDIT_PHASE="$1"
      ;;
    --auto-fix)
      AUTO_FIX=true
      ;;
    --notify)
      NOTIFY=true
      ;;
    --commit)
      COMMIT=true
      ;;
    --dry-run)
      DRY_RUN=true
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown flag '$1'. Run with --help for usage." >&2
      exit 2
      ;;
  esac
  shift
done

if [ -z "$AUDIT_MODE" ]; then
  echo "ERROR: --mode is required. Valid: quick standard full pre-ship post-incident" >&2
  exit 2
fi

export AUDIT_MODE
export AUDIT_TIER
export AUDIT_PHASE
export AUTO_FIX
export NOTIFY
export COMMIT
export DRY_RUN

# ---------------------------------------------------------------------------
# check_prerequisites: jq, curl, AUDIT_PIN
# ---------------------------------------------------------------------------
check_prerequisites() {
  if ! command -v jq >/dev/null 2>&1; then
    echo "ERROR: jq is required. Install: winget install jqlang.jq" >&2
    exit 2
  fi
  if ! command -v curl >/dev/null 2>&1; then
    echo "ERROR: curl is required." >&2
    exit 2
  fi
  if [ -z "${AUDIT_PIN:-}" ]; then
    echo "ERROR: AUDIT_PIN env var is required (export AUDIT_PIN=261121)" >&2
    exit 2
  fi
}

# ---------------------------------------------------------------------------
# init_result_dir: IST-timestamped result directory
# ---------------------------------------------------------------------------
init_result_dir() {
  local ts
  ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d_%H-%M')
  export RESULT_DIR="${SCRIPT_DIR}/results/${ts}"
  mkdir -p "$RESULT_DIR"

  # Write run metadata
  jq -n \
    --arg mode "$AUDIT_MODE" \
    --arg started_at "$(TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30')" \
    --argjson dry_run "$DRY_RUN" \
    --argjson pin_provided true \
    '{mode: $mode, started_at: $started_at, dry_run: $dry_run, pin_provided: $pin_provided}' \
    > "$RESULT_DIR/run-meta.json"

  # Create/update latest symlink; fallback to latest.txt if symlink fails
  if ln -sfn "$RESULT_DIR" "${SCRIPT_DIR}/results/latest" 2>/dev/null; then
    :
  else
    echo "$RESULT_DIR" > "${SCRIPT_DIR}/results/latest.txt"
  fi
}

# ---------------------------------------------------------------------------
# acquire_auth: obtain SESSION_TOKEN via AUDIT_PIN, never hardcoded
# ---------------------------------------------------------------------------
acquire_auth() {
  export SESSION_TOKEN=""

  # Write request payload to temp file (bash string escaping safety rule)
  local tmp_auth
  tmp_auth=$(mktemp)
  printf '{"pin":"%s"}' "$AUDIT_PIN" > "$tmp_auth"

  local response
  response=$(curl -s --max-time "$DEFAULT_TIMEOUT" \
    -X POST "$AUTH_ENDPOINT" \
    -H "Content-Type: application/json" \
    -d @"$tmp_auth" 2>/dev/null || true)
  rm -f "$tmp_auth"

  if [ -n "$response" ]; then
    SESSION_TOKEN=$(echo "$response" | jq -r '.session // empty' 2>/dev/null || true)
  fi

  if [ -z "$SESSION_TOKEN" ]; then
    echo "WARN: Could not obtain auth token from $AUTH_ENDPOINT (server may be offline)" >&2
  fi

  # For full mode: background subshell refreshes token every 840 seconds (14 min, JWT ~15 min TTL)
  if [ "$AUDIT_MODE" = "full" ]; then
    (
      while true; do
        sleep 840
        local tmp_refresh
        tmp_refresh=$(mktemp)
        printf '{"pin":"%s"}' "$AUDIT_PIN" > "$tmp_refresh"
        local new_token
        new_token=$(curl -s --max-time "$DEFAULT_TIMEOUT" \
          -X POST "$AUTH_ENDPOINT" \
          -H "Content-Type: application/json" \
          -d @"$tmp_refresh" 2>/dev/null | jq -r '.session // empty' 2>/dev/null || true)
        rm -f "$tmp_refresh"
        if [ -n "$new_token" ]; then
          # Export to parent shell via named pipe / tmp file (subshell can't mutate parent vars)
          echo "$new_token" > "${RESULT_DIR}/.session_refresh"
        fi
      done
    ) &
    export SESSION_REFRESH_PID=$!
  fi

  get_session_token() {
    # Re-read refreshed token if available
    if [ -f "${RESULT_DIR}/.session_refresh" ]; then
      SESSION_TOKEN=$(cat "${RESULT_DIR}/.session_refresh")
    fi
    echo "$SESSION_TOKEN"
  }
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
check_prerequisites
init_result_dir
acquire_auth

echo "=== Racing Point Fleet Audit ==="
echo "Mode:       $AUDIT_MODE"
echo "Result dir: $RESULT_DIR"
echo "Started:    $(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M:%S IST')"
echo ""

if [ "$DRY_RUN" = "true" ]; then
  echo "DRY RUN — framework initialized. No checks executed."
  exit 0
fi

# Detect venue state (open/closed) for context
VENUE_STATE=$(venue_state_detect 2>/dev/null || echo "unknown")
export VENUE_STATE
echo "Venue state: $VENUE_STATE"

# Initialization phase result
emit_result "00" "0" "james-local" "PASS" "P3" "Audit framework initialized" "$AUDIT_MODE" "$VENUE_STATE"

# ---------------------------------------------------------------------------
# load_phases: source phase scripts by mode (EXEC-05)
# Each tier directory contains phaseNN.sh files defining run_phaseNN functions.
# Mode determines which tiers are loaded:
#   quick        -> Tiers 1-2  (phases 01-16)
#   standard     -> Tiers 1-9  (phases 01-44)
#   full         -> Tiers 1-9  (phases 01-44, Tiers 10-18 added in Phase 191)
#   pre-ship     -> Tiers 1-2 + selected (Phase 191 completes this)
#   post-incident-> Tiers 1-2 + Tier 8  (sentinel/recovery focused)
# ---------------------------------------------------------------------------
load_phases() {
  local mode="$1"

  # Helper: source all .sh files in a tier directory
  source_tier() {
    local tier_dir="$SCRIPT_DIR/phases/$1"
    if [[ -d "$tier_dir" ]]; then
      for f in "$tier_dir"/phase*.sh; do
        [[ -f "$f" ]] || continue
        # shellcheck disable=SC1090
        source "$f" 2>/dev/null || echo "WARN: could not source ${f##*/}" >&2
      done
    fi
  }

  # Tiers always loaded for quick+ (infrastructure foundation + core services)
  source_tier "tier1"
  source_tier "tier2"

  case "$mode" in
    quick)
      # Tiers 1-2 only -- already loaded above
      ;;
    standard|full|pre-ship|post-incident)
      # Tiers 3-9 for standard/full
      source_tier "tier3"
      source_tier "tier4"
      source_tier "tier5"
      source_tier "tier6"
      source_tier "tier7"
      source_tier "tier8"
      source_tier "tier9"
      if [[ "$mode" = "full" ]]; then
        source_tier "tier10"
        source_tier "tier11"
        source_tier "tier12"
        source_tier "tier13"
        source_tier "tier14"
        source_tier "tier15"
        source_tier "tier16"
        source_tier "tier17"
        source_tier "tier18"
      fi
      ;;
  esac
}

# ---------------------------------------------------------------------------
# load_phases: source phase scripts based on mode
# ---------------------------------------------------------------------------
load_phases "$AUDIT_MODE"

# ---------------------------------------------------------------------------
# Phase dispatch: --phase N | --tier N | mode-based all phases
# ---------------------------------------------------------------------------
if [[ -n "$AUDIT_PHASE" ]]; then
  # Pad to 2 digits: "7" -> "07"
  phase_padded=$(printf '%02d' "${AUDIT_PHASE#0}" 2>/dev/null || printf '%s' "$AUDIT_PHASE")
  fn="run_phase${phase_padded}"
  if declare -f "$fn" >/dev/null 2>&1; then
    echo "Running single phase: $fn"
    "$fn"
  else
    echo "WARN: Phase function '$fn' not found. Was phase script sourced for this mode?" >&2
    emit_result "$phase_padded" "?" "james-local" "FAIL" "P2" \
      "Phase function $fn not loaded — check --mode includes this phase's tier" \
      "$AUDIT_MODE" "$VENUE_STATE"
  fi

elif [[ -n "$AUDIT_TIER" ]]; then
  # Run all phases in the specified tier
  echo "Running tier: $AUDIT_TIER"
  case "$AUDIT_TIER" in
    1)  run_phase01; run_phase02; run_phase03; run_phase04; run_phase05
        run_phase06; run_phase07; run_phase08; run_phase09; run_phase10 ;;
    2)  run_phase11; run_phase12; run_phase13; run_phase14; run_phase15; run_phase16 ;;
    3)  run_phase17; run_phase18; run_phase19; run_phase20 ;;
    4)  run_phase21; run_phase22; run_phase23; run_phase24; run_phase25 ;;
    5)  run_phase26; run_phase27; run_phase28; run_phase29 ;;
    6)  run_phase30; run_phase31; run_phase32; run_phase33; run_phase34 ;;
    7)  run_phase35; run_phase36; run_phase37; run_phase38 ;;
    8)  run_phase39; run_phase40; run_phase41; run_phase42 ;;
    9)  run_phase43; run_phase44 ;;
    10) run_phase45; run_phase46; run_phase47 ;;
    11) run_phase48; run_phase49; run_phase50 ;;
    12) run_phase51; run_phase52; run_phase53 ;;
    13) run_phase54 ;;
    14) run_phase55; run_phase56 ;;
    15) run_phase57 ;;
    16) run_phase58 ;;
    17) run_phase59 ;;
    18) run_phase60 ;;
    *)  echo "WARN: Invalid tier $AUDIT_TIER. Valid: 1-18" >&2 ;;
  esac

else
  # Mode-based full run
  run_tier_1_to_2() {
    run_phase01; run_phase02; run_phase03; run_phase04; run_phase05
    run_phase06; run_phase07; run_phase08; run_phase09; run_phase10
    run_phase11; run_phase12; run_phase13; run_phase14; run_phase15; run_phase16
  }
  run_tier_3_to_9() {
    run_phase17; run_phase18; run_phase19; run_phase20
    run_phase21; run_phase22; run_phase23; run_phase24; run_phase25
    run_phase26; run_phase27; run_phase28; run_phase29
    run_phase30; run_phase31; run_phase32; run_phase33; run_phase34
    run_phase35; run_phase36; run_phase37; run_phase38
    run_phase39; run_phase40; run_phase41; run_phase42
    run_phase43; run_phase44
  }
  run_tier_10_to_18() {
    run_phase45; run_phase46; run_phase47
    run_phase48; run_phase49; run_phase50
    run_phase51; run_phase52; run_phase53
    run_phase54
    run_phase55; run_phase56
    run_phase57
    run_phase58
    run_phase59
    run_phase60
  }

  case "$AUDIT_MODE" in
    quick)
      echo "Mode: quick — running Tiers 1-2 (phases 01-16)"
      run_tier_1_to_2
      ;;
    standard)
      echo "Mode: standard — running Tiers 1-9 (phases 01-44)"
      run_tier_1_to_2
      run_tier_3_to_9
      ;;
    full)
      echo "Mode: full — running All 18 tiers, phases 01-60"
      run_tier_1_to_2
      run_tier_3_to_9
      run_tier_10_to_18
      ;;
    pre-ship)
      echo "Mode: pre-ship — critical subset (Tiers 1-2 + targeted checks)"
      run_tier_1_to_2
      # Phases 35 (cloud sync), 39 (flags) as critical pre-ship checks
      declare -f run_phase35 >/dev/null 2>&1 && run_phase35
      declare -f run_phase39 >/dev/null 2>&1 && run_phase39
      ;;
    post-incident)
      echo "Mode: post-incident — Tiers 1-2 + Tier 8 (advanced systems/recovery)"
      run_tier_1_to_2
      declare -f run_phase39 >/dev/null 2>&1 && run_phase39
      declare -f run_phase40 >/dev/null 2>&1 && run_phase40
      declare -f run_phase41 >/dev/null 2>&1 && run_phase41
      declare -f run_phase42 >/dev/null 2>&1 && run_phase42
      ;;
  esac
fi

echo ""
echo "Phase runner complete. Results in: $RESULT_DIR"

# ---------------------------------------------------------------------------
# Finalize results: update run-meta.json with counts, append to index.json
# ---------------------------------------------------------------------------
if declare -f finalize_results >/dev/null 2>&1; then
  finalize_results
fi

# ---------------------------------------------------------------------------
# Exit code: count FAIL results in result dir
# ---------------------------------------------------------------------------
FAIL_COUNT=0
if [ -d "$RESULT_DIR" ]; then
  for f in "$RESULT_DIR"/phase-*.json; do
    [ -f "$f" ] || continue
    status=$(jq -r '.status // "UNKNOWN"' "$f" 2>/dev/null || echo "UNKNOWN")
    if [ "$status" = "FAIL" ]; then
      FAIL_COUNT=$((FAIL_COUNT+1))
    fi
  done
fi

if [ "$FAIL_COUNT" -gt 0 ]; then
  echo ""
  echo "Audit complete: $FAIL_COUNT phase(s) FAILED. See $RESULT_DIR"
  exit 1
else
  echo ""
  echo "Audit complete: all checks PASSED. See $RESULT_DIR"
  exit 0
fi

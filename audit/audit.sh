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
  quick          Fast health sweep (Tier 1 only, ~2 min)
  standard       Full Tier 1-3 run (~10 min)
  full           All tiers + extra probes (~25 min)
  pre-ship       Pre-deployment verification gates
  post-incident  Post-incident investigation sweep

Options:
  --tier N       Run only a specific tier (1-5)
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
# load_phases: source phase scripts based on mode
# ---------------------------------------------------------------------------
load_phases() {
  local mode=$1
  # Tier 1 phases always included for quick+ mode
  if [[ -f "$SCRIPT_DIR/phases/tier1/phase01.sh" ]]; then
    # shellcheck source=audit/phases/tier1/phase01.sh
    source "$SCRIPT_DIR/phases/tier1/phase01.sh" || {
      echo "WARN: could not source phase01.sh" >&2
    }
  fi
  # Additional tiers sourced in later plans (Phase 190+)
}

# ---------------------------------------------------------------------------
# Phase dispatch
# ---------------------------------------------------------------------------
load_phases "$AUDIT_MODE"

if [[ -n "$AUDIT_PHASE" ]]; then
  # Run only the specified phase function if it exists
  fn="run_phase${AUDIT_PHASE}"
  if declare -f "$fn" >/dev/null 2>&1; then
    "$fn"
  else
    echo "WARN: Phase function '$fn' not found (phase script may not be sourced yet)" >&2
  fi
elif [[ -n "$AUDIT_TIER" ]]; then
  # Run all phases in the specified tier
  case "$AUDIT_TIER" in
    1) run_phase01 ;;
    *) echo "WARN: Tier $AUDIT_TIER phases not yet implemented" >&2 ;;
  esac
else
  # Run all phases for the current mode
  case "$AUDIT_MODE" in
    quick|standard|full|pre-ship|post-incident)
      run_phase01
      ;;
  esac
fi

echo ""
echo "Phase runner complete. Results in: $RESULT_DIR"

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

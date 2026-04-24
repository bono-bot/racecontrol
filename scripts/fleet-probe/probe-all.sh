#!/bin/bash
# scripts/fleet-probe/probe-all.sh -- Phase 448 Plan 07 -- FULL wiring.
# Replaces Plan 02 skeleton with real probe invocations.
# Usage:
#   bash scripts/fleet-probe/probe-all.sh --dry-run
#   bash scripts/fleet-probe/probe-all.sh --canary
#   bash scripts/fleet-probe/probe-all.sh
#   bash scripts/fleet-probe/probe-all.sh --help
#
# Flags:
#   --dry-run  enumerate 15 targets, no network calls (Plan 02 contract preserved)
#   --canary   run server_23 + pod_8 only (PACT-012 canary subset)
#   (no flag)  run all 15 probes
#
# Exit: 0 always (probe_failed is a row in _meta.json, not an orchestrator error)
#
# Env propagated to children:
#   MANIFEST_TS         (generated here; exported)
#   _PROBE_PYTHON       (propagated from PROBE_PYTHON env or probe-common.sh default)
#   FLEET_PROBE_VALIDATE (optional -- if =1, subprobes validate each manifest via ajv)
#   SENTRY_KEY, COMMS_PSK, STAFF_JWT (passed through from invoking shell)
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

# 15 targets -- role -- (ordering locked in CONTEXT.md + RESEARCH section 3)
# Count: server=1 + pod=8 + pos=1 + james=1 + vps=1 + cloud_admin=1 + cloud_racecontrol=1 + relay=1 = 15
# Format: "target_id role" (space-separated)
TARGETS=(
  "server_23 server"
  "pod_1 pod"
  "pod_2 pod"
  "pod_3 pod"
  "pod_4 pod"
  "pod_5 pod"
  "pod_6 pod"
  "pod_7 pod"
  "pod_8 pod"
  "pos_130 pos"
  "james_27 james"
  "bono_vps vps"
  "cloud_admin cloud_admin"
  "cloud_racecontrol cloud_racecontrol"
  "relay_james relay"
)

MODE="full"
for arg in "$@"; do
  case "$arg" in
    --dry-run) MODE="dry-run" ;;
    --canary)  MODE="canary"  ;;
    --help|-h)
      printf "Usage: probe-all.sh [--dry-run|--canary]\n"
      printf "  --dry-run  enumerate 15 targets, no network calls\n"
      printf "  --canary   run server_23 + pod_8 only\n"
      printf "  (no flag)  run all 15 probes\n"
      exit 0
      ;;
  esac
done

# --dry-run: enumerate targets in locked order, no network calls (Plan 02 contract preserved)
if [ "$MODE" = "dry-run" ]; then
  for entry in "${TARGETS[@]}"; do
    id="${entry%% *}"
    role="${entry##* }"
    printf "target=%-24s role=%s\n" "$id" "$role"
  done
  exit 0
fi

# --- Full / canary wiring ---
export MANIFEST_TS="${MANIFEST_TS:-$(date -u +%Y-%m-%dT%H%M%SZ)}"
MANIFEST_DIR="$REPO_ROOT/state/fleet-manifest/$MANIFEST_TS"
mkdir -p "$MANIFEST_DIR"
START_EPOCH=$(date +%s)

# Propagate Python interpreter override so subprobes inherit Windows-safe path.
# probe-common.sh sets _PROBE_PYTHON from PROBE_PYTHON already; re-export here so
# child bash processes that re-source probe-common.sh pick up the same value.
export PROBE_PYTHON="${PROBE_PYTHON:-}"

printf "probe-all: MANIFEST_TS=%s mode=%s dir=%s\n" "$MANIFEST_TS" "$MODE" "$MANIFEST_DIR" >&2

# run_probe SCRIPT [ARGS...]
# Invokes a probe script; never propagates its exit code (orchestrator exit is always 0).
run_probe() {
  local script="$1"; shift
  printf "  -> %s %s\n" "$script" "$*" >&2
  bash "$script" "$@" || true
}

if [ "$MODE" = "canary" ]; then
  # Canary: server_23 + pod_8 only (sequential -- both use SSH, avoid concurrent auth)
  run_probe "$SCRIPT_DIR/probe-server.sh"
  run_probe "$SCRIPT_DIR/probe-pod.sh" 8
else
  # Full run: sequential cluster first (server, pos, james, vps, cloud_admin, cloud_rc, relay)
  # then pods 1-8 in parallel via & + wait.
  run_probe "$SCRIPT_DIR/probe-server.sh"
  run_probe "$SCRIPT_DIR/probe-pos.sh"
  run_probe "$SCRIPT_DIR/probe-james.sh"
  run_probe "$SCRIPT_DIR/probe-vps.sh"
  run_probe "$SCRIPT_DIR/probe-cloud-admin.sh"
  run_probe "$SCRIPT_DIR/probe-cloud-rc.sh"
  run_probe "$SCRIPT_DIR/probe-relay.sh"

  # Pod fanout -- parallel via & + wait
  PIDS=()
  for N in 1 2 3 4 5 6 7 8; do
    bash "$SCRIPT_DIR/probe-pod.sh" "$N" &
    PIDS+=($!)
  done
  # Wait for all pod probes (ignore individual exit codes -- probe_failed is in manifest)
  for pid in "${PIDS[@]}"; do
    wait "$pid" || true
  done
fi

# Build _meta.json summary index from whatever manifests were written.
# Use _PROBE_PYTHON so Windows tests can override the python interpreter.
PYTHON_BIN="${PROBE_PYTHON:-python3}"
"$PYTHON_BIN" "$SCRIPT_DIR/build-meta-index.py" "$MANIFEST_DIR" \
  --orchestrator-start-epoch "$START_EPOCH" || true

ELAPSED=$(( $(date +%s) - START_EPOCH ))
printf "probe-all: done in %ds  dir=%s\n" "$ELAPSED" "$MANIFEST_DIR" >&2
exit 0

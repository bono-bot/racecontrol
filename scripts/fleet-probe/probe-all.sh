#!/bin/bash
# scripts/fleet-probe/probe-all.sh -- Phase 448 orchestrator.
# Plan 02: skeleton with --dry-run target enumeration only. Plan 07 wires real invocations.
# Usage:
#   bash scripts/fleet-probe/probe-all.sh --dry-run   # prints 15-target list, no network calls
#   bash scripts/fleet-probe/probe-all.sh             # (Plan 07) runs all 15 probes, exit 3 until then
#   bash scripts/fleet-probe/probe-all.sh --canary    # (Plan 07) runs server_23 + pod_8 only
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
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
    --canary)  MODE="canary" ;;
    --help|-h)
      printf "Usage: probe-all.sh [--dry-run|--canary]\n"
      printf "  --dry-run  enumerate 15 targets, no network calls (Plan 02 skeleton)\n"
      printf "  --canary   run server_23 + pod_8 only (wired in Plan 07)\n"
      printf "  (no flag)  run all 15 probes (wired in Plan 07)\n"
      exit 0
      ;;
  esac
done

if [ "$MODE" = "dry-run" ]; then
  for entry in "${TARGETS[@]}"; do
    id="${entry%% *}"
    role="${entry##* }"
    printf "target=%-24s role=%s\n" "$id" "$role"
  done
  exit 0
fi

# Plan 07 wires the real probe invocations.
# Until then, full and canary modes are not implemented.
printf "probe-all.sh: full/canary modes are wired in Plan 448-07. Use --dry-run for now.\n" >&2
exit 3

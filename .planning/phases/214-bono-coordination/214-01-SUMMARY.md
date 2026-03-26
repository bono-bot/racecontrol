---
phase: 214-bono-coordination
plan: "01"
subsystem: coordination
tags: [bash, coordination, mutex, completion-marker, auto-detect]
dependency_graph:
  requires: [213-self-healing-escalation]
  provides: [COORD-01, COORD-04]
  affects: [scripts/auto-detect.sh, scripts/bono-auto-detect.sh]
tech_stack:
  added: []
  patterns: [bash-sourced-module, jq-json-write, trap-exit-cleanup]
key_files:
  created:
    - scripts/coordination/coord-state.sh
  modified:
    - scripts/auto-detect.sh
decisions:
  - "Guard-wrap all hook calls with [[ $(type -t fn) == function ]] so auto-detect.sh degrades gracefully if coord-state.sh is absent"
  - "Replace single PID-file trap with combined trap after write_active_lock so both cleanup paths are covered atomically"
  - "Use set -uo pipefail (no set -e) in coord-state.sh — sourced files with set -e cause silent exits on non-zero returns from is_james_run_recent"
metrics:
  duration_minutes: 8
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 1
  completed_date: "2026-03-26"
---

# Phase 214 Plan 01: James Coordination Mutex and Completion Marker Summary

James-side coordination primitives for COORD-01 (AUTO_DETECT_ACTIVE mutex) and COORD-04 (completion marker). Enables Bono to detect James auto-detect state via relay-accessible lock file and defer its own run when James completed within the last 10 minutes.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Create coord-state.sh with lock and completion marker functions | `0f2bfc53` | `scripts/coordination/coord-state.sh` (created) |
| 2 | Integrate coordination hooks into auto-detect.sh | `c4a5598e` | `scripts/auto-detect.sh` (modified) |

## What Was Built

**`scripts/coordination/coord-state.sh`** — sourced module with 5 exported functions:

- `write_active_lock()` — writes `audit/results/auto-detect-active.lock` JSON with `{agent, pid, started_ts, relay_url}` using `jq --arg` (safe variable injection)
- `clear_active_lock()` — `rm -f` on the lock file, called from EXIT trap
- `write_completion_marker(verdict, bugs_found, bugs_fixed)` — writes `audit/results/last-run-summary.json` at a fixed path Bono can always find
- `is_james_run_recent()` — returns 0 if `completed_ts` is within `COORD_STALE_SECS` (600s) of now; Bono uses this to skip its scheduled run
- `read_active_lock()` — outputs lock JSON or `{}` for relay queries from Bono

**`scripts/auto-detect.sh`** changes (3 insertion points, zero logic changes):

1. Sources `coord-state.sh` after `escalation-engine.sh` source block
2. Calls `write_active_lock()` after `_acquire_run_lock`; replaces single-file EXIT trap with combined trap covering both PID file and coord lock
3. Calls `write_completion_marker()` in `generate_report_and_notify()` before exit, logging the verdict

## Verification Results

All plan verification steps passed:

```
bash -n scripts/auto-detect.sh            -> syntax ok
bash -n scripts/coordination/coord-state.sh -> syntax ok
declare -F: 5 functions exported (declare -fx)
Lock round-trip: write -> cat (valid JSON) -> clear -> file absent
Completion marker: write -> is_james_run_recent returns 0 (true within 600s window)
```

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- `scripts/coordination/coord-state.sh` — FOUND
- `scripts/auto-detect.sh` — FOUND (modified)
- Commit `0f2bfc53` — FOUND
- Commit `c4a5598e` — FOUND

---
phase: 192-intelligence-layer
plan: "01"
subsystem: audit-framework
tags: [audit, results-storage, index, delta-tracking, shell]
dependency_graph:
  requires:
    - 189-02 (audit/lib/core.sh — ist_now, emit_result primitives)
    - 191-xx (audit/lib/parallel.sh — sourced before results.sh)
  provides:
    - audit/lib/results.sh (finalize_results, update_index, find_previous_run)
    - audit/results/index.json (append-only run history)
  affects:
    - 192-02 (delta comparison uses find_previous_run)
    - 192-04 (report generation reads index.json for run history)
tech_stack:
  added: []
  patterns:
    - "mktemp+mv atomic write for index.json (prevents ctrl-C corruption)"
    - "declare -f guard for backward-compatible optional module loading"
    - "jq .[-2] for previous run lookup without list iteration"
key_files:
  created:
    - audit/lib/results.sh
  modified:
    - audit/audit.sh
decisions:
  - "Atomic index write: mktemp to temp file then mv — prevents index corruption on ctrl-C interrupt"
  - "declare -f finalize_results guard in audit.sh — missing results.sh is gracefully skipped, not fatal"
  - "find_previous_run uses jq .[-2] directly — O(1) vs O(n) iteration, correct for append-only index"
  - "finalize_results reads from RESULT_DIR/phase-*.json (same files exit-code counting reads) — single source of truth"
metrics:
  duration_minutes: 5
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 1
  completed_date: "2026-03-25"
---

# Phase 192 Plan 01: Results Storage and Index Management Summary

Structured results storage is the foundation for delta tracking (Plan 02) and report generation (Plan 04). This plan created `audit/lib/results.sh` with three exported functions and wired them into `audit.sh` so every completed audit run updates `run-meta.json`, appends to `results/index.json`, and makes the previous run directory findable for delta comparison.

## What Was Built

**`audit/lib/results.sh`** — 3 exported functions following core.sh style:

1. `finalize_results` — Called after all phases complete. Counts PASS/WARN/FAIL/QUIET by iterating `$RESULT_DIR/phase-*.json` via jq. Merges counts and `completed_at` into existing `run-meta.json`. Calls `update_index`.

2. `update_index` — Reads `results/index.json` (creates `[]` if missing). Appends one entry with timestamp, mode, result_dir, venue_state, and counts. Writes atomically via mktemp+mv to prevent corruption on ctrl-C.

3. `find_previous_run` — Reads `results/index.json` and returns the `result_dir` of the second-to-last entry using `jq -r '.[-2].result_dir // empty'`. Returns empty string if fewer than 2 entries exist.

**`audit/audit.sh`** — Two changes:
- Source block for `lib/results.sh` added after `lib/parallel.sh` source (guarded by file existence check)
- `finalize_results` call inserted after "Phase runner complete" echo, before `FAIL_COUNT=0` exit-code counting block (guarded by `declare -f finalize_results`)

## Verification Results

All plan verification steps passed:
- `bash -n audit/audit.sh` — no syntax errors
- `bash -n audit/lib/results.sh` — no syntax errors
- `grep -c 'export -f' audit/lib/results.sh` — returns 3
- `grep 'finalize_results' audit/audit.sh` — shows the guarded call at line 447
- `AUDIT_PIN=261121 bash audit/audit.sh --mode quick --dry-run` — exits 0

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

- [x] `audit/lib/results.sh` exists at correct path
- [x] `audit/audit.sh` modified with source + finalize_results call
- [x] Task 1 commit: `60d29fa6`
- [x] Task 2 commit: `5194e83d`

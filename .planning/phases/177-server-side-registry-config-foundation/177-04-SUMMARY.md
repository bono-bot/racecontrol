---
phase: 177-server-side-registry-config-foundation
plan: "04"
subsystem: feature-flags
tags: [feature-flags, websocket, per-pod-override, requirements-tracking]
dependency_graph:
  requires: [177-01, 177-02, 177-03]
  provides: [FF-02-complete, per-pod-override-resolution]
  affects: [broadcast_flag_sync, FlagCacheSync, REQUIREMENTS.md]
tech_stack:
  added: []
  patterns: [serde_json-safe-parse, ok-and-then-unwrap-or]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/ws/mod.rs
    - .planning/REQUIREMENTS.md
decisions:
  - "Per-pod override resolution done inside sender loop (not pre-built) to avoid cloning the map for each pod"
  - "Used .ok().and_then().unwrap_or() chain — no .unwrap() anywhere per standing rules"
  - "Pre-existing crypto test failure confirmed not caused by these changes"
metrics:
  duration: "8 minutes"
  completed: "2026-03-24"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 3
---

# Phase 177 Plan 04: Per-Pod Override Resolution Summary

Per-pod flag override delivery fixed — pods now receive their own resolved boolean values over WebSocket instead of the global enabled value.

## What Was Built

FF-02 was implemented in Phase 177 plans 01-03 (the overrides column, PUT endpoint accepting per-pod JSON), but the delivery path was never fixed. `broadcast_flag_sync()` and the `FlagCacheSync` reconnect handler both ignored the `overrides` field and sent `row.enabled` to all pods. This plan closes that gap.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Per-pod override resolution in broadcast and FlagCacheSync | 1fc92867 | state.rs, ws/mod.rs |
| 2 | Update REQUIREMENTS.md status tracking | 697d20be | REQUIREMENTS.md |

## Changes Made

### Task 1 — broadcast_flag_sync() (state.rs)

Replaced the pre-built single HashMap approach with per-pod resolution inside the sender loop:

- Reads `feature_flags` cache once, computes max `version`
- For each `(pod_id, sender)` in `agent_senders`, builds a per-pod flag map
- Parses `row.overrides` as `HashMap<String, bool>` via `serde_json::from_str`
- If parsing succeeds and the map contains the current `pod_id` key, uses the override value
- Falls back to `row.enabled` if parse fails, overrides is `{}`, or no key matches this pod
- Sends each pod its own `FlagSyncPayload`

### Task 1 — FlagCacheSync handler (ws/mod.rs)

Applied the same override resolution when a reconnecting pod requests a flag sync:

- Captures `&payload.pod_id` before the flag map closure
- Same `.ok().and_then(|ovr| ovr.get(pod_id).copied()).unwrap_or(row.enabled)` pattern
- No struct changes to `FlagSyncPayload` — still `HashMap<String, bool>` + version

### Task 2 — REQUIREMENTS.md

- Checked `[x]` for FF-01, FF-02, FF-03, CP-05 in the requirements list
- Updated all four from `Pending` to `Complete` in the traceability table

## Verification

- `cargo check` passes with no errors (pre-existing unused import warning only)
- `cargo test` passes 450/451 tests — the 1 failure (`crypto::encryption::tests::load_keys_wrong_length`) is pre-existing (confirmed via `git stash` test: 449/451 before these changes, 450/451 after)
- grep confirms `serde_json::from_str` + `ovr.get(pod_id)` in both changed files
- All four requirements confirmed `[x]` and `Complete` in REQUIREMENTS.md

## Deviations from Plan

None — plan executed exactly as written.

## Behavior After This Fix

A flag with `enabled: false` and `overrides: {"pod_8": true}`:
- Pod 8 receives `true` (override applied)
- All other pods receive `false` (global enabled value)

Canary testing use case (FF-02) is now fully functional end-to-end.

## Self-Check: PASSED

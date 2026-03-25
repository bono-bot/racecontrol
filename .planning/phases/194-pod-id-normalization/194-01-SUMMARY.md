---
phase: 194-pod-id-normalization
plan: "01"
subsystem: core-server
tags: [pod-id, normalization, billing, game-launcher, websocket, refactor]
dependency_graph:
  requires: []
  provides: [canonical-pod-id-normalization]
  affects: [billing.rs, game_launcher.rs, ws/mod.rs, api/routes.rs]
tech_stack:
  added: [rc-common::pod_id module]
  patterns: [normalize-at-entry-point, single-source-of-truth]
key_files:
  created:
    - crates/rc-common/src/pod_id.rs
  modified:
    - crates/rc-common/src/lib.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/billing.rs
decisions:
  - "normalize_pod_id() placed in rc-common (shared library) so both racecontrol and rc-agent can use it"
  - "normalize_pod_id returns Result<String,String> so callers can handle invalid IDs explicitly or fall back gracefully"
  - "billing.rs normalizes at all 5 entry points as defense-in-depth even though upstream callers also normalize"
  - "WS registration uses canonical_id for ALL map inserts (agent_senders, agent_conn_ids, pods, game_tracker, billing resync)"
metrics:
  duration_seconds: 664
  completed_date: "2026-03-26"
  tasks_completed: 3
  files_modified: 6
---

# Phase 194 Plan 01: Pod ID Normalization Summary

**One-liner:** Single `normalize_pod_id()` in rc-common canonicalizes all pod ID formats to `pod_N` (underscore, lowercase), eliminating 6+ billing_alt_id and alt_id workarounds across game_launcher.rs, routes.rs, ws/mod.rs, and billing.rs.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Create normalize_pod_id() in rc-common with 10 unit tests | 27adb455 |
| 2 | Replace all alt-id workarounds in game_launcher.rs, routes.rs, ws/mod.rs | 6e77fd4f |
| 3 | Normalize pod_id at all billing.rs entry points | bbdd70a6 |

## What Was Built

### Task 1: normalize_pod_id() in rc-common
- Created `crates/rc-common/src/pod_id.rs` with `pub fn normalize_pod_id(raw: &str) -> Result<String, String>`
- Accepts: pod-1, pod_1, POD_1, Pod-1, POD-8, pod-99 → canonical `pod_N` form
- Rejects: empty string, garbage (no "pod" prefix), missing number after separator
- 10 unit tests covering all 10 spec'd input variants (4 valid formats, multi-digit, edge case zero, 3 error cases)
- Added `pub mod pod_id;` to `crates/rc-common/src/lib.rs`

### Task 2: Remove alt-id workarounds from game_launcher.rs, routes.rs, ws/mod.rs
- **game_launcher.rs `launch_game()`**: normalize at function entry, remove `billing_alt_id` block (5 lines) and `alt_id` block (5 lines), simplify all HashMap lookups to single `get(pod_id)` calls
- **game_launcher.rs `relaunch_game()`**: normalize at entry, remove `relaunch_alt` computation + `or_else` fallback
- **game_launcher.rs `stop_game()`**: normalize at entry, remove `stop_alt` computation + `or_else` fallback
- **game_launcher.rs `handle_game_state_update()`**: normalize `info.pod_id` at entry for Race Engineer billing check
- **routes.rs `pod_self_test()`**: replace `alt_id` + `or_else` with single `normalize_pod_id()` + `senders.get(&pod_id)`
- **ws/mod.rs agent registration**: normalize `pod_info.id` to `canonical_id` before inserting into `agent_senders`, `agent_conn_ids`, `pods`, `active_games`, and billing resync maps

### Task 3: Normalize pod_id at billing.rs entry points
- **`defer_billing_start()`**: normalize owned `pod_id` before `waiting_for_game.insert()`
- **`handle_game_status_update()`**: normalize `&str pod_id` before all `active_timers` and `waiting_for_game` lookups
- **`start_billing_session()`**: normalize owned `pod_id` before `active_timers.contains_key()` check
- **`handle_dashboard_command()` StartBilling variant**: normalize before passing to `start_billing_session`
- **`check_and_stop_multiplayer_server()`**: normalize `&str pod_id` before DB query binding

## Verification Results

1. `cargo test -p rc-common -- pod_id` — 10/10 tests pass
2. `cargo test -p racecontrol-crate --lib -- --test-threads=1` — 505/505 tests pass (0 regressions)
3. `grep -rn "billing_alt_id|replace.*pod.*-.*_|replace.*pod.*_.*-" crates/` — ZERO hits
4. `grep -rn "normalize_pod_id" crates/racecontrol/src/` — hits in game_launcher.rs (5), routes.rs (2), ws/mod.rs (2), billing.rs (6) = all target files

**Note on parallel test failure:** `config::tests::config_fallback_preserved_when_no_env_vars` fails intermittently when run in parallel with other tests due to a pre-existing test isolation issue (environment variables set by other tests leak through). It passes consistently when run alone or with `--test-threads=1`. This is NOT a regression from this plan's changes.

## Decisions Made

1. **normalize_pod_id in rc-common** — both racecontrol and rc-agent can share this function without duplication
2. **Result return type** — callers can choose to propagate errors (launch_game) or fall back gracefully (stop_game, billing entry points)
3. **Defense-in-depth for billing.rs** — billing functions normalize independently even though upstream callers do too; PODID-03 requires billing map lookups to be resilient regardless of caller
4. **WS registration uses canonical_id for ALL downstream maps** — game_tracker reconciliation, billing resync, and pods map all use `canonical_id` from the normalization step, not the original `pod_info.id`

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| crates/rc-common/src/pod_id.rs | FOUND |
| crates/rc-common/src/lib.rs | FOUND |
| .planning/phases/194-pod-id-normalization/194-01-SUMMARY.md | FOUND |
| Commit 27adb455 | FOUND |
| Commit 6e77fd4f | FOUND |
| Commit bbdd70a6 | FOUND |

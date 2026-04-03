---
phase: 317-server-inventory-fleet-intelligence
plan: "02"
subsystem: racecontrol-server
tags: [fleet-health, crash-loop, chain-failure, whatsapp-escalation, LAUNCH-03, LAUNCH-04]
requirements: [LAUNCH-03, LAUNCH-04]

dependency_graph:
  requires: [317-01]
  provides: [crash-loop-escalation-path, chain-launch-failure-tracker]
  affects: [ws/mod.rs, state.rs, fleet_health.rs]

tech_stack:
  added: []
  patterns:
    - "EscalationRequest path via whatsapp_escalation.handle_escalation (dedup by incident_id)"
    - "No-lock-across-.await: snapshot (bool, u32) before tokio::spawn"
    - "TDD: RED (ChainFailureState undefined) → GREEN (6 tests pass) per task"

key_files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/ws/mod.rs

decisions:
  - "incident_id=crash_loop_{pod_id} gives 30-min built-in dedup in WhatsAppEscalation"
  - "ChainFailureState alerted flag prevents repeated escalation within same 10-min window"
  - "Key format {pod_id}:{sim_type:?} uses Debug (stable enum variant names)"
  - "should_escalate tuple (bool, u32) extracted before closing write guard block"

metrics:
  duration_minutes: 25
  completed: "2026-04-03T12:40:37 IST"
  tasks_completed: 2
  files_modified: 2
---

# Phase 317 Plan 02: Crash Loop Alert Path Fix + Chain Failure Detection Summary

JWT-style dedup crash loop alert now routed through EscalationRequest path; chain launch failure tracker fires WhatsApp escalation on 3rd consecutive GameState::Error per pod+sim within 10 minutes.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Fix crash loop WhatsApp path + ChainFailureState in AppState | `bf419e1e` | state.rs, ws/mod.rs |
| 2 | Chain failure detection in GameStateUpdate handler | `3ce86245` | ws/mod.rs |

## What Was Built

### Task 1: Crash Loop Alert Path Fix

**Before:** `crate::whatsapp_alerter::send_admin_alert(&state.config, "crash_loop", &alert_msg).await;`
This bypassed the EscalationRequest deduplication layer entirely (Pitfall 8 violation).

**After:** `state.whatsapp_escalation.handle_escalation(EscalationPayload { incident_id: "crash_loop_{pod_id}", ... })` inside `tokio::spawn`.
- `tracing::error!` retained (LAUNCH-03 requires ERROR-level log)
- 30-min suppression via `incident_id` dedup built into `WhatsAppEscalation`
- `restart_count` from `startup_timestamps.len()` added to summary message

**ChainFailureState struct** added to `state.rs`:
- `consecutive_failures: u32` — resets on window expiry or Running state
- `window_start: Option<Instant>` — 10-minute sliding window
- `alerted: bool` — prevents re-escalation within same window
- `is_window_expired()` → true when `window_start.is_none()` or `elapsed >= 600s`
- `reset()` → zeros all three fields

**AppState field added:**
```rust
pub chain_failure_tracker: RwLock<HashMap<String, ChainFailureState>>,
```
Key format: `"{pod_id}:{sim_type:?}"` (e.g. `"pod_3:AssettoCorsa"`)

### Task 2: Chain Failure Detection

In `GameStateUpdate` handler, after `game_launcher::handle_game_state_update`:

**GameState::Error branch:**
1. Acquire write lock, get-or-default entry by `sim_key`
2. If window expired: `entry.reset()` (starts fresh)
3. If no window yet: set `window_start = Instant::now()`
4. `consecutive_failures = consecutive_failures.saturating_add(1)`
5. If `>= 3 && !alerted`: set `alerted = true`, extract `(true, count)` tuple
6. Drop write lock (block closes `};`)
7. If `should_escalate.0`: `tokio::spawn` sends `EscalationPayload { trigger: "ChainLaunchFailure", incident_id: "chain_fail_{pod_id}_{sim_type:?}" }`

**GameState::Running branch:**
- Acquire write lock, call `entry.reset()` if key exists

## Verification Results

```
cargo test -p racecontrol-crate chain_failure
test result: ok. 6 passed; 0 failed
```

Tests passing:
- `test_chain_failure_state_window_expired_when_no_start`
- `test_chain_failure_state_window_not_expired_recently`
- `test_chain_failure_state_reset_clears_all`
- `test_chain_failure_state_three_failures_triggers_alert`
- `test_chain_failure_state_running_resets`
- `test_chain_failure_state_no_double_alert`

Criteria checks:
- `send_admin_alert` for crash_loop: **zero hits**
- `handle_escalation` + `CrashLoop` trigger: **present** (lines 1185, 1189)
- `chain_failure_tracker` in state.rs: **struct field** (line 311) + **new() init** (line 402)
- `ChainLaunchFailure` trigger: **present** (line 848)
- Write guard drops before `tokio::spawn`: **confirmed** (`};` before `if should_escalate.0`)
- No new `.unwrap()` in added code: **confirmed**

## Deviations from Plan

**[Rule 1 - Bug] Fixed `fleet.get(&pod_id)` Borrow type mismatch**
- **Found during:** Task 1 GREEN compilation
- **Issue:** `fleet.get(&pod_id)` where `pod_id: String` passed `&&String` to HashMap get which expects `Borrow<str>` — E0277 compile error
- **Fix:** Changed to `fleet.get(pod_id.as_str())`
- **Files modified:** crates/racecontrol/src/ws/mod.rs
- **Commit:** `bf419e1e` (included in same commit)

## Pre-Existing Test Failures (Out of Scope)

8 integration tests were failing before and after our changes (confirmed by stash test):
- `test_lap_not_suspect_*` (5 tests) — RowNotFound on existing DB fixture
- `test_notification_*` (3 tests) — pre-existing failures

These were NOT introduced by this plan and are logged to deferred-items as out-of-scope.

## Known Stubs

None. All implementation paths are wired to real `whatsapp_escalation.handle_escalation`.

## Self-Check: PASSED

Files exist:
- `crates/racecontrol/src/state.rs` — FOUND (contains ChainFailureState, chain_failure_tracker)
- `crates/racecontrol/src/ws/mod.rs` — FOUND (contains chain failure detection + escalation path)

Commits exist:
- `bf419e1e` — FOUND
- `3ce86245` — FOUND

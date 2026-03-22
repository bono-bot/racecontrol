---
phase: 161-pod-monitor-merge
plan: 02
subsystem: pod_monitor
tags: [rust, pod-recovery, single-authority, watchdog, refactor]
dependency_graph:
  requires: [161-01]
  provides: [PMON-02]
  affects: [pod_monitor.rs, pod_healer.rs]
tech_stack:
  added: []
  patterns: [single-recovery-authority, detection-only-monitor, graduated-recovery]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/pod_monitor.rs
    - .planning/ROADMAP.md
decisions:
  - "WatchdogState::Restarting skip guard kept in pod_monitor — pod_healer sets this state and the guard prevents double-triggering"
  - "backoff_label helper kept — still used by test-only next_action format tests"
  - "determine_failure_reason and failure_type_from_reason kept as pub functions — used by tests and may be called by pod_healer in future"
  - "record_attempt in tests is test setup only — production code has 0 record_attempt calls in pod_monitor"
metrics:
  duration_minutes: 15
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_modified: 2
---

# Phase 161 Plan 02: Strip Restart/WoL from pod_monitor Summary

**One-liner:** pod_monitor refactored to pure heartbeat detector — all WoL/exec/verify logic removed, pod_healer is sole recovery authority (PMON-02).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Strip restart and WoL execution from pod_monitor | 21c8f6f5 | crates/racecontrol/src/pod_monitor.rs |
| 2 | Verify single-authority and update ROADMAP | a97015a8 | .planning/ROADMAP.md |

## What Was Done

### Task 1: Strip restart/WoL from pod_monitor.rs

Removed all repair execution code from pod_monitor.rs:

- **Removed constants:** `WOL_COOLDOWN_SECS`, `RC_SENTRY_PORT`, `POD_AGENT_TIMEOUT_MS`, `POD_AGENT_PORT`
- **Removed struct:** `PodMonitorLocal` (tracked `last_wol_attempt`, `pod_agent_reachable`)
- **Removed from spawn():** `local: HashMap<String, PodMonitorLocal>` variable
- **Simplified check_all_pods signature:** removed `local` parameter
- **Removed:** `healer_flagged` block (pod_needs_restart coordination)
- **Removed:** entire "Try reaching pod-agent" block (lines 309-597 of original) — rc-agent restart via /exec, rc-sentry fallback, WoL send block
- **Removed:** `verify_restart()` function (600-809)
- **Removed:** `check_process_running()`, `check_lock_screen()` helpers
- **Removed imports:** `email_alerts::EmailAlerter`, `wol`
- **Kept:** `backoff.reset()` on natural recovery, WatchdogState::Healthy reset, BonoEvent sends, DashboardEvent::PodUpdate broadcasts, WatchdogState::Restarting/Verifying skip guard
- **Added:** doc comment at stale-pod handling section documenting single-authority contract
- **Added:** `pod_is_marked_offline_when_heartbeat_stale` test
- **Removed tests:** `needs_restart_flag_*` tests (referenced removed coordination mechanism), `next_watchdog_state_on_restart_*` tests (function removed)

Line reduction: 1 file changed, 42 insertions(+), 672 deletions(-)

### Task 2: Authority verification and ROADMAP update

Confirmed:
- `pod_monitor::spawn(state.clone())` at main.rs line 547
- `pod_healer::spawn(state.clone())` at main.rs line 550
- 0 exec/wol matches in pod_monitor.rs (only in comments)
- `run_graduated_recovery`, `/exec` calls confirmed in pod_healer.rs
- `cargo build --release --bin racecontrol`: Finished (0 errors)
- Phase 161 marked complete in ROADMAP.md progress table

## Deviations from Plan

None — plan executed exactly as written.

The `wol` import warning in main.rs is pre-existing (wol module is still used by api/routes.rs and scheduler.rs, declared in lib.rs). Not caused by this task's changes.

The 1 failing test (`config::tests::env_var_overrides_relay_secret`) is pre-existing and unrelated to pod_monitor changes.

## Verification Results

```
cargo test -p racecontrol-crate: 449 passed, 1 failed (pre-existing config test)
cargo build --release --bin racecontrol: Finished release (0 errors)
grep "wol::" pod_monitor.rs: 0 matches
grep "record_attempt" pod_monitor.rs: 2 matches (tests only — setup calls in backoff_reset test)
grep "verify_restart" pod_monitor.rs: 0 matches
grep "backoff.reset()" pod_monitor.rs: 1 match (natural recovery path kept)
grep "PodStatus::Offline" pod_monitor.rs: 3 matches (production) + 2 in tests
```

## Self-Check: PASSED

Files exist:
- FOUND: crates/racecontrol/src/pod_monitor.rs
- FOUND: .planning/ROADMAP.md

Commits exist:
- FOUND: 21c8f6f5 (feat(161-02): strip restart/WoL execution from pod_monitor)
- FOUND: a97015a8 (chore(161-02): verify single-authority and update ROADMAP Phase 161)

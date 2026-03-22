---
phase: 161-pod-monitor-merge
plan: 01
subsystem: infra
tags: [rust, recovery, pod-healer, graduated-recovery, cascade-guard]

requires:
  - phase: 160-pod-monitor-merge
    provides: context and phase setup for pod monitor merge
provides:
  - PodRecoveryTracker struct and PodRecoveryStep enum in pod_healer.rs
  - run_graduated_recovery function: 4-step offline pod recovery with gates
  - maintenance and billing gates on all recovery actions

affects:
  - 161-02 (pod monitor merge: plan 02 removes needs_restart and integrates graduated recovery)
  - pod_healer
  - pod_monitor

tech-stack:
  added: []
  patterns:
    - "Graduated recovery: wait 30s -> Tier 1 restart -> AI escalation -> staff alert"
    - "Gate pattern: check in_maintenance and billing_active before any recovery action"
    - "Local HashMap<String, PodRecoveryTracker> carried across healer loop ticks"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "PodRecoveryTracker is local to heal_all_pods loop — not in AppState, avoids shared state complexity"
  - "Step 1 uses SkipCascadeGuardActive action in RecoveryLogger (no dedicated Wait variant in RecoveryAction) since waiting is a skip, not a restart"
  - "Offline pods route to run_graduated_recovery; online pods still run heal_pod for proactive diagnostics"
  - "AlertStaff step does not advance — stays at AlertStaff and re-alerts each cycle until pod recovers"

patterns-established:
  - "Gate pattern: check in_maintenance -> check billing -> proceed with recovery"
  - "Graduated recovery: deterministic step machine with Instant-based 30s gate for first transition"

requirements-completed: [PMON-01, PMON-03]

duration: 20min
completed: 2026-03-22
---

# Phase 161 Plan 01: Pod Monitor Merge Summary

**PodRecoveryTracker with 4-step graduated offline recovery — wait 30s, Tier 1 restart, AI escalation, staff alert — gated on in_maintenance and billing_active checks**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-22T15:30:00+05:30
- **Completed:** 2026-03-22T15:50:00+05:30
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added `PodRecoveryStep` enum (Waiting/TierOneRestart/AiEscalation/AlertStaff) and `PodRecoveryTracker` struct to pod_healer.rs
- Added `run_graduated_recovery` function with full 4-step recovery ladder and both in_maintenance and billing gates
- Updated `heal_all_pods` to carry `HashMap<String, PodRecoveryTracker>` across ticks; offline pods go to graduated recovery, online pods to existing `heal_pod`
- All steps log to recovery-log.jsonl via RecoveryLogger; cascade_guard.record() called before Tier 1 restart
- 3 new unit tests for tracker lifecycle; 452 lib tests pass (2 pre-existing failures unrelated to this plan)

## Task Commits

1. **Task 1: Add PodRecoveryTracker to pod_healer** - `0670970e` (feat)

## Files Created/Modified

- `crates/racecontrol/src/pod_healer.rs` - Added PodRecoveryStep enum, PodRecoveryTracker struct, run_graduated_recovery function, updated heal_all_pods signature and loop

## Decisions Made

- PodRecoveryTracker held in a local HashMap inside the spawned loop (not in AppState) to avoid shared state complexity. The healer loop is single-task so no synchronization needed.
- Step 1 logs using `RecoveryAction::SkipCascadeGuardActive` with reason "graduated_step1_wait_30s" — no dedicated "Wait" action variant exists in RecoveryAction, and waiting is semantically a skip.
- AlertStaff step does not advance to a new step; it re-alerts every healer cycle until the pod comes back online and `reset()` is called.
- `Instant::checked_sub` used for the 31s test to avoid panicking on underflow.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Package name in Cargo.toml is `racecontrol-crate` not `racecontrol` — used correct package name for `cargo test -p racecontrol-crate`.
- 2 pre-existing test failures (`config::tests::config_fallback_preserved_when_no_env_vars`, `crypto::encryption::tests::load_keys_valid_hex`) confirmed pre-existing by checking base commit before stash pop.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 01 complete: PodRecoveryTracker is in place, graduated recovery fires for offline pods.
- Plan 02 can now remove `pod_needs_restart` and have pod_monitor signal offline state directly to the graduated recovery tracker.

## Self-Check: PASSED

- FOUND: .planning/phases/161-pod-monitor-merge/161-01-SUMMARY.md
- FOUND: commit 0670970e

---
*Phase: 161-pod-monitor-merge*
*Completed: 2026-03-22*

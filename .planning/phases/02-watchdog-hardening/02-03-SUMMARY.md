---
phase: 02-watchdog-hardening
plan: 03
subsystem: infra
tags: [rust, watchdog, healer, state-machine, boundary-enforcement]

# Dependency graph
requires:
  - phase: 02-watchdog-hardening
    plan: 01
    provides: WatchdogState enum, pod_watchdog_states, pod_needs_restart in AppState

provides:
  - pod_healer skips diagnostic cycle for pods in Restarting or Verifying WatchdogState
  - needs_restart flag set only for genuine rc-agent failure (lock screen down + no WS + no billing)
  - WS liveness uses sender.is_closed() instead of contains_key()
  - Healer no longer advances backoff (record_attempt removed — monitor-only)
  - should_skip_for_watchdog_state() pure helper for testability

affects:
  - 02-04 (pod_monitor — consumes needs_restart flag set by healer)
  - future healer additions — established pattern: healer flags, monitor acts

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Boundary enforcement: healer detects + flags, monitor acts — no concurrent restarts"
    - "Pure helper pattern: should_skip_for_watchdog_state() extracted for testability"
    - "WS liveness: sender.is_closed() not contains_key() — accurate channel status"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "Healer reads pod_watchdog_states but never writes it — FSM transitions are pod_monitor's exclusive job"
  - "needs_restart set only for Rule 2 no-WS failure — disk/memory/zombie issues are healer-only, no restart flag"
  - "Healer reads backoff.ready() for cooldown gating but does NOT call record_attempt() — advancing backoff is monitor-only"
  - "should_skip_for_watchdog_state() extracted as pure fn — tests can verify skip logic without async AppState"

patterns-established:
  - "Healer-monitor boundary: healer sets flags, monitor consumes them — never restart from healer directly"
  - "WS channel liveness: always use sender.is_closed() in place of contains_key()"

requirements-completed: [WD-01, WD-03]

# Metrics
duration: 4min
completed: 2026-03-13
---

# Phase 2 Plan 03: Healer/Monitor Boundary Enforcement Summary

**pod_healer reads WatchdogState to skip recovery pods, sets needs_restart flag for genuine rc-agent failures, and uses is_closed() for WS liveness — eliminating concurrent restart races**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-13T00:06:00Z
- **Completed:** 2026-03-13T00:09:42Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- heal_pod() now reads pod_watchdog_states and returns early for pods in Restarting or Verifying state — no conflicting diagnostic actions during active recovery
- Rule 2 (lock screen unresponsive + no WS + no billing) now writes `needs_restart = true` to AppState so pod_monitor receives the restart signal on next cycle
- WS liveness fixed: `sender.is_closed()` replaces `contains_key()` — contains_key returns true for dead channels; is_closed() is accurate
- record_attempt() removed from healer action block — advancing backoff is pod_monitor's exclusive responsibility
- 9 unit tests added covering all WatchdogState variants (skip/continue) and the needs_restart decision tree (WS + billing guard combinations)

## Task Commits

1. **Task 1: WatchdogState skip logic + WS liveness fix** - `8d2afe9` (feat)
2. **Task 2: Set needs_restart flag, remove record_attempt** - `8f206c6` (feat)

## Files Created/Modified

- `crates/racecontrol/src/pod_healer.rs` - WatchdogState skip check at top of heal_pod(), is_closed() WS liveness, needs_restart flag in Rule 2, record_attempt removal, 9 unit tests

## Decisions Made

- Extracted `should_skip_for_watchdog_state()` as a pure function for testability — heal_pod() delegates to it rather than inlining the match, which allows unit testing the skip logic without constructing a full async AppState
- Healer only reads `backoff.ready()` for cooldown gating — it was wrong for healer to advance backoff because that would interfere with monitor's escalating backoff sequence
- Healer continues to diagnose RecoveryFailed pods (not skipped) — even when all restarts are exhausted, the healer's disk/zombie/temp fixes can still help while awaiting manual intervention

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- Healer/monitor boundary is now clean: healer detects problems and flags them; monitor owns the restart lifecycle
- pod_needs_restart flag is set correctly — pod_monitor (Plan 02-02) can consume it to trigger restarts
- All 83 racecontrol tests pass with no regressions

---
*Phase: 02-watchdog-hardening*
*Completed: 2026-03-13*

## Self-Check: PASSED

- [x] SUMMARY.md created: `.planning/phases/02-watchdog-hardening/02-03-SUMMARY.md`
- [x] Commit 8d2afe9 exists: `feat(02-03): WatchdogState skip logic + WS liveness fix in pod_healer`
- [x] Commit 8f206c6 exists: `feat(02-03): set needs_restart flag for genuine rc-agent failures`
- [x] All 83 racecontrol tests pass
- [x] cargo build -p racecontrol-crate clean (only pre-existing warnings)
- [x] Grep: contains_key not used for WS liveness in pod_healer.rs
- [x] Grep: record_attempt not called in pod_healer.rs (comment only)

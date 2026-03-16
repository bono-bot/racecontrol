---
phase: 24-crash-hang-launch-usb-bot-patterns
plan: 04
subsystem: agent
tags: [rust, tokio, watch-channel, failure-monitor, driving-detector, pod-state]

requires:
  - phase: 24-crash-hang-launch-usb-bot-patterns
    plan: 02
    provides: "fix functions for frozen game, launch timeout, USB reconnect in ai_debugger.rs"
  - phase: 24-crash-hang-launch-usb-bot-patterns
    plan: 03
    provides: "failure_monitor.rs task with FailureMonitorState + spawn() + detection logic"

provides:
  - "DrivingDetector::last_udp_packet_elapsed_secs() public accessor"
  - "tokio::sync::watch channel for FailureMonitorState in main()"
  - "failure_monitor::spawn() called after self_monitor — live polling task every 5s"
  - "13 failure_monitor_tx.send_modify() call sites keeping FailureMonitorState current"
  - "PodStateSnapshot in ai_result handler populated with last_udp_secs_ago, game_launch_elapsed_secs, hid_last_error"

affects:
  - 24-crash-hang-launch-usb-bot-patterns
  - future bot fix phases

tech-stack:
  added: []
  patterns:
    - "tokio::sync::watch::Sender::send_modify() for partial in-place state updates from event loop"
    - "failure_monitor_tx.send_modify() pattern: inline at every state change site, never blocks"
    - "All game_pid, launch_started_at, hid_connected, billing_active, recovery_in_progress updated atomically at transition sites"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/driving_detector.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "send_modify() used instead of full send() at all update sites — enables partial field updates without cloning the whole state"
  - "game_pid update combined with launch_started_at = None at crash site (Sites 3b+5) since both happen simultaneously on game exit"
  - "crash_recovery_timer handler clears billing_active + recovery_in_progress together (Site 3c) — covers force-reset case"
  - "SubSessionEnded clears only launch_started_at + recovery_in_progress, NOT billing_active — billing stays active between splits"
  - "BillingStarted site added even though billing_active is also in HeartbeatStatus — failure_monitor reads its own copy for recovery suppression logic"

patterns-established:
  - "State update pattern: 'let _ = failure_monitor_tx.send_modify(|s| { s.field = value; });' — used at every event loop transition"
  - "send_modify errors ignored with let _ = — receiver drop is impossible before main exits, error means task already dead"

requirements-completed: [CRASH-01, CRASH-02, CRASH-03, UI-01, USB-01]

duration: 18min
completed: 2026-03-16
---

# Phase 24 Plan 04: Failure Monitor Wiring Summary

**failure_monitor spawned as live task in rc-agent with 13 state update sites keeping all 6 FailureMonitorState dimensions current from the event loop**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-16T11:40:00Z
- **Completed:** 2026-03-16T11:58:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `DrivingDetector::last_udp_packet_elapsed_secs()` accessor with 2 TDD tests
- Created `tokio::sync::watch::channel(FailureMonitorState::default())` in main() and spawned `failure_monitor::spawn()` after self_monitor
- Added 13 `failure_monitor_tx.send_modify()` call sites across all state transition points in the event loop
- Populated 3 new `PodStateSnapshot` fields in the ai_result handler (last_udp_secs_ago, game_launch_elapsed_secs, hid_last_error)
- Full workspace test suite: rc-common 112, racecontrol-crate 288, rc-agent all tests — 0 failures

## Task Commits

1. **Task 1: Add last_udp_packet_elapsed_secs() + wire failure_monitor module** - `d87d34d` (feat)
2. **Task 2: Add FailureMonitorState update sites + 3 new PodStateSnapshot fields** - `17344fd` (feat)

## Files Created/Modified

- `crates/rc-agent/src/driving_detector.rs` - Added `last_udp_packet_elapsed_secs()` public accessor + 2 tests
- `crates/rc-agent/src/main.rs` - Watch channel creation, failure_monitor::spawn(), 13 send_modify sites, updated PodStateSnapshot

## Decisions Made

- `send_modify()` used (not `send()`) at all update sites — allows partial field updates without cloning full state on every change
- Game crash handler (Site 3b) also clears `game_pid` in one send_modify call since both transitions happen simultaneously
- SubSessionEnded does NOT clear `billing_active` — billing stays active between session splits
- `crash_recovery_timer` handler clears both `billing_active` and `recovery_in_progress` together in one atomic update

## Deviations from Plan

None — plan executed exactly as written. The game_pid Sites 3b/5 were combined into one send_modify call since both state changes happen at the same code location.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- failure_monitor is live end-to-end: state flows from main loop → watch channel → monitor task → try_auto_fix
- CRASH-01 (game freeze), CRASH-02 (launch timeout), and USB-01 (reconnect) are now fully wired
- PodStateSnapshot in ai_result handler includes all 3 context fields for richer AI prompts
- Phase 24 Wave 2 complete — ready for Phase 25 (billing bot) or fleet deploy

---
*Phase: 24-crash-hang-launch-usb-bot-patterns*
*Completed: 2026-03-16*

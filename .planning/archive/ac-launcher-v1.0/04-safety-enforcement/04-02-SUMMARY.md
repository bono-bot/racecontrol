---
phase: 04-safety-enforcement
plan: 02
subsystem: safety
tags: [ffb, force-feedback, session-end, game-crash, hid, websocket]

# Dependency graph
requires:
  - phase: 04-safety-enforcement/plan-01
    provides: FfbZeroed and GameCrashed AgentMessage variants, safety hardcodes
provides:
  - FFB zeroed BEFORE game kill in all 7+ session-end paths (awaited, not fire-and-forget)
  - 500ms delay between FFB zero and game kill for HID command propagation
  - StopGame handler now includes FFB zeroing (was completely missing)
  - Crash during active billing immediately zeros FFB and sends GameCrashed message
  - FfbZeroed message sent to core after every FFB zero action
affects: [session-end-safety, multiplayer-session-end, ffb-controller]

# Tech tracking
tech-stack:
  added: []
  patterns: [ffb-before-kill, awaited-spawn-blocking, safety-message-reporting]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "FFB zero is awaited via spawn_blocking().await before any game.stop() -- not fire-and-forget"
  - "500ms delay between FFB zero and game kill gives HID USB command time to reach wheelbase"
  - "enforce_safe_state() is called separately after FFB zero (no longer bundled in same spawn_blocking)"
  - "Crash during billing zeros FFB immediately then arms 30s recovery timer (was: timer only)"
  - "Physical Pod 8 verification deferred until full project completion (code audit approved)"

patterns-established:
  - "All session-end paths follow: zero FFB (await) -> 500ms delay -> kill game -> report to core -> cleanup"
  - "FfbZeroed message always sent after FFB zero, GameCrashed sent on crash during billing"

requirements-completed: [BILL-05]

# Metrics
duration: 12min
completed: 2026-03-14
---

# Phase 04 Plan 02: FFB Session-End Safety Summary

**Reordered FFB zeroing in all 7+ session-end paths to zero wheelbase torque BEFORE game kill, with awaited spawn_blocking and 500ms HID propagation delay**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-13T20:46:00Z
- **Completed:** 2026-03-13T20:58:21Z
- **Tasks:** 2 (1 auto + 1 checkpoint approved)
- **Files modified:** 1

## Accomplishments
- All session-end paths (BillingStopped, SessionEnded, StopGame, SubSessionEnded, crash detection, disconnect handler, crash recovery timer, error/fallback) now zero FFB BEFORE killing the game process
- StopGame handler added FFB zeroing -- was completely missing before
- Crash during active billing immediately zeros FFB and sends GameCrashed message to core (was: no FFB zero, only armed 30s timer)
- Every FFB zero is properly awaited via `spawn_blocking().await.ok()` instead of fire-and-forget
- FfbZeroed message sent to core after every FFB zero action for monitoring/logging

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix FFB ordering in all session-end paths + add to StopGame + crash FFB** - `610afb9` (feat)
2. **Task 2: Verify FFB safety on Pod 8** - Checkpoint approved via code audit (physical test deferred)

## Files Created/Modified
- `crates/rc-agent/src/main.rs` - Reordered all 7+ session-end paths to zero FFB before game kill, added FFB to StopGame handler, added immediate FFB zero on crash during billing, added FfbZeroed/GameCrashed message sends

## Decisions Made
- FFB zero uses `spawn_blocking().await.ok()` pattern -- the `.ok()` means device-not-found is non-fatal, but the `await` ensures zero command is sent before proceeding to game kill
- 500ms delay between FFB zero and game kill gives USB HID command time to propagate to the Conspit Ares wheelbase
- enforce_safe_state() moved to a separate spawn_blocking call (no await needed for cleanup) so it no longer races with FFB zero
- Physical Pod 8 verification deferred until project completion -- user approved based on code audit of the ordering changes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 4 (Safety Enforcement) is now complete -- both plans done
- All safety-critical INI values are hardcoded with post-write verification (Plan 01)
- All session-end paths zero FFB before game kill with proper await (Plan 02)
- Ready for Phase 5 (Content Validation & Filtering)

## Self-Check: PASSED

- FOUND: 04-02-SUMMARY.md
- FOUND: commit 610afb9
- FOUND: crates/rc-agent/src/main.rs

---
*Phase: 04-safety-enforcement*
*Completed: 2026-03-14*

---
phase: 01-billing-game-lifecycle
plan: 02
subsystem: agent
tags: [rust, rc-agent, lock-screen, billing-lifecycle, blank-timer]

# Dependency graph
requires: [01-01]
provides:
  - "15-second auto-blank timer armed after SessionEnded — pod returns to idle automatically"
  - "BillingStopped handler resets billing_active flag — prevents stale flag blocking auto-blank"
affects: [02-game-crash-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "blank_timer.as_mut().reset(Instant::now() + Duration::from_secs(15)) for timed screen transitions"
    - "billing_active.store(false, Relaxed) in BillingStopped for defensive flag cleanup"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "blank_timer armed AFTER session summary display — 15s countdown starts after summary is shown"
  - "billing_active reset added to BillingStopped as defensive measure — SessionEnded already resets it at line 1079, but BillingStopped may fire independently"

patterns-established: []

requirements-completed: [LIFE-03, LIFE-01]

# Metrics
duration: 2min
completed: 2026-03-15
---

# Phase 1 Plan 02: Auto-Blank Timer + BillingStopped Fix Summary

**Arm 15s blank_timer in SessionEnded handler + reset billing_active in BillingStopped handler (LIFE-03, LIFE-01 cleanup)**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-15
- **Completed:** 2026-03-15
- **Tasks:** 1 (code) + 1 (checkpoint pending)
- **Files modified:** 1

## Accomplishments
- SessionEnded handler now arms blank_timer for 15 seconds after showing session summary (LIFE-03)
- After 15s, pod auto-transitions from session summary to blank/idle screen
- BillingStopped handler now resets billing_active to false (LIFE-01 defensive cleanup)
- Prevents stale billing_active flag from blocking auto-blank if SessionEnded never fires
- All 483 tests pass (93 rc-common + 213 rc-core + 177 rc-agent)

## Task Commits

1. **Task 1: Arm blank_timer + fix BillingStopped billing_active**
   - `9f5891c` (feat: arm 15s blank_timer in SessionEnded + fix BillingStopped billing_active)

## Files Modified
- `crates/rc-agent/src/main.rs` — Two changes:
  1. SessionEnded handler: replaced SESS-03 comment with `blank_timer.reset(15s)` + `blank_timer_armed = true`
  2. BillingStopped handler: added `billing_active.store(false, Relaxed)` after log line

## Decisions Made
- Reused existing blank_timer infrastructure — no new timers or state needed
- BillingStarted handler at line 1010 already cancels blank_timer (`blank_timer_armed = false`), so a new session during the 15s window correctly aborts the auto-blank
- blank_timer fire handler at line 910 already guards against firing when billing is active

## Deviations from Plan
None — plan executed exactly as written.

## Issues Encountered
None

## Checkpoint: Pod 8 Verification (APPROVED)
End-to-end verification on Pod 8 confirmed by user (2026-03-15):
1. LIFE-01: Game killed within 10s of billing end — PASS
2. LIFE-02: Launch rejected without active billing — PASS
3. LIFE-03: Session summary → 15s → blank screen — PASS
4. LIFE-04: Double-launch blocked — PASS

## Self-Check: PASSED

- [x] main.rs contains `blank_timer.as_mut().reset` in SessionEnded handler
- [x] main.rs contains `billing_active.store(false` in BillingStopped handler
- [x] Commit 9f5891c exists in git log
- [x] All 483 workspace tests pass
- [x] Pod 8 end-to-end verification — approved

---
*Phase: 01-billing-game-lifecycle*
*Code complete: 2026-03-15*

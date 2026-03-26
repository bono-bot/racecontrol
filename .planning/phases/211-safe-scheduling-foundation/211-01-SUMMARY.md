---
phase: 211-safe-scheduling-foundation
plan: 01
subsystem: infra
tags: [bash, pid-guard, cooldown, venue-aware, sentinel, autonomous-execution, safety]

requires:
  - phase: 202-audit-protocol
    provides: audit/lib/core.sh with venue_state_detect() and safe_remote_exec()
  - phase: 202-audit-protocol
    provides: audit/lib/fixes.sh with check_pod_sentinels() and approved fix functions

provides:
  - PID file run guard preventing concurrent auto-detect executions (SCHED-03)
  - 6-hour per-pod+issue escalation cooldown infrastructure (SCHED-04)
  - Venue-state-aware mode selection (open venue forces quick mode, SCHED-05)
  - Extended sentinel check covering OTA_DEPLOYING and MAINTENANCE_MODE

affects:
  - 211-02 (Task Scheduler registration needs safe gates before live trigger fires)
  - 213 (WhatsApp wiring will use _is_cooldown_active/_record_alert infrastructure)
  - Any future phase that calls check_pod_sentinels (now also blocks during MAINTENANCE_MODE)

tech-stack:
  added: []
  patterns:
    - "PID file guard pattern: write PID to /tmp/auto-detect.pid, EXIT trap cleans up, kill -0 tests liveness"
    - "Per-pod+issue cooldown: JSON map keyed pod_ip:issue_type, 6h threshold via Unix epoch arithmetic"
    - "Venue-state mode override: source core.sh after arg parsing, venue_state_detect() result gates MODE"

key-files:
  created: []
  modified:
    - scripts/auto-detect.sh
    - audit/lib/fixes.sh
    - .gitignore

key-decisions:
  - "PID file at /tmp/auto-detect.pid (not REPO_ROOT) -- survives Git clean, host-local, no repo pollution"
  - "Venue-state source inserted BEFORE prerequisites so MODE is locked before any subsequent logic reads it"
  - "Cooldown functions defined after log() -- needed for _record_alert to use log() in future"
  - "WhatsApp send function deferred to Phase 213 -- infrastructure (cooldown gate + _record_alert) wired now"
  - "check_pod_sentinels() extended backward-compatibly -- same return code contract (1=blocked), callers unchanged"

patterns-established:
  - "Safety-gates-first ordering: PID guard and venue-mode selection fire before any work"
  - "Cooldown keyed per pod_ip:issue_type -- never fleet-level, prevents storm suppression hiding different issues"

requirements-completed:
  - SCHED-03
  - SCHED-04
  - SCHED-05

duration: 15min
completed: 2026-03-26
---

# Phase 211 Plan 01: Safe Scheduling Foundation Summary

**Five safety gates added to auto-detect.sh: PID run guard (SCHED-03), 6-hour per-pod+issue escalation cooldown (SCHED-04), venue-state-aware mode override (SCHED-05), and extended MAINTENANCE_MODE sentinel check**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-26T05:46:00Z
- **Completed:** 2026-03-26T05:52:00Z
- **Tasks:** 2 (Task 1: implement; Task 2: verify concurrent execution)
- **Files modified:** 3

## Accomplishments

- PID file guard prevents overlapping auto-detect runs -- second invocation exits immediately with "already running (PID X)" message, EXIT trap cleans up the lock file
- Escalation cooldown infrastructure (_is_cooldown_active, _record_alert) with 6-hour threshold, keyed per pod_ip:issue_type -- prevents WhatsApp alert storms without suppressing new pod+issue combos
- Venue-state mode override: if venue is open (active billing session OR IST 09:00-22:00), mode is forced to quick regardless of --mode argument
- check_pod_sentinels() extended to also check MAINTENANCE_MODE sentinel file (backward-compatible, same return code)
- Concurrent execution test PASSED: second instance blocked with "already running", PID file cleaned by EXIT trap, sequential third run succeeded

## Task Commits

1. **Task 1: PID guard, cooldown, venue-aware mode, extended sentinel** - `090b2b32` (feat)
2. **Task 2: Concurrent execution verification** - verification-only, no new files (uses Task 1 commit)

## Files Created/Modified

- `scripts/auto-detect.sh` - Added PID guard, cooldown functions, venue-state mode selection, cooldown wiring in notify step, updated main() banner
- `audit/lib/fixes.sh` - Extended check_pod_sentinels() to check OTA_DEPLOYING AND MAINTENANCE_MODE
- `.gitignore` - Added audit/results/auto-detect-cooldown.json exclusion

## Decisions Made

- PID file at `/tmp/auto-detect.pid` (not REPO_ROOT) -- survives git clean, host-local scope, no repo pollution
- Venue-state detection sourced immediately after arg parsing so MODE is locked before prerequisites block
- Cooldown functions placed after log() definition (needed for future _record_alert to emit log lines)
- WhatsApp send deferred to Phase 213 -- cooldown gate infrastructure wired now, Phase 213 adds the send call only
- MAINTENANCE_MODE check added to check_pod_sentinels() with same return code (1=blocked) -- all existing callers work unchanged

## Deviations from Plan

None -- plan executed exactly as written.

## Issues Encountered

None. Dry-run verified both expected log lines ("PID lock acquired" and "Venue state:"). Concurrent execution test blocked the second instance as required. PID file absent after completion.

## Next Phase Readiness

- All three SCHED safety gates are live in auto-detect.sh and verified via dry-run and concurrent test
- Phase 211 Plan 02 can now safely register the Task Scheduler trigger -- PID guard prevents overlapping runs at 02:35 IST
- Phase 213 WhatsApp wiring only needs to insert the send call inside the existing cooldown gate block
- Sentinel check covers both OTA and MAINTENANCE_MODE for Phase 213 fix engine

---
*Phase: 211-safe-scheduling-foundation*
*Completed: 2026-03-26*

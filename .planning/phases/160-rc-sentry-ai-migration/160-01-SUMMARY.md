---
phase: 160-rc-sentry-ai-migration
plan: 01
subsystem: infra
tags: [rust, rc-sentry, recovery-logger, sentinel, watchdog, rc-common]

requires:
  - phase: 159-recovery-consolidation-foundation
    provides: RecoveryLogger, RecoveryDecision, RecoveryAction, RecoveryAuthority, RECOVERY_LOG_POD from rc-common/src/recovery.rs

provides:
  - RCAGENT_SELF_RESTART_SENTINEL constant in tier1_fixes.rs (C:\RacingPoint\rcagent-restart-sentinel.txt)
  - is_rcagent_self_restart() helper with cfg(test) guard always-false
  - handle_crash() combined is_graceful flag covering both GRACEFUL_RELAUNCH and RCAGENT_SELF_RESTART
  - RecoveryLogger wired into crash-handler thread in main.rs
  - build_restart_decision() pure helper for testable decision construction
  - Every recovery decision logged to C:\RacingPoint\recovery-log.jsonl

affects:
  - 160-rc-sentry-ai-migration (plans 02+)
  - deploy sequence (RCAGENT_SELF_RESTART sentinel now consumed by rc-sentry)
  - rc-agent (writes sentinel before relaunch_self())

tech-stack:
  added: []
  patterns:
    - "Sentinel file consumed once: check exists → log action → remove_file"
    - "is_graceful = graceful || rcagent_restart — OR-combine all graceful flags"
    - "build_restart_decision() pure fn pattern for testable RecoveryDecision construction"

key-files:
  created: []
  modified:
    - crates/rc-sentry/src/tier1_fixes.rs
    - crates/rc-sentry/src/main.rs

key-decisions:
  - "RCAGENT_SELF_RESTART_SENTINEL path matches CLAUDE.md deploy docs: C:\\RacingPoint\\rcagent-restart-sentinel.txt"
  - "is_graceful OR-combines both sentinel flags so either alone suppresses escalation counter"
  - "RecoveryLogger created once per crash-handler thread lifetime (not per event)"
  - "build_restart_decision() extracted as pub(crate) pure fn — no I/O, fully unit-testable"
  - "EscalateToAi used as fallback action when !restarted && !maintenance_mode (Plan 02 will add real AI query)"

patterns-established:
  - "Recovery audit pattern: every handle_crash() return → RecoveryDecision logged before any other action"
  - "Sentinel pattern: cfg(test) returns false, cfg(not(test)) checks Path::exists()"

requirements-completed: [SENT-03, SENT-04]

duration: 15min
completed: 2026-03-22
---

# Phase 160 Plan 01: RC Sentry AI Migration - Recovery Logger + RCAGENT Sentinel Summary

**RCAGENT_SELF_RESTART sentinel detection wired into handle_crash() plus RecoveryLogger logging every recovery decision to C:\RacingPoint\recovery-log.jsonl via build_restart_decision() pure helper**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-22T14:40:37Z
- **Completed:** 2026-03-22T14:55:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added RCAGENT_SELF_RESTART_SENTINEL constant and is_rcagent_self_restart() helper to tier1_fixes.rs with proper cfg(test) guard
- Combined GRACEFUL_RELAUNCH and RCAGENT_SELF_RESTART into is_graceful flag in handle_crash() — deploy-triggered restarts no longer increment escalation counter
- Wired RecoveryLogger into the crash-handler thread in main.rs — every Restart/Skip/Escalate decision produces a JSONL line
- Extracted build_restart_decision() as a testable pure function, 3 new unit tests covering all action branches
- All 49 rc-sentry tests pass; release build clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Add RCAGENT_SELF_RESTART sentinel to tier1_fixes.rs** - `1b273485` (feat)
2. **Task 2: Wire RecoveryLogger into main.rs crash handler** - `9a3eb2e9` (feat)

_Note: TDD tasks — both phases (RED+GREEN) completed inline per plan_

## Files Created/Modified

- `crates/rc-sentry/src/tier1_fixes.rs` - RCAGENT_SELF_RESTART_SENTINEL constant, is_rcagent_self_restart() helper, is_graceful combined flag, 4 new tests
- `crates/rc-sentry/src/main.rs` - rc_common::recovery use imports, RecoveryLogger::new(RECOVERY_LOG_POD), build_restart_decision() helper, log() call after every handle_crash(), 3 new tests

## Decisions Made

- RCAGENT_SELF_RESTART_SENTINEL path matches CLAUDE.md deploy docs exactly (`C:\RacingPoint\rcagent-restart-sentinel.txt`)
- is_graceful combines both sentinel flags with OR — either alone is sufficient to suppress escalation counter
- RecoveryLogger created once at thread start (not once per crash event) to avoid re-opening path each time
- build_restart_decision() is pub(crate) pure fn for testability — no I/O, takes all inputs as parameters
- EscalateToAi used as the not-restarted && not-maintenance action (real Ollama AI integration is Plan 02)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 160-02 can now query Ollama and log EscalateToAi decisions with real AI suggestions
- SENT-03 (audit trail) and SENT-04 (deploy counter suppression) complete
- rc-agent deploy sequence must write RCAGENT_SELF_RESTART_SENTINEL before calling relaunch_self() — this is the existing deploy protocol per CLAUDE.md

---
*Phase: 160-rc-sentry-ai-migration*
*Completed: 2026-03-22*

## Self-Check: PASSED

- FOUND: crates/rc-sentry/src/tier1_fixes.rs
- FOUND: crates/rc-sentry/src/main.rs
- FOUND: .planning/phases/160-rc-sentry-ai-migration/160-01-SUMMARY.md
- FOUND: commit 1b273485 (Task 1)
- FOUND: commit 9a3eb2e9 (Task 2)

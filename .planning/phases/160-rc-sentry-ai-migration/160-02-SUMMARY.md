---
phase: 160-rc-sentry-ai-migration
plan: 02
subsystem: infra
tags: [rust, rc-sentry, ollama, ai-escalation, pattern-memory, watchdog, SENT-01, SENT-02]

requires:
  - phase: 160-rc-sentry-ai-migration
    plan: 01
    provides: RecoveryLogger, build_restart_decision(), crash-handler loop in main.rs

provides:
  - PATTERN_ESCALATION_THRESHOLD = 3 constant in main.rs
  - should_escalate_pattern(hit_count) helper — returns true when hit_count >= 3
  - Pattern-hit escalation: crash handler skips restart and logs EscalateToAi when same pattern seen 3+ times
  - OLLAMA_TIMEOUT = 8s constant in main.rs
  - query_ollama_with_timeout() — wraps query_async with mpsc + recv_timeout, never blocks longer than timeout
  - Pre-restart Ollama query for unknown patterns (SENT-02) — fires before handle_crash, awaited with 8s deadline
  - Ollama suggestion saved to pattern memory on success

affects:
  - 160-rc-sentry-ai-migration (plans 03+)
  - rc-sentry crash-handler behavior: crash loops now escalate instead of infinite restart

tech-stack:
  added: []
  patterns:
    - "Pattern escalation: hit_count >= threshold → log EscalateToAi + continue (no restart)"
    - "Bounded async: query_async + mpsc::channel + recv_timeout → Option<OllamaResult>"
    - "TDD: RED (failing test) → GREEN (implementation) → tests pass"

key-files:
  created: []
  modified:
    - crates/rc-sentry/src/main.rs

key-decisions:
  - "PATTERN_ESCALATION_THRESHOLD = 3 — same crash 3+ times in session triggers AI escalation instead of restart"
  - "query_ollama_with_timeout uses mpsc::channel + recv_timeout(8s) — no new threading primitives, reuses query_async"
  - "Ollama query fires BEFORE handle_crash for unknown patterns — suggestion available for operator before restart occurs"
  - "Ollama timeout/unavailable is non-blocking: warn + proceed with restart regardless"
  - "Ollama suggestion saved to pattern memory immediately so next crash has instant_fix available"
  - "Old fire-and-forget post-restart Ollama block fully removed — replaced by pre-restart bounded query"

requirements-completed: [SENT-01, SENT-02]

duration: 20min
completed: 2026-03-22
---

# Phase 160 Plan 02: RC Sentry AI Migration - Pattern Escalation + Pre-Restart Ollama Query Summary

**Pattern hit-count escalation skips restart after 3 same-pattern crashes and pre-restart Ollama query with 8s timeout consults AI before handle_crash for unknown patterns**

## Performance

- **Duration:** 20 min
- **Started:** 2026-03-22T14:55:00Z (approx)
- **Completed:** 2026-03-22T15:15:00Z (approx)
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Added `PATTERN_ESCALATION_THRESHOLD = 3` constant and `should_escalate_pattern(hit_count)` helper
- Crash handler now checks `pattern_hit_count` BEFORE calling `handle_crash` — when threshold exceeded, logs EscalateToAi decision and skips restart entirely (resolves SENT-01: no more blind crash loops)
- Added `OLLAMA_TIMEOUT = 8s` constant and `query_ollama_with_timeout()` wrapper using mpsc + recv_timeout
- Moved Ollama query BEFORE `handle_crash` for unknown patterns — suggestion logged and saved to pattern memory before restart attempt (resolves SENT-02)
- Removed old fire-and-forget post-restart Ollama block — now fully pre-restart with bounded wait
- 4 new unit tests: `should_escalate_below_threshold`, `should_escalate_at_threshold`, `query_ollama_timeout_respects_deadline`, `query_ollama_with_timeout_returns_result_when_fast`
- All 53 rc-sentry tests pass; release build clean (no new warnings)

## Task Commits

1. **Task 1: Pattern-hit escalation — skip restart after 3 same-pattern crashes** - `330aaf7b` (feat)
2. **Task 2: Pre-restart Ollama query with 8-second timeout for unknown patterns** - `d07c686d` (feat)

## Files Created/Modified

- `crates/rc-sentry/src/main.rs` — PATTERN_ESCALATION_THRESHOLD, OLLAMA_TIMEOUT, should_escalate_pattern(), query_ollama_with_timeout(), escalation check in crash handler loop, pre-restart Ollama block, removed fire-and-forget, 4 new tests

## Decisions Made

- PATTERN_ESCALATION_THRESHOLD = 3 (same value as RestartTracker threshold for consistency)
- query_ollama_with_timeout uses mpsc::channel + recv_timeout — minimal code, no new deps, reuses existing query_async interface
- Ollama query placed BEFORE handle_crash so suggestion is available before the restart happens
- Ollama unavailability/timeout is fully non-blocking — warn log then proceed normally
- Suggestion written to pattern memory immediately so next occurrence gets instant_fix

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external configuration required.

## Next Phase Readiness

- SENT-01 and SENT-02 are complete
- rc-sentry now escalates crash loops to AI and queries Ollama before unknown-pattern restarts
- Future plans can add fleet-level reporting (Phase 105 comment in main.rs marks the hook point)

---
*Phase: 160-rc-sentry-ai-migration*
*Completed: 2026-03-22*

## Self-Check: PASSED

- FOUND: crates/rc-sentry/src/main.rs
- FOUND: .planning/phases/160-rc-sentry-ai-migration/160-02-SUMMARY.md
- FOUND: commit 330aaf7b (Task 1)
- FOUND: commit d07c686d (Task 2)

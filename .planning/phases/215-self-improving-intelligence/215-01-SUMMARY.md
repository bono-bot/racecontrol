---
phase: 215-self-improving-intelligence
plan: 01
subsystem: infra
tags: [bash, jq, jsonl, intelligence, pattern-tracking, trend-analysis, auto-detect]

# Dependency graph
requires:
  - phase: 213-self-healing-escalation
    provides: auto-detect.sh pipeline with findings.json + fixes.jsonl per run
  - phase: 214-bono-coordination
    provides: generate_report_and_notify() with write_completion_marker hook pattern

provides:
  - scripts/intelligence/pattern-tracker.sh with update_pattern_db() function
  - scripts/intelligence/trend-analyzer.sh with run_trend_analysis() function
  - suggestions.jsonl growing by N entries per auto-detect run (N = findings count)
  - TREND_OUTLIER entries flagging pods with > 4x fleet average bug occurrences

affects:
  - 215-self-improving-intelligence
  - Phase 216 tests (will test suggestions.jsonl growth and TREND_OUTLIER generation)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Source guard pattern: [[ ${_VAR:-} == 1 ]] && return 0 prevents double-sourcing"
    - "Coord guard pattern for post-report hooks: if [[ $(type -t fn) == function ]]; then fn; fi"
    - "jq -sc slurp for JSONL reading, >> append for JSONL writing"
    - "Graceful degradation: every error path returns 0, never propagates to caller"

key-files:
  created:
    - scripts/intelligence/pattern-tracker.sh
    - scripts/intelligence/trend-analyzer.sh
  modified:
    - scripts/auto-detect.sh

key-decisions:
  - "SUGGESTIONS_JSONL path is audit/results/suggestions.jsonl (shared between pattern-tracker and trend-analyzer)"
  - "fix_success = fix_applied AND after_state has no FAIL/blocked/skipped/UNRESOLVED — checks fixes.jsonl for same pod"
  - "Minimum 10 entries required before trend analysis runs (statistical guard against false outliers)"
  - "TREND_OUTLIER entries filtered out when reading for analysis (prevents feedback loop inflating pod counts)"
  - "Intelligence hooks placed after write_completion_marker and before return in generate_report_and_notify()"
  - "Source with || true fallback: missing intelligence scripts never abort the pipeline"

patterns-established:
  - "Post-report hook pattern: source script with || true at top, call with type -t guard at use site"
  - "JSONL append pattern: one jq -n entry per finding, echo >> file (never overwrite)"
  - "Statistical outlier: group_by(.pod_ip), compute fleet_avg, flag where pod_count > fleet_avg * threshold"

requirements-completed: [LEARN-01, LEARN-04]

# Metrics
duration: 20min
completed: 2026-03-26
---

# Phase 215 Plan 01: Pattern Tracking & Trend Analysis Foundation Summary

**Pattern tracking (LEARN-01) + trend outlier detection (LEARN-04) wired into auto-detect.sh — every run now permanently records what was found, what was fixed, and flags pods with 4x+ fleet-average bug frequency**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-26T09:10:00Z
- **Completed:** 2026-03-26T09:30:00Z
- **Tasks:** 3
- **Files modified:** 3 (2 created, 1 modified)

## Accomplishments

- Created scripts/intelligence/pattern-tracker.sh: exports update_pattern_db() which writes one JSONL entry per finding to suggestions.jsonl with all 7 required fields (run_ts, bug_type, pod_ip, severity, fix_applied, fix_success, frequency)
- Created scripts/intelligence/trend-analyzer.sh: exports run_trend_analysis() which reads suggestions.jsonl, groups by bug_type, computes fleet averages, and writes TREND_OUTLIER entries for pods exceeding 4x fleet average
- Wired both intelligence scripts into auto-detect.sh as post-report hooks using the existing coord guard pattern — both calls are non-fatal and degrade gracefully if scripts are absent

## Task Commits

Each task was committed atomically:

1. **Task 1: Create pattern-tracker.sh** - `52ba2db4` (feat)
2. **Task 2: Create trend-analyzer.sh** - `11fe3745` (feat)
3. **Task 3: Wire into auto-detect.sh** - `33c26f6d` (feat)

## Files Created/Modified

- `scripts/intelligence/pattern-tracker.sh` - Exports update_pattern_db(); reads findings.json + fixes.jsonl per run, writes JSONL entries to suggestions.jsonl
- `scripts/intelligence/trend-analyzer.sh` - Exports run_trend_analysis(); statistical outlier detection per bug_type across all runs, writes TREND_OUTLIER entries
- `scripts/auto-detect.sh` - Added source lines for both intelligence scripts (|| true fallback) + guard-wrapped calls at end of generate_report_and_notify()

## Decisions Made

- SUGGESTIONS_JSONL at audit/results/suggestions.jsonl: both scripts share the same path, trend-analyzer reads what pattern-tracker writes
- fix_success determination: fix_applied AND after_state field in fixes.jsonl contains no failure indicators (FAIL, blocked, skipped, UNRESOLVED)
- 10-entry minimum for trend analysis: prevents spurious TREND_OUTLIER flags when only 1-2 runs have completed
- TREND_OUTLIER entries excluded from analysis input via jq select(.entry_type != "TREND_OUTLIER"): prevents compounding inflation of pod counts across runs
- threshold_used and multiplier both captured in TREND_OUTLIER entries for auditability

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all three tasks completed cleanly on first attempt. Integration smoke test passed: injected a finding with issue_type=rc_agent_crash_loop, verified suggestions.jsonl grew by 1 entry with all 7 fields present.

## User Setup Required

None - no external service configuration required. Intelligence scripts source automatically when auto-detect.sh runs.

## Next Phase Readiness

- Pattern tracking and trend analysis foundation is complete
- suggestions.jsonl will grow on every auto-detect run going forward
- Phase 215-02+ (LEARN-07/08/09: self-patch loop) can build on top of suggestions.jsonl entries
- Phase 216 (tests) can validate suggestions.jsonl growth and TREND_OUTLIER generation using the smoke test pattern established in plan verification

## Self-Check

Verified:
- scripts/intelligence/pattern-tracker.sh exists and exports update_pattern_db (sourced + type check passed)
- scripts/intelligence/trend-analyzer.sh exists and exports run_trend_analysis (sourced + type check passed)
- auto-detect.sh has 4 matches for update_pattern_db|run_trend_analysis (wire test passed)
- Integration smoke test: suggestions.jsonl written with correct 7-field structure
- All 3 commits present: 52ba2db4, 11fe3745, 33c26f6d

## Self-Check: PASSED

---
*Phase: 215-self-improving-intelligence*
*Completed: 2026-03-26*

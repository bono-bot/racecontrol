---
phase: 41-test-foundation
plan: 01
subsystem: testing
tags: [bash, shell, e2e, test-infrastructure, pod-map]

# Dependency graph
requires: []
provides:
  - Shared shell test library (tests/e2e/lib/common.sh) with pass/fail/skip/info/summary_exit helpers
  - Pod IP map (tests/e2e/lib/pod-map.sh) with pod_ip() for all 8 pods — single source of truth
  - Refactored smoke.sh, cross-process.sh, game-launch.sh sourcing shared library
  - summary_exit exits with FAIL count only (skips are informational, not failures)
affects:
  - 41-02 (Playwright config — scripts in same tests/e2e/ directory)
  - 42-kiosk-tests (new specs must source lib/common.sh from day one)
  - 43-game-launch-tests (game-launch.sh pattern established)
  - 44-run-all (run-all.sh will orchestrate these refactored scripts)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "source SCRIPT_DIR/lib/common.sh pattern for all E2E shell scripts"
    - "pod_ip() function call pattern replacing inline IP lookup"
    - "summary_exit as single exit point in every shell test script"

key-files:
  created:
    - tests/e2e/lib/common.sh
    - tests/e2e/lib/pod-map.sh
  modified:
    - tests/e2e/smoke.sh
    - tests/e2e/cross-process.sh
    - tests/e2e/game-launch.sh

key-decisions:
  - "summary_exit exits with FAIL count only — skips are informational, do not cause failure"
  - "No set options in lib/common.sh — each calling script manages its own error handling"
  - "TTY check [ -t 1 ] gates color codes — CI captures clean text, terminals get colors"
  - "pod_ip() uses hyphens (pod-1 through pod-8) matching POD_ID variable format — fixes Python dict bug that used underscores"

patterns-established:
  - "Pattern 1: Every new E2E shell script adds SCRIPT_DIR + source lib/common.sh at top"
  - "Pattern 2: Every E2E shell script ends with summary_exit as sole exit point"
  - "Pattern 3: Any script that needs to reach a pod by number calls pod_ip POD_ID from lib/pod-map.sh"

requirements-completed: [FOUND-01, FOUND-02]

# Metrics
duration: 3min
completed: 2026-03-19
---

# Phase 41 Plan 01: Test Foundation — Shell Library Summary

**Shared POSIX shell test library (lib/common.sh + lib/pod-map.sh) with pass/fail/skip/info/summary_exit helpers and pod IP map, refactored into all three existing E2E scripts**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-18T21:31:39Z
- **Completed:** 2026-03-18T21:35:20Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created `tests/e2e/lib/common.sh` with TTY-conditional colors, PASS/FAIL/SKIP counters, pass/fail/skip/info helpers, and `summary_exit` that exits with `$FAIL` count (not `$FAIL + $SKIP`)
- Created `tests/e2e/lib/pod-map.sh` with `pod_ip()` function for all 8 pods — single source of truth for pod IPs matching MEMORY.md network map
- Refactored smoke.sh, cross-process.sh, and game-launch.sh to source `lib/common.sh` and use `summary_exit`, eliminating duplicate inline helper definitions
- Replaced game-launch.sh Python pod IP dict (which used underscores and would fail for `pod-1` format) with `pod_ip "${POD_ID}"` function call

## Task Commits

Each task was committed atomically:

1. **Task 1: Create lib/common.sh and lib/pod-map.sh** - `7323f7c` (feat)
2. **Task 2: Refactor existing scripts to source shared library** - `c390deb` (feat) *(committed by prior session — verified complete)*

## Files Created/Modified
- `tests/e2e/lib/common.sh` - Shared pass/fail/skip/info/summary_exit helpers with TTY-conditional ANSI colors
- `tests/e2e/lib/pod-map.sh` - pod_ip() function returning correct IP for pod-1 through pod-8
- `tests/e2e/smoke.sh` - Sources lib/common.sh; removed inline color/counter/summary blocks
- `tests/e2e/cross-process.sh` - Sources lib/common.sh; removed inline pass/fail/skip definitions
- `tests/e2e/game-launch.sh` - Sources lib/common.sh + lib/pod-map.sh; Python pod IP dict replaced with pod_ip()

## Decisions Made
- `summary_exit` exits with `$FAIL` only — skips are informational (intentional skip on "no billing session" must not fail run-all.sh)
- `lib/common.sh` has NO `set` options — smoke.sh needs `set -euo pipefail`, game-launch.sh needs `set -uo pipefail` (no `-e`); callers own their own error handling
- Pod map uses hyphens (`pod-1` through `pod-8`) matching the `POD_ID` variable format — the original Python dict used underscores and would have silently returned empty string for any pod

## Deviations from Plan

None — plan executed exactly as written. The prior session had already committed the Task 2 refactoring (commit `c390deb`) along with Task 1's library creation. All plan acceptance criteria were verified against the committed state.

## Issues Encountered
- Prior agent session had already executed this plan (commits `c390deb` and `4332d5a` both present). All plan acceptance criteria verified against existing commits — all checks passed. No re-work needed.

## User Setup Required
None — no external service configuration required.

## Next Phase Readiness
- `lib/common.sh` and `lib/pod-map.sh` are ready for Phase 42+ scripts to source on day one
- Phase 41-02 (Playwright config) already committed in prior session (`4332d5a`)
- Phase 42 can write kiosk wizard specs using `source "$SCRIPT_DIR/../lib/common.sh"` pattern
- No blockers for Phase 42

---
*Phase: 41-test-foundation*
*Completed: 2026-03-19*

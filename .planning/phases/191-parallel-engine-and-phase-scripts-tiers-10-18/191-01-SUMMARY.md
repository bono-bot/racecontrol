---
phase: 191-parallel-engine-and-phase-scripts-tiers-10-18
plan: 01
subsystem: infra
tags: [bash, audit, parallel, semaphore, concurrency]

# Dependency graph
requires:
  - phase: 189-core-scaffold-and-shared-primitives
    provides: "audit/lib/core.sh with emit_result, http_get, safe_remote_exec, ist_now"
  - phase: 190-phase-scripts-tiers-1-9
    provides: "audit.sh entry point and tier 1-9 phase scripts"
provides:
  - "audit/lib/parallel.sh: file-based semaphore with mkdir atomic locking (4 slots)"
  - "parallel_pod_loop(): dispatches phase_fn(ip,host) for all 8 pods in parallel with 200ms stagger"
  - "semaphore_acquire() / semaphore_release(): slot-based concurrency control"
  - "wait_all_jobs(): waits for all background PIDs, returns max exit code"
  - "audit.sh updated: sources parallel.sh, loads tier 10-18 in full mode, dispatches phases 45-60"
affects:
  - "191-02 through 191-18: all pod-looping phase scripts use parallel_pod_loop"
  - "audit.sh consumers: any caller of --mode full or --tier 10-18"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "File-based semaphore via mkdir atomicity (POSIX guarantee, works on all filesystems)"
    - "Stale lock detection via kill -0 PID check before eviction"
    - "Background subshell pattern: (semaphore_acquire; fn; semaphore_release) &"
    - "200ms stagger between pod launches to prevent ARP flood"
    - "phase_fn(ip, host) signature convention for all parallel pod phase functions"

key-files:
  created:
    - "audit/lib/parallel.sh"
  modified:
    - "audit/audit.sh"

key-decisions:
  - "mkdir atomic locking chosen over flock -- works on Windows/WSL/Linux without kernel dependency"
  - "MAX_CONCURRENT=4 caps pod connections (8 pods, max 4 simultaneous HTTP calls to prevent network saturation)"
  - "STAGGER_MS=0.2 (200ms) between launches prevents ARP flood when all 8 pods queried simultaneously"
  - "parallel_pod_loop takes function name (not closure) -- enables export -f pattern for subshell dispatch"
  - "Tier 10-18 loading scoped to full mode only -- standard/pre-ship/post-incident don't need extended checks"

patterns-established:
  - "Parallel phase pattern: define phase_fn(ip, host) -> call parallel_pod_loop phase_fn in run_phaseNN"
  - "Semaphore pattern: acquire -> store slot -> do work -> release slot (always in subshell)"

requirements-completed: [EXEC-03, EXEC-04]

# Metrics
duration: 3min
completed: 2026-03-25
---

# Phase 191 Plan 01: Parallel Engine and Phase Scripts Tiers 10-18 Summary

**File-based semaphore parallel engine (audit/lib/parallel.sh) with 4-slot mkdir locking, 200ms pod stagger, and audit.sh updated to source it and dispatch all 60 phases across tiers 1-18 in full mode.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-25T15:10:55Z
- **Completed:** 2026-03-25T15:14:00Z (approx)
- **Tasks:** 2/2
- **Files modified:** 2

## Accomplishments
- Created `audit/lib/parallel.sh` with 4 exported functions: semaphore_acquire, semaphore_release, parallel_pod_loop, wait_all_jobs
- File-based semaphore uses `mkdir` atomic locking with stale PID detection via `kill -0`
- 200ms stagger between pod launches prevents ARP flooding when all 8 pods queried concurrently
- Updated `audit.sh` to source parallel.sh, load tier 10-18 in full mode, and dispatch phases 45-60

## Task Commits

Each task was committed atomically:

1. **Task 1: Create audit/lib/parallel.sh with semaphore and parallel_pod_loop** - `ef9afff9` (feat)
2. **Task 2: Update audit.sh to source parallel.sh and dispatch tiers 10-18** - `8d274061` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `audit/lib/parallel.sh` - Parallel pod execution engine with 4-slot file-based semaphore
- `audit/audit.sh` - Updated to source parallel.sh, load tiers 10-18, dispatch run_phase45 through run_phase60

## Decisions Made
- Used `mkdir` for atomic slot acquisition (not `flock`) -- works on Windows/WSL/Linux without kernel dependency
- Set `MAX_CONCURRENT=4` -- 8 pods but only 4 simultaneous HTTP connections prevents network saturation
- `parallel_pod_loop` accepts a function name string and calls it with `(ip, host)` arguments -- enables `export -f` pattern needed for background subshells
- Tier 10-18 loading in `load_phases()` gated to `full` mode only -- standard/pre-ship/post-incident remain unaffected

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- parallel.sh is ready for all pod-looping phase scripts in tiers 10-18 (plans 191-02 through 191-18)
- `parallel_pod_loop my_fn` is the standard call pattern: define `my_fn(ip, host)` that writes via `emit_result`
- audit.sh full mode dispatches run_phase45 through run_phase60 -- phase scripts must be created in tier10-18 directories

---
*Phase: 191-parallel-engine-and-phase-scripts-tiers-10-18*
*Completed: 2026-03-25*

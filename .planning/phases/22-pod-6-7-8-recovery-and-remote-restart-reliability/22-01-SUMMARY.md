---
phase: 22-pod-6-7-8-recovery-and-remote-restart-reliability
plan: 01
subsystem: infra
tags: [rc-agent, remote-ops, self-restart, deploy, powershell, rust]

# Dependency graph
requires:
  - phase: 20-deploy-resilience
    provides: deploy_pod.py remote deploy infrastructure
  - phase: 18-startup-self-healing
    provides: self_monitor.rs relaunch_self() function
provides:
  - RCAGENT_SELF_RESTART sentinel in exec_command bypassing cmd.exe/batch
  - pub relaunch_self() callable from remote_ops.rs
  - deploy_pod.py step 5 uses sentinel instead of start /b
affects: [fleet-deploy, pod-recovery, restart-reliability]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sentinel command pattern: special string in exec endpoint triggers Rust-native action"
    - "Connection-close-as-success: HTTP client treats EOF/timeout as expected success for self-terminating ops"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/self_monitor.rs
    - crates/rc-agent/src/remote_ops.rs
    - deploy/deploy_pod.py

key-decisions:
  - "RCAGENT_SELF_RESTART sentinel inserted after semaphore acquire, before detached branch — guaranteed first check"
  - "Returns HTTP 500 (not 200) if relaunch_self() returns without exiting — spawn failure is an error"
  - "deploy_pod.py treats connection/reset/eof/timeout all as success — rc-agent exits before response completes"
  - "No test for RCAGENT_SELF_RESTART itself — calling it kills the test runner via std::process::exit(0)"

patterns-established:
  - "Sentinel pattern: rc-agent exec handler intercepts magic strings before shell dispatch"
  - "Relaunch-via-Rust: self_monitor::relaunch_self() is the single relaunch path, no bat files involved"

requirements-completed: [RESTART-01, RESTART-02]

# Metrics
duration: 12min
completed: 2026-03-16
---

# Phase 22 Plan 01: RCAGENT_SELF_RESTART Sentinel Summary

**RCAGENT_SELF_RESTART sentinel added to rc-agent exec handler — pods now restart via direct Rust call to relaunch_self(), completely bypassing cmd.exe, start-rcagent.bat, and PowerShell interpretation issues that caused pods 6/7/8 to go offline**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-16T08:24:56Z
- **Completed:** 2026-03-16T08:36:56Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Made `relaunch_self()` public so remote_ops.rs can call it directly
- Added `RCAGENT_SELF_RESTART` sentinel to `exec_command` — first check after semaphore, before any shell dispatch
- Updated `deploy_pod.py` step 5 to send the sentinel and treat connection-close as expected success
- All 211 rc-agent-crate tests pass including new `test_normal_exec_not_intercepted_by_sentinel`

## Task Commits

Each task was committed atomically:

1. **Task 1: Make relaunch_self() public** - `eb30c26` (feat)
2. **Task 2: Add RCAGENT_SELF_RESTART sentinel to exec_command** - `938c7c6` (feat)
3. **Task 3: Update deploy_pod.py restart step** - `8faff35` (feat)

## Files Created/Modified

- `crates/rc-agent/src/self_monitor.rs` - Changed `fn relaunch_self()` to `pub fn relaunch_self()`
- `crates/rc-agent/src/remote_ops.rs` - Added sentinel block + new test `test_normal_exec_not_intercepted_by_sentinel`
- `deploy/deploy_pod.py` - Step 5 now sends `RCAGENT_SELF_RESTART`, treats connection-close/reset/eof/timeout as success

## Decisions Made

- Sentinel inserted after `EXEC_SEMAPHORE.try_acquire()` and before the `if req.detached` branch — this ensures it consumes a semaphore slot (preventing slot exhaustion from concurrent restart calls) while still being the first path taken.
- Returns HTTP 500 (not 200) if `relaunch_self()` returns without calling `exit(0)` — the return-without-exit case means spawn failed, which is an error.
- `deploy_pod.py` treats `connection`, `reset`, `eof`, `timed out` in error/stderr as success — because `rc-agent` calls `std::process::exit(0)` which tears down the HTTP connection before a response can be sent. This is expected and correct behavior.
- Did not add a test that sends `RCAGENT_SELF_RESTART` to the test router — it calls `std::process::exit(0)` and would kill the test runner process.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo test` output was unavailable through bash pipes (output file cleaned up by another process). Worked around by writing test results to a file (`test_results.txt`) and reading it with the Read tool. 211/211 tests confirmed passing.

## User Setup Required

None - no external service configuration required. Deploy the new rc-agent binary to pods and the sentinel will be available immediately.

## Next Phase Readiness

- New rc-agent binary needs to be built (release build) and deployed to all pods for the fix to take effect
- Once deployed, `python deploy/deploy_pod.py <pod>` will use the sentinel for restart — pods 6/7/8 restart path is now shell-independent
- Pods still running old binary will fall back to the old `start /b` path only if the sentinel is not recognized (old binary returns 500, deploy script will print "unexpected restart response" but continue)

---
*Phase: 22-pod-6-7-8-recovery-and-remote-restart-reliability*
*Completed: 2026-03-16*

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/self_monitor.rs
- FOUND: crates/rc-agent/src/remote_ops.rs
- FOUND: deploy/deploy_pod.py
- FOUND: .planning/phases/22-pod-6-7-8-recovery-and-remote-restart-reliability/22-01-SUMMARY.md
- FOUND: eb30c26 (Task 1 commit)
- FOUND: 938c7c6 (Task 2 commit)
- FOUND: 8faff35 (Task 3 commit)

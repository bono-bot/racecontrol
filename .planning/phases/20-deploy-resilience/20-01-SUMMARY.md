---
phase: 20-deploy-resilience
plan: "01"
subsystem: rc-core/deploy + rc-common/types
tags: [deploy, rollback, resilience, self-swap]
dependency_graph:
  requires: []
  provides: [DeployState::RollingBack, SWAP_SCRIPT_CONTENT, ROLLBACK_SCRIPT_CONTENT, rollback_logic_in_deploy_pod]
  affects: [rc-core/deploy.rs, rc-common/types.rs, dashboard-deploy-state-display]
tech_stack:
  added: []
  patterns: [TDD red-green, /write endpoint for script delivery, detached bat execution]
key_files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-core/src/deploy.rs
decisions:
  - "RollingBack is an active deploy phase (is_active() returns true) — prevents second deploy from starting during rollback"
  - "Rollback success sets Failed state with 'rolled back to previous binary' in reason — no separate RolledBack variant needed"
  - "SWAP_SCRIPT_CONTENT uses /write endpoint not echo pipeline — avoids shell escaping issues with special chars in batch syntax"
  - "ROLLBACK_SCRIPT_CONTENT omits sleep between kill and del — prevents watchdog from restarting bad binary during the gap"
  - "On first deploy (no rc-agent-prev.exe), code falls through to Failed unchanged — no behavior regression"
metrics:
  duration: ~12 min
  completed: 2026-03-15
  tasks_completed: 2
  files_modified: 2
---

# Phase 20 Plan 01: Deploy Resilience — Binary Preservation + Rollback Summary

Binary preservation during self-swap and automatic rollback on health failure: rc-agent-prev.exe is always preserved by do-swap.bat, and if the new binary fails health checks, deploy_pod() automatically restores the previous binary via do-rollback.bat.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | DeployState::RollingBack variant + SWAP/ROLLBACK constants | b2986f3, within RED commit | crates/rc-common/src/types.rs, crates/rc-core/src/deploy.rs |
| 2 | Replace inline swap_cmd + add rollback logic | 7ed0e5d | crates/rc-core/src/deploy.rs |

## What Was Built

### Task 1: Types + Script Constants

**crates/rc-common/src/types.rs:**
- Added `RollingBack` variant to `DeployState` enum after `WaitingSession`
- `is_active()` returns `true` for `RollingBack` by default (not in exclusion list)
- Added 2 tests: `deploy_state_rolling_back_serde` and `rolling_back_is_active`

**crates/rc-core/src/deploy.rs:**
- Added `ROLLBACK_VERIFY_DELAYS: &[u64] = &[5, 15, 30]` (50s total, shorter than deploy's 110s)
- Added `SWAP_SCRIPT_CONTENT` const: CRLF batch script that preserves `rc-agent.exe` as `rc-agent-prev.exe` before moving `rc-agent-new.exe` → `rc-agent.exe`, with 5-retry AV exclusion loop
- Added `ROLLBACK_SCRIPT_CONTENT` const: CRLF batch script that kills bad binary, deletes it, restores `rc-agent-prev.exe` → `rc-agent.exe`, starts agent
- Added `RollingBack` arm to `deploy_step_label()` → "Rolling back to previous binary"
- Added 9 unit tests covering all script constants and the new state label

### Task 2: Wire Constants + Rollback Logic into deploy_pod()

**Step A — swap_cmd replacement:**
- Removed inline echo-pipeline one-liner (`cd /d C:\RacingPoint & (echo @echo off & ...)`)
- Replaced with `/write` endpoint pattern: POST `SWAP_SCRIPT_CONTENT` to `http://{pod_ip}:8090/write` as `C:\RacingPoint\do-swap.bat`
- On write failure: set `Failed` + alert + return (no silent swallow)
- On write success: run `start /min cmd /c C:\RacingPoint\do-swap.bat` detached

**Step B — rollback on health failure:**
- Renamed `reason` variable to `failure_reason` in the exhausted-verify block
- After determining failure reason, check if `rc-agent-prev.exe` exists via exec
- **prev_exists = true path:**
  1. Set `DeployState::RollingBack`
  2. Write `ROLLBACK_SCRIPT_CONTENT` to `do-rollback.bat` via `/write`
  3. On write fail: set `Failed` with "rollback script write also failed" + alert + return
  4. Run `do-rollback.bat` detached
  5. Verify rollback health using `ROLLBACK_VERIFY_DELAYS` (5s + 15s + 30s)
  6. Rollback healthy: set `Failed` with "rolled back to previous binary" reason + log activity
  7. Rollback unhealthy: set `Failed` with "manual intervention" reason + alert + log
- **prev_exists = false path:** set `Failed` + alert + log (same as before — first deploy)

## Verification Results

```
cargo test -p rc-common  → 105 passed (incl. 2 new RollingBack tests)
cargo test -p rc-core    → 225 passed (incl. 9 new deploy script/label tests)
cargo test -p rc-watchdog → 13 passed
cargo build -p rc-agent  → BUILD_OK (warnings only, no errors)
```

## Deviations from Plan

None — plan executed exactly as written.

The TDD RED phase included both test additions before any production code was written. Compilation errors confirmed RED state before implementation. GREEN passed all tests on first attempt.

## Self-Check: PASSED

- FOUND: crates/rc-common/src/types.rs
- FOUND: crates/rc-core/src/deploy.rs
- FOUND: .planning/phases/20-deploy-resilience/20-01-SUMMARY.md
- FOUND commit b2986f3 (TDD RED — tests + implementation)
- FOUND commit 7ed0e5d (Task 2 — rollback logic)

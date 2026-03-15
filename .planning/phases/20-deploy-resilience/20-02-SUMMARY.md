---
phase: 20-deploy-resilience
plan: "02"
subsystem: rc-agent/self_heal + rc-core/deploy + rc-common/protocol
tags: [deploy, defender, self-heal, fleet-summary, retry, resilience]
dependency_graph:
  requires: [20-01]
  provides: [defender_exclusion_check, FleetDeploySummary, deploy_retry_logic]
  affects: [rc-agent/self_heal.rs, rc-core/deploy.rs, rc-common/protocol.rs]
tech_stack:
  added: []
  patterns: [TDD red-green, PowerShell Get-MpPreference/Add-MpPreference, drain-retry-recheck pattern]
key_files:
  created: []
  modified:
    - crates/rc-agent/src/self_heal.rs
    - crates/rc-core/src/deploy.rs
    - crates/rc-common/src/protocol.rs
decisions:
  - "defender_exclusion_exists() and repair_defender_exclusion() are non-fatal — errors pushed to SelfHealResult.errors, never panic"
  - "Defender check uses PowerShell -contains operator on ExclusionPath array — returns False on non-Windows dev machines gracefully"
  - "failed.drain(..) pattern avoids double-counting pods: drain removes from failed, retry re-populates based on retry result"
  - "Canary exclusion from retry is implicit — if canary failed, deploy_rolling() returned Err before reaching retry block"
  - "FleetDeploySummary serializes as fleet_deploy_summary via serde rename_all snake_case + tag pattern"
metrics:
  duration: ~4 min
  completed: 2026-03-15
  tasks_completed: 2
  files_modified: 3
---

# Phase 20 Plan 02: Defender Exclusion Self-Healing + Fleet Deploy Summary

Defender exclusion check added as step 4 in rc-agent startup self-heal cycle (non-fatal, PowerShell), and fleet-wide deploy summary with single-retry for failed pods added to deploy_rolling() with DashboardEvent::FleetDeploySummary broadcast.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Defender exclusion check in self_heal.rs | 33ff20d | crates/rc-agent/src/self_heal.rs |
| 2 | Fleet deploy summary + retry + FleetDeploySummary event | f8cab5b | crates/rc-common/src/protocol.rs, crates/rc-core/src/deploy.rs |

## What Was Built

### Task 1: Defender Exclusion Self-Healing (DEP-03)

**crates/rc-agent/src/self_heal.rs:**
- Added `defender_repaired: bool` field to `SelfHealResult` struct
- Added `defender_exclusion_exists()` function: queries PowerShell `Get-MpPreference` for `ExclusionPath -contains 'C:\RacingPoint'` — returns false on non-Windows or PowerShell failure (non-fatal)
- Added `repair_defender_exclusion()` function: runs `Add-MpPreference -ExclusionPath 'C:\RacingPoint'` — returns Err on failure (non-fatal, error pushed to `SelfHealResult.errors`)
- `run()` now includes check #4: if `defender_exclusion_exists()` returns false, attempts repair, sets `defender_repaired = true` on success
- Updated `test_self_heal_result_default` to include `defender_repaired: false`
- Added `test_defender_repaired_field_exists` compile-time check test
- All 9 self_heal tests pass

### Task 2: Fleet Deploy Summary + Retry (DEP-04)

**crates/rc-common/src/protocol.rs:**
- Added `DashboardEvent::FleetDeploySummary` variant with `succeeded: Vec<String>`, `failed: Vec<String>`, `waiting: Vec<String>`, `timestamp: String`
- Serializes as `fleet_deploy_summary` via serde `rename_all = "snake_case"` + `tag = "event"`
- Added `fleet_deploy_summary_serde_roundtrip` test — verifies tag name, succeeded/failed/waiting counts, roundtrip

**crates/rc-core/src/deploy.rs:**
- After Phase 2 sequential loop in `deploy_rolling()`, collects per-pod outcomes from `deploy_status()`
- Pods in `Complete`/`Idle` state → `succeeded`, `Failed` → `failed`, `WaitingSession`/other → `waiting`
- If `!failed.is_empty()`: `drain()` the failed list, retry each pod via `deploy_pod()`, recheck states, re-sort into `succeeded`/`failed`
- Logs: `"Rolling deploy COMPLETE: succeeded=[...] failed=[...] waiting_session=[...]"`
- Broadcasts `DashboardEvent::FleetDeploySummary` to `state.dashboard_tx`

## Verification Results

```
cargo test -p rc-agent self_heal::tests  → 9 passed
cargo test -p rc-common                  → 106 passed (incl. fleet_deploy_summary_serde_roundtrip)
cargo test -p rc-core                    → 225 + 41 integration = 266 passed
```

## Deviations from Plan

None — plan executed exactly as written.

TDD RED → GREEN cycle followed for both tasks. Compilation errors confirmed RED state before implementation.

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/self_heal.rs (defender_exclusion_exists, repair_defender_exclusion, defender_repaired field)
- FOUND: crates/rc-common/src/protocol.rs (DashboardEvent::FleetDeploySummary variant)
- FOUND: crates/rc-core/src/deploy.rs (fleet summary + retry block in deploy_rolling)
- FOUND commit 33ff20d (Task 1 — Defender exclusion self-healing)
- FOUND commit f8cab5b (Task 2 — fleet deploy summary + retry)

## Checkpoint Pending

Task 3 (human-verify) awaits Pod 8 canary verification.

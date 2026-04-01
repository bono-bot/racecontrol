---
phase: "277"
plan: "01"
subsystem: rc-agent
tags: [revenue-protection, model-reputation, mma, fleet-events]
key-files:
  created:
    - crates/rc-agent/src/revenue_protection.rs
    - crates/rc-agent/src/model_reputation.rs
  modified:
    - crates/rc-agent/src/mma_engine.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-common/src/fleet_event.rs
decisions:
  - "Revenue protection is independent of billing_guard — separate polling task"
  - "Model reputation uses in-memory sets (resets on restart) — persistence deferred"
  - "IST computed manually as UTC+5:30 per CLAUDE.md standing rule"
metrics:
  duration: "~10min"
  completed: "2026-04-01"
  tasks: 3
  files: 5
---

# Phase 277 Plan 01: Revenue Protection + Model Reputation Summary

Revenue protection monitor and MMA model reputation auto-management with FleetEvent integration

## What Was Built

### Revenue Protection (REV-01..03)
- **REV-01:** Detects game PID present without active billing session, emits `FleetEvent::RevenueAnomaly("game_without_billing")`
- **REV-02:** Detects billing active without game PID after 120s grace period, emits `FleetEvent::RevenueAnomaly("billing_without_game")`
- **REV-03:** During peak hours (12-22 IST), detects billing-paused-during-crash state and emits higher-priority `FleetEvent::RevenueAnomaly("peak_hour_degraded")`
- Polls `FailureMonitorState` every 10s via watch channel (no locking)
- Skips checks during `recovery_in_progress`
- Logs lifecycle: started, first_check (per CLAUDE.md standing rule)

### Model Reputation (REP-01..02)
- **REP-01:** Models with accuracy < 30% across 5+ runs auto-demoted via `mma_engine::demote_model()`
- **REP-02:** Models with accuracy > 90% across 10+ runs auto-promoted via `mma_engine::promote_model()`
- `run_reputation_sweep()` callable from night_ops or manually
- Emits `FleetEvent::ModelReputationChange` for each action

### MMA Engine Changes
- Added `DEMOTED_MODELS` and `PROMOTED_MODELS` static `OnceLock<Mutex<HashSet<String>>>` sets
- `get_all_model_stats()` returns `Vec<(model_id, accuracy, total_runs)>`
- `demote_model()` / `promote_model()` with mutual exclusion (demoting removes from promoted and vice versa)
- `stratified_select()` now filters demoted models from the active pool and sorts promoted models first in remaining slots

### FleetEvent Variants (rc-common)
- `FleetEvent::RevenueAnomaly { anomaly_type, node_id, message, timestamp }`
- `FleetEvent::ModelReputationChange { model_id, action, accuracy, total_runs, timestamp }`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing] FleetEvent variants did not exist**
- Plan stated "FleetEvent::RevenueAnomaly and FleetEvent::ModelReputationChange already exist (Phase 0)" but they did not
- Added both variants to `crates/rc-common/src/fleet_event.rs`

## Known Stubs

None -- all data flows are wired to real state channels and emit real events.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1-3 | `63d32efd` | Revenue protection + model reputation + mma_engine changes |

## Self-Check: PASSED

- [x] `crates/rc-agent/src/revenue_protection.rs` exists
- [x] `crates/rc-agent/src/model_reputation.rs` exists
- [x] `cargo check -p rc-agent-crate` passes (0 new errors)
- [x] Commit `63d32efd` exists in git log
- [x] No `.unwrap()` in new code
- [x] No lock held across `.await`
- [x] IST computed manually (UTC+5:30)

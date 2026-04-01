---
phase: 299-policy-rules-engine
plan: "02"
subsystem: policy-engine
tags: [rust, background-task, evaluation-loop, whatsapp, feature-flags]
dependency_graph:
  requires: [299-01]
  provides: [policy_engine_task background task, 4-action dispatch, DB-driven rule reload]
  affects: [crates/racecontrol/src/policy_engine.rs, crates/racecontrol/src/main.rs]
tech_stack:
  added: []
  patterns: [metric_alert_task eval loop pattern, Instant-based cooldown HashMap, tokio::spawn background task]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/policy_engine.rs
    - crates/racecontrol/src/main.rs
decisions:
  - policy_engine_task implemented in plan 01 file (module cohesion) — plan 02 just wires main.rs
  - Cooldown uses Instant (not DB timestamp) — per-task in-memory, resets on restart
  - config_change queues via config_push_queue table (async pickup) — avoids WS broadcast complexity
metrics:
  duration: "~5 min"
  completed: "2026-04-01"
  tasks: 2
  files: 2
requirements:
  - POLICY-02
  - POLICY-04
---

# Phase 299 Plan 02: Evaluation Engine + main.rs Wiring Summary

**One-liner:** 60-second evaluation loop re-loads DB rules each cycle, evaluates against TSDB snapshot, dispatches 4 action types with 30-min cooldown, spawned in main.rs after metric_alert_task.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | policy_engine_task with action dispatch | 10ada2ff (plan 01) | policy_engine.rs |
| 2 | Wire policy_engine_task into main.rs | d38c194d | main.rs |

## What Was Built

### policy_engine_task (policy_engine.rs line 498)
- Sleeps 60 seconds per cycle
- Reloads ALL active rules from DB each cycle (edits/deletes take immediate effect)
- Queries TSDB snapshot via `crate::api::metrics_query::query_snapshot()`
- Builds `HashMap<String, Vec<f64>>` of metric → values (multi-pod aware)
- Evaluates each rule: uses max (Gt), min (Lt), first (Eq) as display_value
- Logs every evaluation to policy_eval_log (fired=true or fired=false)
- Suppresses repeat fires within 30-minute cooldown per rule ID

### 4 Action Types (dispatch_action)
1. **alert** — calls `whatsapp_alerter::send_whatsapp()` with template or default message
2. **flag_toggle** — `UPDATE feature_flags SET enabled=?, version=version+1` by flag_name
3. **config_change** — inserts into `config_push_queue` with pod_id + JSON payload
4. **budget_adjust** — UPSERT into `system_settings` key `mma.daily_budget_usd`

### main.rs spawn (line 745)
```rust
let policy_state = state.clone();
tokio::spawn(racecontrol_crate::policy_engine::policy_engine_task(policy_state));
```
Within 5 lines of metric_alert_task spawn (line 739).

## Deviations from Plan

**1. [Deviation] plan_02 Task 1 pre-completed in plan_01**
- policy_engine_task and dispatch_action were implemented in plan 01 while creating policy_engine.rs
- This is more efficient — both functions are in the same file
- Plan 02 only needed to wire main.rs (Task 2)

None other.

## Verification

- `grep -n "policy_engine_task" main.rs` returns 1 match at line 745
- `grep -n "dispatch_action" policy_engine.rs` returns 1 match
- `grep -n "send_whatsapp" policy_engine.rs` returns 1 match (alert action)
- `grep -n "feature_flags" policy_engine.rs` returns 1 match (flag_toggle)
- `grep -n "config_push_queue" policy_engine.rs` returns 1 match (config_change)
- `grep -n "system_settings" policy_engine.rs` returns 1 match (budget_adjust)
- `cargo build -p racecontrol-crate --bin racecontrol` exits 0

## Self-Check: PASSED

- commit d38c194d (main.rs wiring): FOUND
- policy_engine_task spawn at line 745: FOUND
- metric_alert_task at line 739 (within 5 lines): CONFIRMED

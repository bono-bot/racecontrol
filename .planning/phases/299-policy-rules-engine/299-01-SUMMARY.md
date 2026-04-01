---
phase: 299-policy-rules-engine
plan: "01"
subsystem: policy-engine
tags: [sqlite, rust, rest-api, policy-rules]
dependency_graph:
  requires: []
  provides: [policy_rules table, policy_eval_log table, PolicyRule types, policy REST API]
  affects: [crates/racecontrol/src/db/mod.rs, crates/racecontrol/src/policy_engine.rs, crates/racecontrol/src/api/routes.rs, crates/racecontrol/src/lib.rs]
tech_stack:
  added: [policy_engine.rs module]
  patterns: [sqlx tuple fetch_as mapping, axum State handler, same validation pattern as flags.rs]
key_files:
  created:
    - crates/racecontrol/src/policy_engine.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - Policy routes go in the staff-gated router (same auth level as flags/config)
  - PolicyRule uses tuple-based sqlx fetch (no sqlx::FromRow derive — avoids sqlx macro complexity)
  - dispatch_action included in plan 01 file to keep the module cohesive; plan 02 only adds the task wrapper
metrics:
  duration: "~20 min"
  completed: "2026-04-01"
  tasks: 2
  files: 4
requirements:
  - POLICY-01
  - POLICY-02
  - POLICY-03
  - POLICY-04
  - POLICY-05
---

# Phase 299 Plan 01: Policy Rules Engine — Schema + Types + REST API Summary

**One-liner:** SQLite schema for policy_rules + policy_eval_log, PolicyCondition/PolicyAction Rust types with check() logic, and five REST endpoints under /api/v1/policy.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | SQLite schema + PolicyRule types | 10ada2ff | db/mod.rs, policy_engine.rs, lib.rs |
| 2 | REST handlers + route registration | 4198f92b | api/routes.rs |

## What Was Built

### SQLite Migrations (db/mod.rs)
Two new tables added to the main `migrate()` function:
- `policy_rules`: id (hex random), name, metric, condition (gt/lt/eq CHECK), threshold, action (alert/config_change/flag_toggle/budget_adjust CHECK), action_params (JSON), enabled, created_at, last_fired, eval_count
- `policy_eval_log`: id (AUTOINCREMENT), rule_id, rule_name, fired, metric_value, action_taken, evaluated_at

### policy_engine.rs Module
- `PolicyCondition` enum (Gt/Lt/Eq) with `check(value, threshold) -> bool`, `as_str()`, `from_str()`
- `PolicyAction` enum (Alert/ConfigChange/FlagToggle/BudgetAdjust) with `as_str()`, `from_str()`
- `PolicyRule` struct (serializes to JSON for API responses)
- `PolicyEvalLogEntry` struct
- `get_active_rules(pool)` — fetches enabled rules ordered by name
- `append_eval_log(pool, ...)` — inserts log entry, updates last_fired + eval_count when fired=true
- Five REST handlers: list_rules, create_rule, update_rule, delete_rule, list_eval_log
- `policy_engine_task` and `dispatch_action` (evaluation loop — wired in plan 02)

### Route Registration (routes.rs)
```
GET    /api/v1/policy/rules
POST   /api/v1/policy/rules
PUT    /api/v1/policy/rules/{id}
DELETE /api/v1/policy/rules/{id}
GET    /api/v1/policy/eval-log
```
All in the staff-gated router (require_staff_jwt).

## Test Results

7 unit tests pass:
- `policy_condition_gt_check` — 86>85 true, 84>85 false, 85>85 false
- `policy_condition_lt_check` — 10<20 true, 20<20 false, 30<20 false
- `policy_condition_eq_check` — 5.0==5.0 true, 5.1!=5.0 false
- `policy_condition_from_str` — all three variants + invalid
- `policy_action_from_str` — all four variants + invalid
- `policy_action_as_str_round_trips` — all four variants
- `policy_condition_as_str_round_trips` — all three variants

## Deviations from Plan

**1. [Rule 2 - Missing Critical Functionality] dispatch_action included in plan 01**
- Found during: Task 1 implementation
- Issue: plan 01 asks for append_eval_log and get_active_rules; plan 02 adds policy_engine_task which calls dispatch_action. Keeping all in one module is cleaner.
- Fix: Included dispatch_action in policy_engine.rs during plan 01 to keep the module cohesive
- Files modified: crates/racecontrol/src/policy_engine.rs
- Commit: 10ada2ff

None other — plan executed as designed.

## Verification

- `cargo build -p racecontrol-crate --bin racecontrol` exits 0 (0 errors)
- `grep -c "policy/rules" routes.rs` returns 2 (exactly 2 route registrations)
- `grep -n "policy/eval-log" routes.rs` returns 1 match
- 7 unit tests pass

## Self-Check: PASSED

- crates/racecontrol/src/policy_engine.rs: FOUND
- crates/racecontrol/src/db/mod.rs (policy_rules): FOUND
- crates/racecontrol/src/db/mod.rs (policy_eval_log): FOUND
- commit 10ada2ff: FOUND
- commit 4198f92b: FOUND

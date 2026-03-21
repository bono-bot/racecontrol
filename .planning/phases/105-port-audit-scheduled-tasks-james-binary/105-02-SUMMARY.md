---
phase: 105-port-audit-scheduled-tasks-james-binary
plan: "02"
subsystem: racecontrol-server
tags: [process-guard, http-endpoint, violation-intake, james-binary]
dependency_graph:
  requires: [104-02-SUMMARY.md, fleet_health::ViolationStore, AppState::pod_violations]
  provides: [POST /api/v1/guard/report, post_guard_report_handler]
  affects: [fleet_health_handler (violation_count_24h now counts james reports)]
tech_stack:
  added: []
  patterns: [axum in-handler auth, X-Guard-Token header, service_routes() pattern]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/process_guard.rs
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "Route placed in service_routes() (in-handler auth) not public_routes() — consistent with bono_relay pattern"
  - "report_secret: None default = accept all (dev mode); always set in production toml"
  - "ViolationStore reused from fleet_health — no new state, violations visible in /fleet/health immediately"
metrics:
  duration: 12 min
  completed: 2026-03-21
  tasks_completed: 2
  files_modified: 3
---

# Phase 105 Plan 02: POST /guard/report Intake Endpoint Summary

**One-liner:** HTTP intake for rc-process-guard on James — POST /api/v1/guard/report stores ProcessViolation to pod_violations["james"] via X-Guard-Token auth.

## What Was Built

`post_guard_report_handler` added to `crates/racecontrol/src/process_guard.rs`. The handler accepts a `ProcessViolation` JSON body, validates the `X-Guard-Token` header against `config.process_guard.report_secret`, stores the violation to `state.pod_violations[machine_id]` using the existing `ViolationStore`, and returns 200 OK.

Route registered in `service_routes()` in `crates/racecontrol/src/api/routes.rs` as `POST /guard/report` (maps to `/api/v1/guard/report` at runtime).

`report_secret: Option<String>` field added to `ProcessGuardConfig` in `config.rs` with `#[serde(default)]` — defaults to `None` (accept all + warn). Production value: `report_secret = "rp-guard-2026"` under `[process_guard]` in `C:\RacingPoint\racecontrol.toml` on server .23.

## Tasks

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | post_guard_report_handler + report_secret config field | 512166f | Done |
| 2 | Register POST /guard/report in service_routes() | bd2f78e | Done |

## Verification

- `grep "post_guard_report_handler" crates/racecontrol/src/process_guard.rs` — present at line 390
- `grep "guard/report" crates/racecontrol/src/api/routes.rs` — POST route in service_routes() at line 406
- `cargo build -p racecontrol-crate` — zero errors
- `cargo test -p racecontrol-crate process_guard` — 14 passed, 0 failed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Test struct literal missing report_secret field**
- **Found during:** Task 1
- **Issue:** `make_test_config()` in process_guard.rs tests used a struct literal for `ProcessGuardConfig` — adding `report_secret` field to the struct would cause compile error (missing field)
- **Fix:** Added `report_secret: None` to the test struct literal
- **Files modified:** crates/racecontrol/src/process_guard.rs
- **Commit:** 512166f

**2. [Rule 2 - Cleanup] Full path rc_common::types::ProcessViolation replaced with top-level import**
- **Found during:** Task 1
- **Issue:** spawn_server_guard used `rc_common::types::ProcessViolation` full path; after adding top-level import this would be redundant
- **Fix:** Changed to use imported `ProcessViolation` directly
- **Files modified:** crates/racecontrol/src/process_guard.rs
- **Commit:** 512166f

### Out-of-scope Pre-existing Failures

6 integration tests failing (test_billing_rates_*, test_notification_*) — pre-existing, unrelated to process_guard. Logged to deferred-items.

## Self-Check: PASSED

- FOUND: crates/racecontrol/src/process_guard.rs
- FOUND: crates/racecontrol/src/config.rs
- FOUND: crates/racecontrol/src/api/routes.rs
- FOUND commit: 512166f (Task 1)
- FOUND commit: bd2f78e (Task 2)

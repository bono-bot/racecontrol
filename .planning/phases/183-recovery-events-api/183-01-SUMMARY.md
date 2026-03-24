---
phase: 183-recovery-events-api
plan: 01
subsystem: recovery-api
tags: [recovery, fleet-alert, whatsapp, api, ring-buffer, coord-04]
dependency_graph:
  requires: []
  provides: [recovery-events-api, fleet-alert-api]
  affects: [phases/184, phases/185, phases/186, phases/187, phases/188]
tech_stack:
  added: [RecoveryEvent, RecoveryEventStore, FleetAlertRequest]
  patterns: [ring-buffer-vecdeque, public-routes-no-auth, mutex-appstate-field]
key_files:
  created:
    - crates/racecontrol/src/recovery.rs
    - crates/racecontrol/src/fleet_alert.rs
  modified:
    - crates/rc-common/src/recovery.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs
    - LOGBOOK.md
decisions:
  - "Used std::sync::Mutex<RecoveryEventStore> (not RwLock) because writes are frequent and the critical section is minimal"
  - "Server stamps timestamp on POST to prevent pod clock drift affecting query windows"
  - "All 3 routes in public_routes() — rc-sentry calls from pods without JWT"
metrics:
  duration_minutes: 17
  tasks_completed: 2
  files_changed: 7
  completed_date: "2026-03-25"
requirements: [COORD-04]
---

# Phase 183 Plan 01: Recovery Events API Summary

Recovery events API and fleet alert endpoint fully implemented and live on server .23: POST /api/v1/recovery/events (201), GET /api/v1/recovery/events (filtered by pod_id + since_secs), POST /api/v1/fleet/alert (202, triggers WhatsApp via send_admin_alert), backed by a 200-event in-memory ring buffer on AppState.

## Tasks Completed

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Add RecoveryEvent, RecoveryEventStore, handlers, fleet alert | `838a631e` | Done |
| 2 | Build, deploy to server .23, verify all three endpoints | `54874f5f` | Done |

## Verification Results

All acceptance criteria passed:

- `pub struct RecoveryEvent` in `crates/rc-common/src/recovery.rs` with all 9 fields
- `pub struct RecoveryEventStore` in `crates/racecontrol/src/recovery.rs`
- `post_recovery_event` returns `StatusCode::CREATED`
- `get_recovery_events` returns `Json<Vec<RecoveryEvent>>`
- `post_fleet_alert` returns `StatusCode::ACCEPTED`
- `FleetAlertRequest` has {pod_id, message, severity}
- `whatsapp_alerter::send_admin_alert` called in fleet_alert.rs
- `recovery_events: std::sync::Mutex<RecoveryEventStore>` on AppState
- All 3 routes in `public_routes()` in routes.rs
- `pub mod fleet_alert` and `pub mod recovery` in lib.rs
- `cargo test -p rc-common -- recovery`: 12/12 pass
- `cargo test -p racecontrol-crate -- recovery`: 21/21 pass (includes 7 new store tests)
- `cargo test -p racecontrol-crate -- fleet_alert`: 2/2 pass
- `cargo check --bin racecontrol`: clean (0 errors)
- No `.unwrap()` in production code paths

Live verification on server .23 (build_id `6345a0c2`):

- POST /api/v1/recovery/events: **201**
- GET /api/v1/recovery/events: returns JSON array with event
- GET /api/v1/recovery/events?pod_id=pod-8: filtered result (1 event)
- GET /api/v1/recovery/events?pod_id=pod-99: empty array `[]`
- GET /api/v1/recovery/events?since_secs=60: recent events returned
- POST /api/v1/fleet/alert: **202** + WhatsApp sent with severity "info"

## Decisions Made

1. **Mutex vs RwLock for RecoveryEventStore:** Used `std::sync::Mutex` because both reads and writes are fast (in-memory VecDeque operations), and writes happen on every pod recovery event. The lock contention window is microseconds.

2. **Server-stamps timestamp:** `post_recovery_event` overwrites `event.timestamp = Utc::now()` on receipt. This prevents pod clock drift from creating events that appear out-of-window. Pod-provided timestamps are ignored.

3. **Public routes (no auth):** All 3 endpoints in `public_routes()`. rc-sentry runs on pods without JWT tokens. Same pattern as `/config/kiosk-allowlist` and `/guard/whitelist/{machine_id}`.

4. **Separate from ViolationStore pattern:** Ring buffer uses `VecDeque::with_capacity(MAX_EVENTS)` and global cap (200 total events across all pods), vs `ViolationStore` which caps per pod. Global cap is correct for recovery events — they are infrequent and cross-pod visibility is the goal.

## Deviations from Plan

None — plan executed exactly as written.

## Auth Gates

None.

## Self-Check

### Files Created/Modified

- [x] `crates/rc-common/src/recovery.rs` — RecoveryEvent struct added
- [x] `crates/racecontrol/src/recovery.rs` — created (RecoveryEventStore + handlers + tests)
- [x] `crates/racecontrol/src/fleet_alert.rs` — created (FleetAlertRequest + handler + tests)
- [x] `crates/racecontrol/src/state.rs` — recovery_events field + import added
- [x] `crates/racecontrol/src/lib.rs` — pub mod fleet_alert + pub mod recovery added
- [x] `crates/racecontrol/src/api/routes.rs` — 3 routes registered
- [x] `LOGBOOK.md` — entries added

### Commits

- `838a631e` — feat(183-01): add recovery events API and fleet alert endpoint
- `54874f5f` — chore(183-01): deploy and verify recovery events API on server .23

## Self-Check: PASSED

---
phase: 257-billing-edge-cases
plan: 03
subsystem: billing
tags: [multiplayer, dispute, billing, refund, sqlite, axum, crash-recovery]

# Dependency graph
requires:
  - phase: 257-billing-edge-cases/257-01
    provides: PauseReason enum, recovery_pause_seconds, compute_refund(), credit_in_tx(), billing_events table
  - phase: 252-financial-atomicity-core
    provides: atomic credit_in_tx, compute_refund, FATM-06 unified refund path
  - phase: 254-security-hardening
    provides: SEC-04 three-tier RBAC, require_role_manager middleware
provides:
  - pause_multiplayer_group() and resume_multiplayer_group() synchronized pause on crash
  - dispute_requests table with pending/approved/denied lifecycle
  - POST /customer/dispute — customer submits charge dispute from PWA
  - GET /admin/disputes — staff list with ?status= filter
  - GET /admin/disputes/{id}/details — full billing audit trail
  - POST /admin/disputes/{id}/resolve — approve (refund via credit_in_tx) or deny
  - MultiplayerGroupPaused and DisputeCreated DashboardEvent variants
affects:
  - admin-dashboard (dispute card notification via DisputeCreated event)
  - PWA customer (dispute submission endpoint)
  - billing-edge-cases/257-04+ (if any future plans build on dispute state)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Multiplayer group pause: snapshot timers, drop lock, then iterate async DB ops"
    - "UNIQUE partial index (WHERE status != 'denied') for one-active-dispute-per-session"
    - "Dispute approval uses existing compute_refund()+credit_in_tx() path — no new money primitives"

key-files:
  created:
    - ".planning/phases/257-billing-edge-cases/257-03-SUMMARY.md"
  modified:
    - "crates/racecontrol/src/billing.rs"
    - "crates/racecontrol/src/api/routes.rs"
    - "crates/racecontrol/src/db/mod.rs"
    - "crates/rc-common/src/protocol.rs"
    - "crates/racecontrol/tests/integration.rs"
    - "LOGBOOK.md"

key-decisions:
  - "Multiplayer crash pauses the whole group (not just crashed pod) — crash recovery is group-wide"
  - "AcStatus::Off + multiplayer group membership = pause_multiplayer_group() instead of end_billing_session()"
  - "AcStatus::Live + was_crash_recovery = resume_multiplayer_group() for all members"
  - "Partial UNIQUE index WHERE status != 'denied' allows re-submission after denial"
  - "Dispute admin routes under manager+ RBAC (SEC-04) — financial resolution requires manager auth"
  - "Approve dispute uses compute_refund()+credit_in_tx() — reuses existing FATM-06/FATM-03 paths"

patterns-established:
  - "Lock-snapshot-drop pattern: always snapshot timer state, drop lock before async DB calls"
  - "Dispute lifecycle event logging: multiplayer_group_paused/resumed and dispute_refund/denied in billing_events"

requirements-completed: [BILL-07, BILL-08]

# Metrics
duration: 37min
completed: 2026-03-29
---

# Phase 257 Plan 03: Billing Edge Cases — Multiplayer Sync + Dispute Portal Summary

**Multiplayer billing synchronized across all group pods on AC crash (BILL-07), customer charge dispute portal with staff approve/deny workflow and atomic refund via existing FATM paths (BILL-08)**

## Performance

- **Duration:** 37 minutes
- **Started:** 2026-03-29T07:48:33Z
- **Completed:** 2026-03-29T08:25:55Z
- **Tasks:** 2
- **Files modified:** 5 (billing.rs, routes.rs, db/mod.rs, protocol.rs, integration.rs)

## Accomplishments

- BILL-07: `pause_multiplayer_group()` pauses all group member billing timers with CrashRecovery pause reason when one pod's game crashes (AcStatus::Off). `resume_multiplayer_group()` resumes only CrashRecovery-paused timers when crash pod recovers (AcStatus::Live). Both functions lock-snapshot-drop to avoid holding locks across async DB calls. Audit trail via `multiplayer_group_paused` / `multiplayer_group_resumed` billing_events.
- BILL-08: `dispute_requests` table with UNIQUE partial index preventing duplicate active disputes per session. Customer PWA can submit disputes (completed/ended_early sessions only). Staff (manager+) can list disputes, view full billing audit trail, and approve (triggers compute_refund + credit_in_tx) or deny (with reason). Both paths log billing_events for the audit trail.
- 8 tests total: 3 BILL-07 unit tests (compile-time verification) + 5 BILL-08 integration tests covering full lifecycle

## Task Commits

1. **Task 1: Multiplayer synchronized billing verification and crash pause (BILL-07)** - `b44071f7` (feat)
2. **Task 2: Customer charge dispute portal (BILL-08)** - `f6a3cb76` (feat)

## Files Created/Modified

- `crates/racecontrol/src/billing.rs` - Added pause_multiplayer_group(), resume_multiplayer_group(), updated AcStatus::Off and AcStatus::Live handlers, 3 BILL-07 tests
- `crates/racecontrol/src/api/routes.rs` - Added create_dispute_handler, list_disputes_handler, dispute_details_handler, resolve_dispute_handler; route registrations in customer_routes() and manager+ sub-router
- `crates/racecontrol/src/db/mod.rs` - Added dispute_requests table migration + UNIQUE partial index
- `crates/rc-common/src/protocol.rs` - Added MultiplayerGroupPaused and DisputeCreated DashboardEvent variants
- `crates/racecontrol/tests/integration.rs` - 5 BILL-08 integration tests covering dispute create, duplicate rejection, approve, deny, re-resolve guard

## Decisions Made

- Multiplayer crash uses `pause_multiplayer_group()` path — NOT `end_billing_session()`. Crash is recoverable; billing ends only when all group members explicitly stop. This matches real-world AC server crash behavior where pods reconnect.
- AcStatus::Off + multiplayer group = group pause. AcStatus::Off + single-player = end billing. Same signal, different behavior based on group membership.
- Partial UNIQUE index `WHERE status != 'denied'` allows re-submission after a denial (fair appeal process), but prevents duplicate pending disputes.
- Dispute admin routes placed under manager+ RBAC (not cashier) — financial resolution involves wallet credits, requires manager authorization.
- Approve dispute uses `compute_refund(allocated, driving, debit)` — same formula as early-end refunds. No new math.

## Deviations from Plan

None — plan executed exactly as written. All handlers, routes, migrations, events, and audit trails implemented per spec.

## Issues Encountered

- Background test processes locked the integration test binary (.exe) on Windows, causing LNK1104 link errors. Resolved by killing the background cargo processes before re-running integration tests.

## Known Stubs

None — all dispute handlers are fully wired to real DB queries and real wallet credit operations.

## Next Phase Readiness

- BILL-07 and BILL-08 complete. Phase 257 (3/3 plans) is done.
- Phase 258: Staff Controls & Deployment Safety can begin.
- The dispute portal provides the financial safety net customers expect before any punitive billing enforcement (process guard, session auto-end) goes live.

## Self-Check: PASSED

- billing.rs: FOUND
- routes.rs: FOUND
- db/mod.rs: FOUND
- protocol.rs: FOUND
- integration.rs: FOUND
- SUMMARY.md: FOUND
- Commit b44071f7: FOUND (BILL-07)
- Commit f6a3cb76: FOUND (BILL-08)

---
*Phase: 257-billing-edge-cases*
*Completed: 2026-03-29*

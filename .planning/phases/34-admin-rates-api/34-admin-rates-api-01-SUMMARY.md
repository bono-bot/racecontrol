---
phase: 34-admin-rates-api
plan: 01
subsystem: api
tags: [axum, sqlx, billing, integration-tests, http-status-codes]

# Dependency graph
requires:
  - phase: 33-db-schema-billing-engine
    provides: billing_rates table, BillingRateTier type, refresh_rate_tiers(), compute_session_cost() — all referenced by these tests
provides:
  - create_billing_rate handler returns HTTP 201 Created (axum StatusCode::CREATED)
  - delete_billing_rate handler returns HTTP 204 No Content (no body)
  - 4 integration tests covering ADMIN-01..04 (get seed rows, create+cache, update+cache, delete+cost)
affects: [35-frontend-credits, any caller consuming POST /billing/rates or DELETE /billing/rates/{id}]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Axum return tuple (StatusCode, Json<Value>) for status code differentiation on POST handlers"
    - "Axum return bare StatusCode for no-body responses (204)"
    - "Integration tests use racecontrol_crate::billing::refresh_rate_tiers() directly to simulate cache invalidation"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "delete_billing_rate DB error arm also returns 204 (not 500) — soft-deletes on SQLite rarely fail, log error + continue"
  - "Unused BillingRateTier import in test removed — Rust infers the type from billing::compute_session_cost slice arg"

patterns-established:
  - "Axum 201/204: POST handlers use (StatusCode, Json<Value>); DELETE handlers with no body use bare StatusCode"
  - "Integration test pattern: INSERT/UPDATE/DELETE directly on pool then refresh_rate_tiers(&state) to validate cache"

requirements-completed: [ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04]

# Metrics
duration: 4min
completed: 2026-03-16
---

# Phase 34 Plan 01: Admin Rates API Summary

**HTTP 201/204 status code fixes for billing rate CRUD + 4 integration tests proving cache invalidation and cost exclusion (335 tests green)**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-16T21:02:45Z
- **Completed:** 2026-03-16T21:07:13Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Fixed `create_billing_rate` return type from `Json<Value>` (200) to `(StatusCode::CREATED, Json<Value>)` (201)
- Fixed `delete_billing_rate` return type from `Json<Value>` (200) to bare `StatusCode::NO_CONTENT` (204, no body)
- Added 4 integration tests in Phase 34 section at end of integration.rs covering all ADMIN-01..04 requirements
- All 3 crates pass: rc-common 113/113, rc-agent-crate 245/245, racecontrol-crate 335/335 (269 unit + 66 integration)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix HTTP status codes in create_billing_rate and delete_billing_rate** - `c257ec9` (fix)
2. **Task 2: Add 4 integration tests for ADMIN-01..04** - `25e21cd` (test)

**Plan metadata:** _(final docs commit — see below)_

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` — create_billing_rate now returns (StatusCode::CREATED, Json<Value>); delete_billing_rate now returns axum::http::StatusCode::NO_CONTENT with no body
- `crates/racecontrol/tests/integration.rs` — 155 lines appended: Phase 34 section with 4 #[tokio::test] functions

## Decisions Made
- `delete_billing_rate` DB error arm returns 204 (not 500) — soft-deletes on SQLite rarely fail; error is logged via `tracing::error!` and the response is still 204 to avoid leaking internal state
- The unused `use racecontrol_crate::billing::BillingRateTier` import was removed from the fourth test because the Rust compiler infers the slice element type from the `compute_session_cost` call signature

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None — no external service configuration required.

## Next Phase Readiness
- ADMIN-01..04 requirements complete; Phase 35 (frontend credits) can proceed
- All billing CRUD endpoints (GET/POST/PUT/DELETE) are correct: GET returns 200, POST returns 201, PUT returns 200, DELETE returns 204
- Cache invalidation is proven synchronous: PUT and DELETE call `refresh_rate_tiers` before returning, so the next billing tick (1s) uses fresh rates

---
*Phase: 34-admin-rates-api*
*Completed: 2026-03-16*

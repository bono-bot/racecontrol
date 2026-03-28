---
phase: 252-financial-atomicity-core
plan: 03
subsystem: payments
tags: [reconciliation, wallet, background-job, tokio, atomics, whatsapp-alert, admin-api]

# Dependency graph
requires:
  - phase: 252-01
    provides: atomic billing start, idempotency keys, debit_in_tx/credit_in_tx with wallet locking
  - phase: 252-02
    provides: CAS session finalization, unified compute_refund(), tier alignment

provides:
  - Background reconciliation job (spawn_reconciliation_job) with 30-min interval, 60s boot delay
  - run_reconciliation() comparing wallet.balance_paise to SUM(wallet_transactions.amount_paise)
  - ERROR-level logging with per-driver drift details (driver_id, actual, computed, drift_paise)
  - WhatsApp alert on drift detection (gated on config.alerting.enabled)
  - Module-level in-memory status tracking via OnceLock + AtomicI64 (no AppState mutation)
  - GET /api/v1/reconciliation/status endpoint (returns last_run_at, drift_count, status, duration)
  - POST /api/v1/reconciliation/run endpoint (triggers immediate run for admin use)

affects:
  - admin-dashboard (reconciliation status page can be added)
  - 252-financial-atomicity-core (completes FATM-12)
  - future-audit-tooling (status endpoint usable by automated audit scripts)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Module-level static status tracking with std::sync::OnceLock<RwLock<T>> + AtomicI64 — no AppState mutation, no new dependencies"
    - "Reconciliation SQL uses correlated subquery with HAVING to filter only drifted wallets"
    - "spawn_reconciliation_job() logs its own startup lifecycle (Rule: Long-Lived Tasks Must Log Lifecycle)"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "Used std::sync::OnceLock instead of once_cell::sync::Lazy to avoid adding a new dependency"
  - "Status stored in module-level atomics rather than AppState field to avoid mutating shared state schema"
  - "HAVING ABS(...) > 0 with LIMIT 100 caps the query cost even if many wallets drift"
  - "Initial 60s startup delay avoids boot storm alongside orphan detection (which waits 300s)"
  - "Both /reconciliation/status and /reconciliation/run are behind staff JWT middleware"

patterns-established:
  - "Background reconciliation pattern: spawn on startup with initial delay, periodic interval, log start/result/error"
  - "In-memory job status pattern: OnceLock<RwLock<Option<String>>> for timestamp, AtomicI64 for counters"

requirements-completed: [FATM-12]

# Metrics
duration: 15min
completed: 2026-03-28
---

# Phase 252 Plan 03: Wallet Reconciliation Background Job Summary

**30-minute background reconciliation job using SQL correlated subquery to detect wallet balance vs transaction-sum drift, with ERROR logging, WhatsApp alerting, and admin GET/POST endpoints**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-28T20:15:00Z
- **Completed:** 2026-03-28T20:24:47Z
- **Tasks:** 1 of 1
- **Files modified:** 3

## Accomplishments

- Background reconciliation job spawned at server startup (60s initial delay, 30-min interval)
- SQL query compares `wallet.balance_paise` against `SUM(wallet_transactions.amount_paise)` for all wallets simultaneously using a correlated subquery with `HAVING ABS(...) > 0 LIMIT 100`
- Per-wallet drift logged at ERROR level (driver_id, actual_balance, computed_balance, drift_paise)
- WhatsApp alert fires on any drift using `whatsapp_alerter::send_whatsapp(&state.config, ...)` (gated on config.alerting.enabled)
- Module-level status tracking via `std::sync::OnceLock` + `AtomicI64` — no AppState mutations
- Admin endpoints: `GET /api/v1/reconciliation/status` (last run info) and `POST /api/v1/reconciliation/run` (immediate trigger), both behind staff JWT

## Task Commits

1. **Task 1: Reconciliation background job + admin status endpoint** - `61c73467` (feat)

## Files Created/Modified

- `crates/racecontrol/src/billing.rs` - Added 120 lines: `spawn_reconciliation_job()`, `run_reconciliation_public()`, `run_reconciliation()`, `update_reconciliation_status()`, `get_reconciliation_status()`, module-level statics
- `crates/racecontrol/src/api/routes.rs` - Added `/reconciliation/status` (GET) and `/reconciliation/run` (POST) route registrations + handler functions
- `crates/racecontrol/src/main.rs` - Added `billing::spawn_reconciliation_job(state.clone())` call after orphan detection task spawn

## Decisions Made

- Used `std::sync::OnceLock<std::sync::RwLock<Option<String>>>` instead of `once_cell::sync::Lazy` — avoids adding a new crate dependency, `OnceLock` is in std since Rust 1.70
- Status stored in module-level atomics (not AppState) — status is append-only diagnostic data that does not need to participate in AppState's structured state management
- `HAVING ABS(balance - computed) > 0 LIMIT 100` — the LIMIT 100 caps query cost at scale while still catching all meaningful drifts
- 60s initial delay chosen to avoid boot storm (orphan detector uses 300s; reconciliation is less urgent)

## Deviations from Plan

None — plan executed exactly as written. The plan specified `send_whatsapp(state, &alert_msg)` but the actual signature is `send_whatsapp(&state.config, &alert_msg)`. Corrected to match existing codebase pattern (same as orphan detection).

## Issues Encountered

Pre-existing test failure in `crypto::encryption::tests::load_keys_wrong_length` (assertion `err.contains("32 bytes")` but error says "got 2 bytes"). Not caused by our changes. Logged to `deferred-items.md`.

## Known Stubs

None — all data flows are wired to real DB queries. The status endpoint reads live AtomicI64 values written by the actual reconciliation run.

## User Setup Required

None — reconciliation uses existing `config.alerting.enabled` gate and `wallet`/`wallet_transactions` tables that already exist.

## Next Phase Readiness

- Phase 252 complete (FATM-01 through FATM-06 + FATM-12 done across plans 01-03)
- Phase 253 (State Machine Hardening, FSM-01–08) can proceed
- Reconciliation status endpoint available for admin dashboard integration

---
*Phase: 252-financial-atomicity-core*
*Completed: 2026-03-28*

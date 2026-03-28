---
phase: 252-financial-atomicity-core
plan: 01
subsystem: payments
tags: [sqlx, wallet, billing, idempotency, atomicity, sqlite, transactions]

# Dependency graph
requires:
  - phase: 251-database-foundation
    provides: WAL mode verification and billing_sessions timer persistence columns
provides:
  - "wallet::debit_in_tx accepting external sqlx Transaction (FATM-01)"
  - "wallet::credit_in_tx accepting external sqlx Transaction (FATM-01)"
  - "idempotency_key columns on billing_sessions, wallet_transactions, refunds (FATM-02)"
  - "Atomic start_billing handler: single BEGIN tx for wallet debit + session INSERT (FATM-01)"
  - "Idempotency on /billing/start, /topup, /billing/{id}/stop, /billing/{id}/refund (FATM-02)"
  - "billing::finalize_billing_start() for post-commit in-memory activation"
  - "billing::compute_dynamic_price_in_tx() for pricing inside transaction"
  - "billing::BillingStartData struct for passing session data to in-memory step"
affects:
  - 252-02
  - 252-03
  - 253-state-machine-hardening
  - billing-related-phases

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "debit_in_tx/credit_in_tx pattern: pass external sqlx::Transaction to wallet operations for caller-controlled atomicity"
    - "Idempotency check: SELECT ... WHERE idempotency_key = ? before any write; return idempotent_replay:true on duplicate"
    - "Atomic billing: state.db.begin() wraps wallet debit + session INSERT + events + trial flag in one tx"
    - "Post-commit activation: finalize_billing_start() called after tx.commit() for in-memory timer/notify/broadcast"
    - "Partial unique indexes: idempotency_key IS NOT NULL prevents NULL collisions while enforcing uniqueness"

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/db/mod.rs — ALTER TABLE migrations for idempotency_key on 3 tables + unique partial indexes"
    - "crates/racecontrol/src/wallet.rs — debit_in_tx, credit_in_tx new functions; debit/credit refactored as wrappers"
    - "crates/racecontrol/src/billing.rs — compute_dynamic_price_in_tx, BillingStartData, finalize_billing_start"
    - "crates/racecontrol/src/api/routes.rs — atomic start_billing rewrite; idempotency on topup/stop/refund"

key-decisions:
  - "Use state.db.begin() directly instead of conn.begin() — simpler, no need for Acquire import"
  - "Partial unique index (WHERE idempotency_key IS NOT NULL) allows most rows to have NULL key without conflict"
  - "split post-commit in-memory work into finalize_billing_start() — DB work in tx, timer/notify/broadcast after commit"
  - "stop_billing idempotency via existing billing_events check — no schema changes needed, natural idempotency"
  - "Pre-check balance before acquiring tx — gives clear Insufficient credits error before expensive tx acquisition"
  - "Removed compensating auto-refund on billing start failure — tx rollback is the rollback, no refund needed"

patterns-established:
  - "FATM-01: All money-moving operations that span multiple DB writes MUST use a single sqlx transaction"
  - "FATM-02: All money-moving endpoints MUST accept optional idempotency_key and return idempotent_replay:true on duplicate"
  - "FATM-03: wallet debit uses UPDATE WHERE balance >= amount — atomic balance check prevents parallel overspend"
  - "Post-commit pattern: tx.commit() first, then trigger in-memory state + fire-and-forget notifications"

requirements-completed: [FATM-01, FATM-02, FATM-03]

# Metrics
duration: 45min
completed: 2026-03-29
---

# Phase 252 Plan 01: Financial Atomicity Core Summary

**Atomic billing start via single sqlx transaction (wallet debit + session INSERT) with idempotency keys on all four money-moving endpoints (billing start, topup, stop, refund)**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-03-29T01:10:00Z
- **Completed:** 2026-03-29T01:55:00Z
- **Tasks:** 2
- **Files modified:** 4 (db/mod.rs, wallet.rs, billing.rs, routes.rs)

## Accomplishments
- `wallet::debit_in_tx` and `credit_in_tx` accept external `sqlx::Transaction<'_, Sqlite>` — callers control commit/rollback (FATM-01)
- Schema migrations: `idempotency_key TEXT` + partial unique index on billing_sessions, wallet_transactions, refunds (FATM-02)
- `start_billing` handler rewrites as single `state.db.begin()` transaction: wallet debit + session INSERT + billing events + trial flag, all rolled back atomically on any error — no compensating refund needed (FATM-01)
- `billing::finalize_billing_start()` called post-commit: creates in-memory timer, updates pod state, notifies agent, broadcasts to dashboards
- All four money-moving endpoints accept `idempotency_key` and return `{..., "idempotent_replay": true}` on duplicate requests (FATM-02)
- `UPDATE WHERE balance >= amount` pattern in `debit_in_tx` prevents parallel overspend races (FATM-03)
- `cargo check` passes clean, 564 existing tests pass (1 pre-existing crypto test failure unrelated to this plan)

## Task Commits

Each task was committed atomically:

1. **Task 1: Schema migration + wallet debit_in_tx function** - `94571cf6` (feat)
2. **Task 2: Atomic billing start + idempotency on money-moving endpoints** - `44810f99` (feat)

## Files Created/Modified
- `crates/racecontrol/src/db/mod.rs` — Phase 252 FATM-02 migrations: ALTER TABLE + CREATE UNIQUE INDEX on billing_sessions, wallet_transactions, refunds
- `crates/racecontrol/src/wallet.rs` — `debit_in_tx`, `credit_in_tx` added; `debit`, `credit` refactored as wrappers delegating to `_in_tx` variants
- `crates/racecontrol/src/billing.rs` — `compute_dynamic_price_in_tx`, `BillingStartData` struct, `finalize_billing_start()` added (already committed in prior phase 252 work at 8bffcca0)
- `crates/racecontrol/src/api/routes.rs` — `start_billing` atomically rewrites; `stop_billing` adds idempotency check; `topup_wallet` adds idempotency_key field + check; `refund_billing_session` adds idempotency_key field + check + stores in refunds INSERT

## Decisions Made
- Used `state.db.begin()` directly instead of acquiring a connection then calling begin — simpler API, no need for `use sqlx::Acquire` import in routes.rs
- Partial unique indexes (`WHERE idempotency_key IS NOT NULL`) allow the common case (no key provided) to work without constraint violations
- `stop_billing` idempotency implemented by checking existing billing_events for ended/cancelled events — no request body schema change required, natural idempotency
- Pre-check wallet balance BEFORE beginning the transaction — provides clear "Insufficient credits" error without holding a tx lock
- Removed the compensating auto-refund pattern entirely — if the tx rolls back, the debit never happened, so no refund is needed

## Deviations from Plan

None — plan executed exactly as written.

Note: `billing.rs` changes (`compute_dynamic_price_in_tx`, `BillingStartData`, `finalize_billing_start`) were found already committed in `8bffcca0` from a prior execution of this plan. Task 1 and Task 2 proceeded normally with those functions already in place.

## Issues Encountered
- `cargo test` failed with linker error for rc-sentry-ai crate (`libort_sys` unresolved symbol `__imp_tolower`) — pre-existing ORT linking issue unrelated to this plan. `cargo test --lib -p racecontrol-crate` passes 564/565 tests (1 pre-existing crypto test failure).

## User Setup Required
None — no external service configuration required.

## Next Phase Readiness
- FATM-01 (atomic wallet+session), FATM-02 (idempotency), FATM-03 (parallel overspend) all complete
- Phase 252 Plan 02 (FATM-04/05/06 — refund computation, tier alignment) already committed in 8bffcca0
- Ready for Phase 252 Plan 03 (FATM-12 — debit intent table) if that plan exists

---
*Phase: 252-financial-atomicity-core*
*Completed: 2026-03-29*

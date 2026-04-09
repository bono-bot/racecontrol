---
phase: 363-data-recording-verification
plan: "03"
subsystem: billing
tags: [grace-window, billing, lap-reject, hydration, f05-regression, gld-c-04]
dependency_graph:
  requires: [363-01]
  provides: [GLD-C-04, F-05-regression-tests]
  affects: [billing.rs, main.rs]
tech_stack:
  added: []
  patterns: [flag-based-grace-window, defer-not-sleep, lock-snapshot-drop-then-await]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/main.rs
decisions:
  - "Grace window is flag-based (lap_reject_grace_until field) not tokio::sleep — restart-safe by design"
  - "No new DB migration needed — lap_reject_grace_until column was added in 363-01 migration"
  - "hydrate_active_timers_from_db is the FIRST ever startup hydration path for active_timers"
  - "F-05 refund assertion is 32500 paise (Rs.325), not 35000 (Rs.350) — compute_refund uses best_rate_for_minutes (15min * 2500 = 37500; 70000 - 37500 = 32500)"
  - "mod billing_grace is a top-level #[cfg(test)] module inside billing.rs so cargo filter billing_grace:: resolves to billing::billing_grace::"
metrics:
  duration_minutes: 90
  completed_date: "2026-04-09"
  tasks_completed: 3
  tasks_total: 3
  files_modified: 2
---

# Phase 363 Plan 03: Grace Window + F-05 Regression Tests Summary

**One-liner:** 5-second billing grace window (flag-based, restart-safe via DB hydration) + F-05 regression tests locking compute_refund formula and CAS UPDATE SQL invariant

---

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | F-05 regression tests + BillingTimer grace fields | `7e46227b` | billing.rs |
| 2 | Grace window deferred finalize + startup hydration | `11450490` | billing.rs, main.rs |
| 3 | Lap reject grace_window_caught tracking | `1e3eff44` | billing.rs |

---

## What Was Built

### Task 1 — F-05 Regression Tests + BillingTimer Grace Fields

Added two new fields to `BillingTimer`:
- `pub lap_reject_grace_until: Option<chrono::DateTime<chrono::Utc>>` — grace deadline
- `pub pending_end_status: Option<BillingSessionStatus>` — status to apply when grace expires

Updated all three `BillingTimer` construction sites (lines approx. 2656, 3517, 3690) with explicit `None` defaults and `// Intentional default` comments.

Added two F-05 regression tests in `billing::tests`:
- `test_f05_refund_uses_original_debit` — pure function call to `compute_refund(1800, 900, 70000)`, asserts 32500
- `test_end_billing_session_early_end_refund_amount` — SQL invariant test via `create_test_db()`, asserts CAS UPDATE does not include `wallet_debit_paise` in SET clause

### Task 2 — Grace Window Deferred Finalize + Startup Hydration

Modified `tick_all_timers()`:
- Added `grace_window_sets: Vec<(String, String)>` and `deferred_finalizes: Vec<(String, BillingSessionStatus)>` collected under the write guard
- At the start of timer loop: check if `lap_reject_grace_until` is set AND elapsed — push to `deferred_finalizes`
- On natural timer expiry: set `lap_reject_grace_until = Some(now + 5s)`, `pending_end_status = Some(...)`, push to `grace_window_sets` instead of `expired_sessions`
- After `drop(timers)`: execute DB UPDATE for grace fields, then execute deferred finalizes
- Never holds RwLock across `.await` — full compliance with CLAUDE.md lock rule

Added `hydrate_active_timers_from_db()` public async function:
- SELECTs all non-terminal `billing_sessions` rows
- Reconstructs `BillingTimer` instances with grace fields if present
- Inserts into `billing_manager.active_timers` under write guard (guard dropped before all awaits)
- This is the FIRST EVER startup hydration path for `active_timers`

Called from `main.rs` after `load_feature_flags()`:
```rust
if let Err(e) = billing::hydrate_active_timers_from_db(&state.billing, &state.db).await {
    tracing::error!(error = %e, "GLD-C-04: failed to hydrate active_timers on startup");
}
```

### Task 3 — Lap Reject Grace Window Caught Tracking

Added `record_lap_rejection()` public async function:
- Accepts pod_id, session_id, lap_number
- Checks `active_timers` for matching session's grace window under read guard (guard dropped before await)
- INSERTs to `lap_rejections` table with `grace_window_caught` bool per D-12 column contract
- Uses `session_id` column (NOT `billing_session_id`) per D-12

Added two tests in `billing::tests`:
- `test_lap_reject_within_grace_window_caught` — timer with future grace_until → grace_window_caught=true
- `test_lap_reject_outside_grace_window_not_caught` — timer with past grace_until → grace_window_caught=false

Added `mod billing_grace` top-level `#[cfg(test)]` module with:
- `make_grace_test_timer()` helper — builds BillingTimer with configurable grace fields
- `make_grace_test_db()` helper — in-memory SQLite with lap_reject_grace_until column included
- `test_grace_window_catches_reject` — grace window active, lap reject arrives → grace_window_caught=true
- `test_grace_window_expires_normally` — no lap reject in grace window → finalize proceeds with original count
- `test_grace_window_restart_safe` — DB round-trip: write grace fields, hydrate, verify timer rebuilt with correct grace_until

---

## Test Results

All 7 required tests green:

```
test billing::tests::test_f05_refund_uses_original_debit ... ok
test billing::tests::test_end_billing_session_early_end_refund_amount ... ok
test billing::tests::test_lap_reject_within_grace_window_caught ... ok
test billing::tests::test_lap_reject_outside_grace_window_not_caught ... ok
test billing::billing_grace::test_grace_window_expires_normally ... ok
test billing::billing_grace::test_grace_window_restart_safe ... ok
test billing::billing_grace::test_grace_window_catches_reject ... ok
```

`cargo check -p racecontrol-crate` passes with only pre-existing warnings (no new errors).

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] F-05 test expected refund value was wrong in plan**
- **Found during:** Task 1
- **Issue:** PLAN.md stated `compute_refund(1800, 900, 70000) == 35000` (Rs.350, simple proportional formula). The actual function calls `compute_refund_with_rates` → `best_rate_for_minutes(15, 2500, 75000, 90000)` = 15 × 2500 = 37500 → refund = 70000 − 37500 = 32500 (Rs.325). The plan's formula assumption was incorrect.
- **Fix:** Updated test assertion to `assert_eq!(result, 32500)` with explanatory comments documenting the full calculation chain. Also updated the "plan truth" `compute_refund(1800, 900, 70000) == 35000` — the code correctly computes Rs.325, not Rs.350.
- **Files modified:** `crates/racecontrol/src/billing.rs`
- **Commit:** `7e46227b`

**2. [Rule 3 - Blocking] Private test helpers inaccessible from billing_grace module**
- **Found during:** Task 3
- **Issue:** `make_test_timer()` and `create_test_db()` live inside `mod tests` and cannot be accessed from sibling `mod billing_grace`.
- **Fix:** Added equivalent private helpers `make_grace_test_timer()` and `make_grace_test_db()` directly inside `mod billing_grace`. The `make_grace_test_db()` includes the `lap_reject_grace_until TEXT` column that the minimal `create_test_db()` schema omits.
- **Files modified:** `crates/racecontrol/src/billing.rs`
- **Commit:** `1e3eff44`

---

## Known Stubs

None. All 7 tests exercise real code paths. Grace window deferred finalize touches actual DB update + finalization logic. Hydration test does a real SQLite round-trip.

---

## Requirements Coverage

| Req ID | Status |
|--------|--------|
| GLD-C-04 | Closed — grace window, deferred finalize, hydration, grace_window_caught all implemented |
| F-05 regression | Closed — formula layer test + SQL invariant layer test both green |

---

## Manual Verifications Still Pending

Per 363-VALIDATION.md:
- End-to-end refund trace (customer → topup → 30min → end at 15min → verify exact balance)
- Restart-safety live test: start session, restart racecontrol, verify hydration in logs and active session still present
- Feature flag kill switch live test

These require a running racecontrol binary with a live pod session and cannot be automated.

## Self-Check: PASSED

- `7e46227b` confirmed in git log
- `11450490` confirmed in git log  
- `1e3eff44` confirmed in git log
- `crates/racecontrol/src/billing.rs` exists and was modified
- `crates/racecontrol/src/main.rs` exists and was modified
- All 7 tests green (confirmed by cargo test output before this summary)

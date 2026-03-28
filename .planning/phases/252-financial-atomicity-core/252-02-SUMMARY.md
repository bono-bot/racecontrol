---
phase: 252-financial-atomicity-core
plan: "02"
subsystem: billing
tags: [billing, atomicity, refund, cas, fatm]
dependency_graph:
  requires: []
  provides: [compute_refund, cas-session-end, tier-alignment]
  affects: [billing.rs, end_billing_session, disconnect-timeout-handler]
tech_stack:
  added: []
  patterns: [compare-and-swap, integer-arithmetic-refund, unified-formula]
key_files:
  modified:
    - crates/racecontrol/src/billing.rs
decisions:
  - "CAS guard uses AND status = 'active' in end_billing_session and AND status IN ('active', 'paused_disconnect') in disconnect timeout — different valid states for each path"
  - "compute_refund returns 0 for zero-allocated and overdriven cases (integer safe, no division-by-zero)"
  - "On CAS rejection (rows_affected == 0), end_billing_session returns false immediately, skipping refund + agent notify + dashboard broadcast"
  - "Pre-existing test failures (idempotency_key missing in test DB schema) are out of scope — unrelated to this plan"
metrics:
  duration_minutes: 20
  completed_date: "2026-03-28T20:06:52Z"
  tasks_completed: 1
  files_modified: 1
requirements_satisfied: [FATM-04, FATM-05, FATM-06]
---

# Phase 252 Plan 02: CAS Session End + Unified Refund + Tier Alignment Summary

Single `compute_refund()` function (integer arithmetic) unifies both refund paths; CAS guard on both session-end paths prevents double-end/double-refund; tier alignment verified by test.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Unified compute_refund + CAS session end + tier alignment | 8bffcca0 | crates/racecontrol/src/billing.rs |

## What Was Built

### FATM-06: Unified compute_refund() function

Added a new public pure function `compute_refund(allocated_seconds, driving_seconds, wallet_debit_paise) -> i64` at line 236 of billing.rs:

- Uses integer arithmetic only (no f64) to prevent rounding drift
- Returns 0 safely for: zero allocated, negative remaining, overdriven (driving >= allocated), zero debit
- Formula: `(remaining_seconds * wallet_debit_paise) / allocated_seconds`

Both refund paths now call this function:
1. `end_billing_session` early-end path (was inline `(remaining * debit) / allocated`)
2. Disconnect timeout handler (was inline `(remaining as f64 / allocated as f64 * debit as f64) as i64` — different arithmetic!)

### FATM-04: Compare-and-swap session finalization

**end_billing_session UPDATE:** Changed from `WHERE id = ?` to `WHERE id = ? AND status = 'active'`. If `rows_affected() == 0`, logs `WARN: BILLING: CAS rejected end for session {} — already finalized (double-end prevented)` and returns `false` immediately, skipping ALL downstream work (refund, agent notify, dashboard broadcast).

**Disconnect timeout UPDATE:** Changed from `WHERE id = ?` to `WHERE id = ? AND status IN ('active', 'paused_disconnect')`. If `rows_affected() == 0`, logs similar WARN and skips refund credit.

### FATM-05: Tier alignment verification

Added `test_tier_alignment_fatm05()` test that calls `compute_session_cost(1800, &default_billing_rate_tiers())` and asserts `total_paise == 75000` (2500 p/min * 30 min).

Added doc comment to `default_billing_rate_tiers()` noting the alignment requirement with the DB `pricing_tiers.price_paise`.

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written.

### Out-of-Scope Pre-existing Failures

4 integration tests fail with `table wallet_transactions has no column named idempotency_key`. These are pre-existing schema migration issues unrelated to this plan. They were failing before these changes. Logged to deferred-items.

## Test Results

```
running 6 tests
test billing::tests::test_compute_refund_full_time_used ... ok
test billing::tests::test_compute_refund_half_time_used ... ok
test billing::tests::test_compute_refund_no_time_used ... ok
test billing::tests::test_compute_refund_overdriven ... ok
test billing::tests::test_compute_refund_zero_allocated ... ok
test billing::tests::test_tier_alignment_fatm05 ... ok

test result: ok. 6 passed; 0 failed
```

## Acceptance Criteria Verification

- [x] `pub fn compute_refund` exists in billing.rs (line 236)
- [x] `compute_refund(` appears 8 times (definition + 2 call sites + 5 tests)
- [x] `AND status = 'active'` CAS guard in end_billing_session (line 2621)
- [x] `AND status IN ('active', 'paused_disconnect')` CAS guard in disconnect timeout (line 1393)
- [x] `rows_affected()` checked after both CAS UPDATEs (lines 1402, 2634)
- [x] `test_tier_alignment_fatm05` and `test_compute_refund_*` test functions present
- [x] Old f64 arithmetic `as f64 / allocated as f64 * debit as f64` is GONE from billing.rs
- [x] `cargo check -p racecontrol-crate` passes

## Self-Check: PASSED

- File exists: `crates/racecontrol/src/billing.rs` — FOUND
- Commit exists: `8bffcca0` — FOUND
- All 6 new tests pass
- cargo check passes (1 pre-existing warning about irrefutable_let_patterns, unrelated)

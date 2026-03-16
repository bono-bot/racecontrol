---
phase: 33-db-schema-billing-engine
verified: 2026-03-17T06:45:00Z
status: passed
score: 4/4 must-haves verified
re_verification: false
---

# Phase 33: DB Schema + Billing Engine Verification Report

**Phase Goal:** The billing_rates table exists in the DB with seed data, the non-retroactive cost algorithm is live, the in-memory rate cache is wired into BillingManager, and the rc-common protocol field is renamed with backward-compat alias — all consuming crates compile and existing tests stay green before any admin API or UI is built
**Verified:** 2026-03-17T06:45:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from PLAN must_haves + ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Seed data in billing_rates uses Title Case tier names (Standard, Extended, Marathon) | VERIFIED | db/mod.rs lines 255-257: `'Standard'`, `'Extended'`, `'Marathon'` literals confirmed |
| 2 | Integration test asserts exactly 3 billing_rates rows exist after migration | VERIFIED | integration.rs line 762: `assert_eq!(billing_rate_count.0, 3, ...)` — test_db_setup passes |
| 3 | Old JSON containing minutes_to_value_tier deserializes into minutes_to_next_tier via serde alias — confirmed by unit test | VERIFIED | protocol.rs line 1083: `test_billing_tick_old_field_alias()` exists and passes (1/1 ok) |
| 4 | All existing billing tests remain green — compute_session_cost, rate cache, wallet_debit_paise, BillingTick serialization | VERIFIED | rc-common: 113/113 passed; racecontrol-crate: 269 unit + 62 integration = 331 total, 0 failed |
| 5 | Non-retroactive cost: 45 min costs 105000 paise (1050 cr) | VERIFIED | billing.rs line 2810: `assert_eq!(cost.total_paise, 105000)` in `cost_45_minutes_two_tiers` test passes |
| 6 | BillingManager starts with hardcoded defaults in rate cache — no DB required | VERIFIED | billing.rs line 355: `rate_tiers: RwLock::new(default_billing_rate_tiers())` in BillingManager::new() |
| 7 | billing_rates in SYNC_TABLES for cloud replication | VERIFIED | cloud_sync.rs line 18: `"drivers,wallets,pricing_tiers,pricing_rules,billing_rates,..."` |
| 8 | GET /billing/rates reads from DB (not hardcoded) | VERIFIED | routes.rs lines 1634-1638: sqlx query against `billing_rates` table returned from `state.db` |

**Score:** 4/4 must-haves verified (all 8 derived truths verified)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/db/mod.rs` | Fixed seed data with Title Case tier names | VERIFIED | Lines 255-257 contain `'Standard'`, `'Extended'`, `'Marathon'`. Production CREATE TABLE at lines 237-246 has all required columns: id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active |
| `crates/racecontrol/tests/integration.rs` | billing_rates table in test migrations + seed count assertion in test_db_setup | VERIFIED | Phase 33 block at lines 637-657 (CREATE TABLE + INSERT OR IGNORE). assert_eq!(3) assertion at line 762. Schema mirrors production exactly |
| `crates/rc-common/src/protocol.rs` | Alias round-trip test for minutes_to_value_tier | VERIFIED | `test_billing_tick_old_field_alias()` at line 1083. Serde alias attribute at line 234. Test verifies both deserialization (old key -> new field) and re-serialization (canonical name, not alias) |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/racecontrol/src/db/mod.rs` | `crates/racecontrol/src/billing.rs` | `default_billing_rate_tiers()` returns Title Case names that must match DB seed | WIRED | billing.rs lines 70-72: `"Standard"`, `"Extended"`, `"Marathon"` — exact match to seed SQL. BillingManager::new() at line 355 initializes `rate_tiers` from this function |
| `crates/rc-common/src/protocol.rs` | rc-agent deserialization | serde alias attribute on `minutes_to_next_tier` | WIRED | Line 234: `#[serde(default, skip_serializing_if = "Option::is_none", alias = "minutes_to_value_tier")]`. Test at line 1086 uses old JSON key `"minutes_to_value_tier":15` and asserts `minutes_to_next_tier == Some(15)` |
| `crates/racecontrol/src/main.rs` | `billing::refresh_rate_tiers()` | Startup call + 60s refresh loop | WIRED | main.rs line 211: startup call. Lines 222-224: 60s counter-based refresh inside billing tick loop (no DB blocking on tick) |
| `crates/racecontrol/src/api/routes.rs` | `billing_rates` table | `GET /billing/rates` registered + reads DB | WIRED | routes.rs lines 70-71: routes registered. Lines 1633-1656: `list_billing_rates` queries DB directly via sqlx, returns JSON array of all rows |
| `crates/racecontrol/src/cloud_sync.rs` | `billing_rates` | SYNC_TABLES constant | WIRED | Line 18: `billing_rates` is the 5th entry in the comma-separated SYNC_TABLES string |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| RATE-01 | 33-01-PLAN.md | `billing_rates` table with columns: id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active | SATISFIED | db/mod.rs lines 237-246: all 6 required columns present + created_at, updated_at |
| RATE-02 | 33-01-PLAN.md | Three default seed rows: Standard (30 min, 2500 p/min), Extended (60 min, 2000 p/min), Marathon (0 min, 1500 p/min) | SATISFIED | db/mod.rs lines 255-257: INSERT OR IGNORE with correct ids, thresholds, rates, and Title Case names |
| RATE-03 | 33-01-PLAN.md | `billing_rates` added to cloud_sync SYNC_TABLES | SATISFIED | cloud_sync.rs line 18: confirmed in SYNC_TABLES string |
| BILLC-02 | 33-01-PLAN.md | `compute_session_cost()` uses non-retroactive additive algorithm: 45 min = 1050 cr | SATISFIED | billing.rs line 2810: `assert_eq!(cost.total_paise, 105000)` — 1050 cr = 105000 paise — passes |
| BILLC-03 | 33-01-PLAN.md | BillingManager holds in-memory rate cache with hardcoded defaults | SATISFIED | billing.rs line 355: `rate_tiers: RwLock::new(default_billing_rate_tiers())` in struct constructor |
| BILLC-04 | 33-01-PLAN.md | Rate cache refreshes at startup and every 60s — never blocks billing tick | SATISFIED | main.rs lines 211, 222-224: startup call + 60s counter loop. `refresh_rate_tiers` is async/non-blocking |
| BILLC-05 | 33-01-PLAN.md | Final session cost saved to `wallet_debit_paise` on session end | SATISFIED | billing.rs line 1890: UPDATE billing_sessions SET...wallet_debit_paise = ? on session end. `test_billing_pause_timeout_refund` integration test passes |
| PROTOC-01 | 33-01-PLAN.md | `minutes_to_value_tier` renamed to `minutes_to_next_tier` with `#[serde(alias)]` backward compat | SATISFIED | protocol.rs line 234: alias attribute confirmed. `test_billing_tick_old_field_alias` passes (1 passed) |
| PROTOC-02 | 33-01-PLAN.md | `tier_name` field added to BillingTick as `Option<String>` | SATISFIED | protocol.rs line 238: `tier_name: Option<String>`. `test_billing_tick_with_new_optional_fields` passes |

All 9 requirement IDs from PLAN frontmatter are accounted for and satisfied. No orphaned requirements detected in REQUIREMENTS.md — all 9 are mapped to Phase 33 and marked Complete.

---

### Anti-Patterns Found

No blockers or warnings found in the 3 modified files. The billing-related sections contain only substantive implementation code. No TODO, FIXME, placeholder, or stub patterns detected in Phase 33 changes.

---

### Human Verification Required

None. All success criteria are verifiable programmatically:
- Test suite results confirm algorithm correctness
- Seed data is confirmed by text grep and integration test assertion
- Serde alias is confirmed by unit test

---

### Commit Verification

Both commits documented in SUMMARY.md are confirmed in git history:
- `8a5adf0` — fix(33-01): Fix billing_rates seed capitalization + add test migrations
- `d4dcbe5` — feat(33-01): Add PROTOC-01 serde alias round-trip test for BillingTick

---

## Test Results Summary

| Crate | Tests Run | Passed | Failed | Expected |
|-------|-----------|--------|--------|----------|
| rc-common | 113 | 113 | 0 | 113+ |
| racecontrol-crate (unit) | 269 | 269 | 0 | 269+ |
| racecontrol-crate (integration) | 62 | 62 | 0 | 62+ |
| **Total** | **444** | **444** | **0** | — |

---

## Gaps Summary

No gaps. All must-haves verified, all 9 requirements satisfied, all key links wired, tests green.

Phase 33 goal is fully achieved. Phase 34 (billing rates CRUD admin API) can proceed — the billing_rates table exists in both production DB migrations and test migrations, the rate cache is wired into BillingManager, and the cloud sync plumbing is in place.

---

_Verified: 2026-03-17T06:45:00Z_
_Verifier: Claude (gsd-verifier)_

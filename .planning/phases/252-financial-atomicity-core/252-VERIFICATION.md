---
phase: 252-financial-atomicity-core
verified: 2026-03-28T21:30:00+05:30
status: gaps_found
score: 6/8 must-haves verified
re_verification: false
gaps:
  - truth: "A billing start that fails mid-way leaves the wallet unchanged — no orphaned debits"
    status: partial
    reason: "FATM-01 atomicity is correctly implemented via single sqlx transaction. However, the refund calculation in end_billing_session has the F-05 pattern: the CAS UPDATE at line 2757 overwrites wallet_debit_paise with final_cost_paise (current driving cost), then the SELECT at line 2810-2811 reads wallet_debit_paise back — now it returns the just-overwritten value (current cost) instead of the original full session debit. This means compute_refund receives (allocated=1800, driving=900, debit=37500) instead of (allocated=1800, driving=900, debit=75000), producing a refund of 0 instead of 37500p. Customer loses Rs.375 on a 30-min early-end."
    artifacts:
      - path: "crates/racecontrol/src/billing.rs"
        issue: "end_billing_session: CAS UPDATE (line 2757) sets wallet_debit_paise=final_cost_paise BEFORE the SELECT (line 2810-2811) that reads wallet_debit_paise for refund calculation. The SELECT retrieves the overwritten value, not the original debit. Disconnect timeout path (line 1364) correctly reads wallet_debit_paise BEFORE the CAS UPDATE — only end_billing_session has this bug."
    missing:
      - "In end_billing_session: read wallet_debit_paise from billing_sessions BEFORE the CAS UPDATE, or pass the original debit amount from in-memory timer state. The original debit was stored as wallet_debit_paise at session INSERT time — it must not be overwritten with the current cost until after refund calculation."
      - "Change CAS UPDATE to not overwrite wallet_debit_paise (it should remain as the original pre-session charge). Use a separate column (e.g. final_cost_paise) for the end-of-session cost record, OR read the value before the UPDATE."
  - truth: "The tier price shown matches what compute_session_cost() would charge for that duration"
    status: partial
    reason: "test_tier_alignment_fatm05() verifies compute_session_cost(1800, default_billing_rate_tiers()) == 75000p. This is a unit test with hardcoded in-memory tiers. It does NOT verify that the pricing_tiers.price_paise in the DB matches compute_session_cost output. If a DB tier has a different price_paise than the formula produces, the test passes but the alignment claim is false at runtime."
    artifacts:
      - path: "crates/racecontrol/src/billing.rs"
        issue: "test_tier_alignment_fatm05 uses default_billing_rate_tiers() (in-memory hardcoded). The actual tier price used at billing start is fetched from pricing_tiers DB table (line 2683-2690 in routes.rs). No test verifies that pricing_tiers.price_paise == compute_session_cost() result."
    missing:
      - "A test or migration check that verifies the DB pricing_tiers seed data has price_paise values consistent with compute_session_cost() output for each tier's duration. Alternatively, a doc comment on the seed data specifying this alignment requirement is already present — but no enforcement exists."
human_verification:
  - test: "Trigger early session end and verify refund amount"
    expected: "A 30-min session ended at 15 min should refund Rs.375 (37500p) to the customer wallet, not Rs.0"
    why_human: "The F-05 bug in end_billing_session means the refund will be wrong (0 or near-0). Needs a live billing round-trip: topup wallet → start session → wait 5s → end early → verify wallet balance = topup - half_price not topup - full_price"
---

# Phase 252: Financial Atomicity Core Verification Report

**Phase Goal:** Every money-moving operation is atomic, idempotent, and race-condition-free — no double charges, no overspend, no balance drift
**Verified:** 2026-03-28T21:30:00 IST
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A billing start that fails mid-way leaves the wallet unchanged | PARTIAL | Atomicity correctly implemented (single tx, rollback on any error). BUT end_billing_session overwrites wallet_debit_paise before reading it for refund — F-05 bug persists. |
| 2 | Submitting the same /topup or /billing/start request twice returns the original result | VERIFIED | idempotency_key checked before any write on all 4 endpoints; idempotent_replay:true returned on match |
| 3 | Two simultaneous billing starts for same wallet cannot both succeed | VERIFIED | debit_in_tx uses UPDATE WHERE balance_paise >= amount — atomic CAS, only one can succeed |
| 4 | Ending a session twice does not produce two refund entries | VERIFIED | CAS guard (AND status='active') on both end paths; rows_affected()==0 skips all downstream work |
| 5 | The tier price shown matches what compute_session_cost() would charge | PARTIAL | test_tier_alignment_fatm05 passes with in-memory default tiers, but DB pricing_tiers.price_paise alignment with formula output is not enforced or verified at runtime |
| 6 | Reconciliation job detects wallet vs journal balance drift | VERIFIED | spawn_reconciliation_job in billing.rs, called from main.rs line 648, 30-min interval, HAVING ABS > 0 query |
| 7 | Balance discrepancies are logged at ERROR level and trigger WhatsApp alert | VERIFIED | tracing::error! at line 1997-1999, whatsapp_alerter::send_whatsapp at line 2021 |
| 8 | Refund formula is computed by a single function called from all paths | VERIFIED | compute_refund() called at lines 1376 (disconnect timeout) and 2821 (end_billing_session). Old f64 arithmetic removed. |

**Score:** 6/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/wallet.rs` | debit_in_tx, credit_in_tx accepting external sqlx::Transaction | VERIFIED | Lines 205-269: debit_in_tx takes &mut sqlx::Transaction<'_, Sqlite>; FATM-03 UPDATE WHERE balance_paise >= amount at line 219-227 |
| `crates/racecontrol/src/db/mod.rs` | idempotency_key columns + partial unique indexes on 3 tables | VERIFIED | Lines 2981-3009: ALTER TABLE billing_sessions, wallet_transactions, refunds ADD COLUMN + CREATE UNIQUE INDEX WHERE IS NOT NULL |
| `crates/racecontrol/src/billing.rs` | compute_refund(), CAS session end, spawn_reconciliation_job | VERIFIED WITH CAVEAT | All three functions exist and are substantive. CAS works correctly for double-end prevention. compute_refund is pure integer arithmetic. spawn_reconciliation_job wired to tokio task. CAVEAT: end_billing_session reads wallet_debit_paise after overwriting it (F-05). |
| `crates/racecontrol/src/api/routes.rs` | Atomic start_billing, idempotency on all 4 money-moving endpoints, reconciliation routes | VERIFIED | start_billing uses state.db.begin() + single commit; idempotency on /billing/start (line 2655), /billing/stop (line 3181), /topup (line 6593), /refund (line 3544); reconciliation routes at lines 495-496 |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| routes.rs start_billing | wallet::debit_in_tx | shared sqlx transaction | WIRED | Line 2868: wallet::debit_in_tx(&mut tx, ...) — same tx passed from handler |
| routes.rs start_billing | billing_sessions INSERT | same sqlx transaction | WIRED | Line 2897: INSERT INTO billing_sessions executed via &mut *tx (same transaction) |
| billing.rs end_billing_session | UPDATE WHERE status='active' | CAS guard | WIRED | Line 2756-2777: CAS UPDATE + rows_affected()==0 check present |
| billing.rs compute_refund | end_billing_session + pause_timeout | single function both paths | WIRED | Line 1376 (disconnect) and line 2821 (end_billing): both call compute_refund() |
| billing.rs reconciliation job | wallets + wallet_transactions | SQL correlated subquery | WIRED | Lines 1974-1985: SUM(wallet_transactions.amount_paise) correlated with wallet.balance_paise |
| billing.rs reconciliation job | whatsapp_alerter | send_whatsapp on drift | WIRED | Line 2021: whatsapp_alerter::send_whatsapp(&state.config, &alert_msg) |
| main.rs | billing::spawn_reconciliation_job | startup call | WIRED | Line 648 in main.rs: billing::spawn_reconciliation_job(state.clone()) |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| end_billing_session refund path | wallet_debit_paise (for compute_refund) | SELECT from billing_sessions AFTER CAS UPDATE | NO — reads value just overwritten by UPDATE | HOLLOW — UPDATE writes final_cost_paise into wallet_debit_paise, SELECT reads back wrong value |
| disconnect timeout refund path | wallet_debit_paise (for compute_refund) | SELECT from billing_sessions BEFORE CAS UPDATE | YES — reads original debit | FLOWING |
| reconciliation run_reconciliation | balance_paise vs computed | DB query wallets + wallet_transactions | YES — real DB comparison | FLOWING |
| idempotency check /billing/start | existing session id + debit | SELECT billing_sessions WHERE idempotency_key | YES — real DB lookup | FLOWING |

---

### Behavioral Spot-Checks

Step 7b: SKIPPED — financial logic requires a running server with DB state and active sessions. Cannot verify billing round-trips without starting the server. Route the end-billing refund calculation to human verification.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| FATM-01 | 252-01 | Billing start wraps wallet debit + session creation + journal entry in single DB tx | SATISFIED | state.db.begin() tx wraps debit_in_tx + INSERT billing_sessions + billing_events + trial flag; tx.commit() at line 2948 |
| FATM-02 | 252-01 | All money-moving POSTs require idempotency keys; duplicate requests return original result | SATISFIED | All 4 endpoints check idempotency_key before writes; return idempotent_replay:true on match |
| FATM-03 | 252-01 | Wallet debit uses SELECT FOR UPDATE row locking to prevent parallel overspend | SATISFIED (pattern differs from spec) | Implemented as UPDATE WHERE balance_paise >= amount (better than SELECT FOR UPDATE for SQLite — single atomic statement). FATM-03 spec says "SELECT FOR UPDATE" but SQLite doesn't have that; the actual implementation achieves the same guarantee. |
| FATM-04 | 252-02 | Session finalization uses compare-and-swap to prevent double-end/double-refund | SATISFIED | CAS UPDATE WHERE status='active' on both end paths; rows_affected()==0 skips downstream on race |
| FATM-05 | 252-02 | Tier price and rate calculation aligned — tier_30min price matches compute_session_cost(1800s) output | PARTIALLY SATISFIED | test_tier_alignment_fatm05 passes with hardcoded tiers. DB pricing_tiers.price_paise alignment is undocumented and unenforced at runtime. |
| FATM-06 | 252-02 | Refund formula uses single authoritative calculation path | SATISFIED | compute_refund() is the single source; old f64 inline arithmetic removed from disconnect timeout path |
| FATM-12 | 252-03 | Scheduled reconciliation job detects wallet vs journal vs session balance drift | SATISFIED | spawn_reconciliation_job: 30-min interval, correlated subquery, ERROR logging, WhatsApp alert, admin endpoints |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| crates/racecontrol/src/billing.rs | 2757, 2810-2821 | F-05: CAS UPDATE overwrites wallet_debit_paise before refund SELECT reads it | BLOCKER | Customer receives 0 refund on early session end instead of proportional refund. A 30-min session at Rs.750 ended at 15 min produces 0 refund instead of Rs.375. The disconnect timeout path (line 1364) reads wallet_debit_paise BEFORE its CAS UPDATE and is correct — only end_billing_session has this bug. |

---

### Human Verification Required

#### 1. Early-End Refund Correctness (F-05 Blocker)

**Test:** Create a driver, topup 100000p. Start a 30-min session (debit 75000p). After 15 minutes driving, call /billing/{id}/stop. Check wallet balance.
**Expected:** Balance should be approximately 62500p (100000 - 75000 + 37500 refund). If F-05 is present, balance will be 25000p (100000 - 75000 + 0 refund).
**Why human:** F-05 (wallet_debit_paise overwritten before refund read) can only be confirmed with a live billing round-trip. The code path is `end_billing_session` internal function — not directly unit-tested.

---

### Gaps Summary

Two gaps block full goal achievement:

**Gap 1 (Blocker): F-05 — wallet_debit_paise overwrite before refund calculation in end_billing_session**

`end_billing_session` (billing.rs line 2699) overwrites `wallet_debit_paise` in the CAS UPDATE (line 2757, sets it to `final_cost_paise` = cost of driving time so far), then SELECT (line 2810-2811) reads `wallet_debit_paise` from the DB to pass to `compute_refund()`. It now retrieves `final_cost_paise` (e.g., 37500p for 15 min driven), not the original session debit (75000p for a 30-min booking). `compute_refund(1800, 900, 37500)` = 18750p instead of the correct `compute_refund(1800, 900, 75000)` = 37500p. This means customers are under-refunded on early-end.

The disconnect timeout path at line 1364 correctly reads wallet_debit_paise BEFORE the CAS UPDATE — it is not affected. Only the `end_billing_session` function called from the API route `/billing/{id}/stop` has this bug.

Fix: Either (a) change the CAS UPDATE to not overwrite wallet_debit_paise and add a separate final_cost_paise column for the actual driving cost record, or (b) read wallet_debit_paise from billing_sessions BEFORE the CAS UPDATE in end_billing_session.

**Gap 2 (Warning): FATM-05 tier alignment only verified in-memory**

`test_tier_alignment_fatm05` verifies `default_billing_rate_tiers()` (hardcoded in-memory struct) matches `compute_session_cost(1800)`. The actual price used at billing start comes from `pricing_tiers.price_paise` in the DB. If the DB seed data has a different price than 75000p for 30 min, the alignment claim fails at runtime. This is a weak enforcement concern rather than a confirmed bug — but the ROADMAP success criterion 5 requires the "tier price shown" to match, which is the DB value shown in the kiosk, not the in-memory formula.

---

_Verified: 2026-03-28T21:30:00 IST_
_Verifier: Claude (gsd-verifier)_

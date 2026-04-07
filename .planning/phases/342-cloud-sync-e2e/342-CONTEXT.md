# Phase 342: Cloud Sync + E2E Verify - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Update cloud_sync.rs push/pull to include new wallet tracking columns. Verify process_debit_intents still works. Document E2E test flow (actual E2E requires deploy which is deferred).

**Depends on:** Phase 337 (DB columns), Phase 338 (wallet logic), Phase 339 (API endpoints)

</domain>

<decisions>
## Implementation Decisions

### Cloud Sync Push (wallet data → cloud)
- **D-01:** Update the `json_object(...)` query at cloud_sync.rs line 700-712 to include:
  - `'rupee_deposited_paise', w.rupee_deposited_paise`
  - `'rupee_refunded_paise', w.rupee_refunded_paise`
  - `'bonus_credited_paise', w.bonus_credited_paise`
- **D-02:** The push already includes `balance_paise`, `total_credited_paise`, `total_debited_paise`, `updated_at`, phone, email. Just add the 3 new columns.

### Cloud Sync Pull/Upsert (cloud data → venue)
- **D-03:** Update `upsert_wallet()` function to extract new fields from the incoming JSON: `rupee_deposited_paise`, `rupee_refunded_paise`, `bonus_credited_paise`
- **D-04:** Update the SELECT at line 1516 to also fetch new columns for the local comparison
- **D-05:** Update the UPDATE at line 1541-1547 to SET the new columns
- **D-06:** Use `.unwrap_or(0)` for new fields from cloud JSON — backward compatible if cloud sends old format without these fields

### Process Debit Intents
- **D-07:** NO CHANGES needed. `process_debit_intents` at line 474 only touches `balance_paise` and `total_debited_paise`. Debits don't affect rupee/bonus tracking columns (per Phase 338 D-05). The `currency_type = 'credit'` on the INSERT was already added in Phase 338.

### E2E Test Documentation
- **D-08:** Document the E2E test flow as a verification checklist (actual execution requires deploy):
  - Topup ₹1000 → verify 1000 credits + bonus in wallet
  - Spend 200 credits → verify balance = 800 + bonus
  - Request cash refund → verify max = (1000 - 0 - 200) = 800 (not 800 + bonus)
- **D-09:** E2E test is manual (curl-based against running server) — not automated test code

### Claude's Discretion
- Whether to add the INSERT for new wallets case in upsert_wallet (if it exists — currently only does UPDATE)
- Error handling approach for missing fields in cloud JSON

</decisions>

<specifics>
## Specific Ideas

- This is purely a Rust backend change in one file (cloud_sync.rs)
- Backward compatible — cloud may send old format without new fields, use .unwrap_or(0)
- The wallet_transactions push already includes currency_type (Phase 338 added it)
- No frontend changes in this phase

</specifics>

<canonical_refs>
## Canonical References

### Cloud sync (PRIMARY — being modified)
- `crates/racecontrol/src/cloud_sync.rs` lines 700-712 — wallet push json_object query
- `crates/racecontrol/src/cloud_sync.rs` lines 1430-1560 — upsert_wallet function
- `crates/racecontrol/src/cloud_sync.rs` lines 443-525 — process_debit_intents (verify no change needed)

### Phase 338 context (wallet column semantics)
- `.planning/phases/338-wallet-core-logic/338-CONTEXT.md` — D-05 (debits don't touch rupee/bonus)

</canonical_refs>

<code_context>
## Existing Code Insights

### Current Push Query (line 700-712)
```sql
SELECT json_object(
    'driver_id', w.driver_id, 'balance_paise', w.balance_paise,
    'total_credited_paise', w.total_credited_paise,
    'total_debited_paise', w.total_debited_paise,
    'updated_at', w.updated_at,
    'phone', d.phone, 'email', d.email
) FROM wallets w JOIN drivers d ON d.id = w.driver_id
WHERE w.updated_at > ?
```

### Current Upsert UPDATE (line 1541-1547)
```sql
UPDATE wallets SET
    balance_paise = ?,
    total_credited_paise = ?,
    total_debited_paise = ?,
    updated_at = ?
WHERE driver_id = ?
```

### Established Patterns
- `.unwrap_or(0)` for optional i64 fields from JSON — safe default
- `wallet.get("field").and_then(|v| v.as_i64()).unwrap_or(0)` — existing pattern at lines 1436-1451

</code_context>

<deferred>
## Deferred Ideas

- Actual E2E test execution requires deploy — deferred to deploy session
- Automated E2E test script — could be a future enhancement

</deferred>

---

*Phase: 342-cloud-sync-e2e*
*Context gathered: 2026-04-07*
*[auto] Analysis from direct cloud_sync.rs code inspection.*

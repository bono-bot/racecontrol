# Phase 338: Wallet Core Logic - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Update wallet.rs so top-ups track rupee deposits, bonuses track bonus credits, and cash refunds are capped at net rupee deposits. Credit refunds (game resets) continue to work as before — only touching balance_paise. New cash refund function with admin-only cap calculation.

**Depends on:** Phase 337 (DB schema columns must exist)

</domain>

<decisions>
## Implementation Decisions

### Credit Functions (credit_in_tx, credit, credit_wallet)
- **D-01:** When `txn_type` starts with `topup_` (topup_cash, topup_card, topup_upi, topup_online): also increment `rupee_deposited_paise += amount_paise`
- **D-02:** When `txn_type` is `bonus` or `adjustment`: also increment `bonus_credited_paise += amount_paise`
- **D-03:** For `refund_session` and `refund_manual` txn_types: do NOT touch rupee/bonus columns — these are credit refunds that only affect `balance_paise`
- **D-04:** All credit functions must set `currency_type` on the wallet_transaction record: 'rupee' for topup_*, 'credit' for everything else

### Debit Functions (debit_in_tx, debit, debit_wallet)
- **D-05:** Debits continue to only touch `balance_paise` and `total_debited_paise` — no change to debit logic. All spending comes from the single credits pool.
- **D-06:** Debit transactions get `currency_type = 'credit'` (all spending is in credits)

### Cash Refund (NEW function)
- **D-07:** New function `cash_refund(state, driver_id, amount_paise, staff_id, notes)` in wallet.rs
- **D-08:** Max cash refund = `rupee_deposited_paise - rupee_refunded_paise - total_debited_paise` (floor at 0)
- **D-09:** Cash refund decrements `balance_paise` (like a debit) AND increments `rupee_refunded_paise`
- **D-10:** Cash refund txn_type = `'refund_cash'` with `currency_type = 'rupee'`
- **D-11:** Cash refund requires admin/staff auth — enforced at API layer (Phase 339), not in wallet.rs. But wallet.rs must accept and store `staff_id`.
- **D-12:** Cash refund posts accounting journal: Dr. acc_wallet Cr. acc_cash/acc_bank

### Max Cash Refund Calculation
- **D-13:** New function `get_max_cash_refund(state, driver_id) -> i64` that returns the cap
- **D-14:** Formula: `rupee_deposited_paise - rupee_refunded_paise - total_debited_paise` clamped to `0..=balance_paise`
- **D-15:** This is exposed via API in Phase 339 as `max_cash_refund` field

### Existing Refund Function
- **D-16:** The existing `refund()` function stays unchanged — it's for game reset credit refunds. It calls `credit()` with txn_type `refund_session`. No rupee tracking involved.

### WalletInfo Struct Update
- **D-17:** Add `rupee_deposited_paise`, `rupee_refunded_paise`, `bonus_credited_paise` to `WalletInfo` struct in rc-common/types.rs
- **D-18:** Update `get_wallet_info()` SELECT query to fetch new columns
- **D-19:** Add `max_cash_refund` computed field to WalletInfo (calculated, not stored)

### wallet_transactions INSERT
- **D-20:** Every INSERT INTO wallet_transactions must now include `currency_type` column
- **D-21:** Determine currency_type from txn_type: topup_* and refund_cash → 'rupee', everything else → 'credit'

### Claude's Discretion
- Helper function for txn_type → currency_type mapping (inline match or separate fn)
- Whether cash_refund uses debit_in_tx internally or has its own SQL
- Error message wording for exceeding cash refund cap

</decisions>

<specifics>
## Specific Ideas

- Business rule: spending burns from the single balance_paise pool — no distinction at debit time between "rupee credits" and "bonus credits"
- Cash refund is the ONLY operation that needs to distinguish rupees from bonus — and it does so via the tracking columns, not by splitting the balance
- The `txn_type` CHECK constraint in wallet_transactions needs `'refund_cash'` added — but this is in CREATE TABLE, not ALTER TABLE. For existing DBs, app-level validation suffices (SQLite won't enforce CHECK on existing tables anyway)
- `ensure_wallet()` INSERT OR IGNORE should include new columns with DEFAULT 0 values (already handled by ALTER TABLE DEFAULT, but explicit is safer)

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Wallet functions (PRIMARY — being modified)
- `crates/racecontrol/src/wallet.rs` — ALL functions: ensure_wallet, credit_in_tx, credit, credit_wallet, debit_in_tx, debit, debit_wallet, refund, get_transactions, get_wallet_info
- `crates/rc-common/src/types.rs` lines 779-798 — WalletInfo and WalletTransaction structs

### Accounting (called from wallet functions)
- `crates/racecontrol/src/accounting.rs` — post_topup, post_bonus, post_refund, post_wallet_debit

### Phase 337 migration (prerequisite — columns must exist)
- `crates/racecontrol/src/db/mod.rs` lines 3896-3957 — ALTER TABLE statements for new columns

### Existing API refund handler (read-only — understand current refund flow)
- `crates/racecontrol/src/api/routes.rs` lines 9215-9275 — refund_wallet handler

### Business rules
- Memory: `~/.claude/projects/C--Users-bono/memory/project_credits_rupees_separation.md` — Full business model

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `credit_in_tx` / `debit_in_tx` — transaction-based wallet operations, can be extended
- `accounting::post_*` functions — double-entry journal posting for each txn_type
- The `match txn_type` pattern in `credit()` for routing to accounting functions

### Established Patterns
- All wallet mutations within SQLite transactions for atomicity
- `let _ =` for idempotent operations, `?` for fallible operations
- Separate `_in_tx` variants (caller-managed tx) and standalone variants (own tx)
- Accounting journal posted OUTSIDE the wallet transaction (wallet_transactions is source of truth)

### Integration Points
- `credit_in_tx` is called by `credit()` and by `billing.rs` for session-related credits
- `debit_in_tx` is called by `debit()` and by `billing.rs` for session debits
- `credit_wallet` / `debit_wallet` are standalone DB-only operations used by incentive system
- `cloud_sync.rs` process_debit_intents calls its own UPDATE wallets query — needs currency_type on its INSERT
- `WalletInfo` struct consumed by API routes, cloud sync, and admin dashboard

</code_context>

<deferred>
## Deferred Ideas

None — all wallet logic changes are within Phase 338 scope. API response changes (Phase 339), frontend display (340-341), and cloud sync updates (342) are explicitly later phases.

</deferred>

---

*Phase: 338-wallet-core-logic*
*Context gathered: 2026-04-07*
*[auto] All decisions derived from ROADMAP.md success criteria + codebase analysis. Business rules locked by Uday 2026-04-07.*

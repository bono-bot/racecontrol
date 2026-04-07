# Phase 337: DB Schema Migration - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Add wallet tracking columns for rupee/credit separation to SQLite schema without breaking existing functionality. Three new columns on `wallets` table, one new column on `wallet_transactions` table, backfill existing transactions with currency_type based on txn_type. Migration must be idempotent and work on both venue and cloud databases.

</domain>

<decisions>
## Implementation Decisions

### Schema Changes — wallets table
- **D-01:** Add `rupee_deposited_paise INTEGER NOT NULL DEFAULT 0` — tracks total real-money deposits
- **D-02:** Add `rupee_refunded_paise INTEGER NOT NULL DEFAULT 0` — tracks total cash refunds given
- **D-03:** Add `bonus_credited_paise INTEGER NOT NULL DEFAULT 0` — tracks total promotional credits issued
- **D-04:** `balance_paise` remains unchanged — still the single spendable credits pool. No split pools.

### Schema Changes — wallet_transactions table
- **D-05:** Add `currency_type TEXT NOT NULL DEFAULT 'credit'` — values: 'rupee' or 'credit'
- **D-06:** Default is 'credit' so existing rows are valid without explicit backfill (safe for ALTER TABLE ADD COLUMN)

### Backfill Strategy
- **D-07:** Backfill `currency_type` on existing transactions: `topup_cash`, `topup_card`, `topup_upi`, `topup_online` → 'rupee'; all others → 'credit'
- **D-08:** Backfill `rupee_deposited_paise` on wallets: SUM of positive amounts from wallet_transactions WHERE txn_type LIKE 'topup_%'
- **D-09:** Backfill `rupee_refunded_paise`: stays 0 for all existing wallets (no cash refund feature existed before)
- **D-10:** Backfill `bonus_credited_paise`: SUM of positive amounts from wallet_transactions WHERE txn_type IN ('bonus', 'adjustment')

### Migration Pattern
- **D-11:** Use `ALTER TABLE ... ADD COLUMN` with `let _ =` error suppression (same pattern as all other migrations in db/mod.rs — idempotent, SQLite ignores "duplicate column" errors)
- **D-12:** No CHECK constraint on currency_type via ALTER TABLE (SQLite limitation) — app-level enforcement only, matching the pattern used for balance_paise CHECK
- **D-13:** Backfill runs after column addition, uses UPDATE WHERE to be idempotent (re-running is safe)

### Cloud Sync
- **D-14:** Cloud DB gets migration automatically — `cloud_sync.rs` SYNC_TABLES already includes 'wallets'. New columns will be pushed/pulled after migration runs on both sides.

### Claude's Discretion
- Index strategy for new columns (likely not needed — queries filter by driver_id which is already indexed)
- Order of ALTER TABLE statements within the migrate() function
- Whether to log backfill counts at INFO or DEBUG level

</decisions>

<specifics>
## Specific Ideas

- Business rules confirmed by Uday 2026-04-07: 1 rupee = 1 credit, bonuses are promotional only, cash refund max = deposited - refunded - spent
- The existing `txn_type` CHECK constraint already distinguishes topup types from bonus/adjustment — this is the backfill key
- SQLite doesn't support ALTER TABLE ADD CHECK — so `currency_type` validation must be in Rust code, not DB constraint

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing schema and migration pattern
- `crates/racecontrol/src/db/mod.rs` lines 1308-1342 — Current wallets and wallet_transactions CREATE TABLE definitions
- `crates/racecontrol/src/db/mod.rs` lines 1377-1390 — Example ALTER TABLE pattern with `let _ =` for idempotency

### Wallet core logic (read-only context — DO NOT modify in this phase)
- `crates/racecontrol/src/wallet.rs` — All wallet operations: credit_in_tx, debit_in_tx, get_wallet_info, get_transactions
- `crates/rc-common/src/types.rs` lines 779-798 — WalletInfo and WalletTransaction structs

### Cloud sync (read-only context — verify compatibility)
- `crates/racecontrol/src/cloud_sync.rs` line 29 — SYNC_TABLES constant includes 'wallets'

### Business rules
- Memory file: `~/.claude/projects/C--Users-bono/memory/project_credits_rupees_separation.md` — Full business model, DB schema changes, debit order, accounting impact

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `db/mod.rs` migrate() function: central migration point, all schema changes go here
- ALTER TABLE pattern with `let _ = sqlx::query("ALTER TABLE ...").execute(pool).await;` — proven idempotent

### Established Patterns
- All migrations in single `migrate()` fn, no versioned migration files — SQLite + ALTER TABLE approach
- CHECK constraints in CREATE TABLE only (can't ALTER TABLE ADD CHECK in SQLite)
- `let _ =` to suppress "duplicate column" errors on re-run
- Backfill queries use UPDATE WHERE to be idempotent

### Integration Points
- `migrate()` in db/mod.rs — called at server startup, this is where new ALTER TABLE statements go
- `cloud_sync.rs` push/pull — will automatically sync new columns since 'wallets' is in SYNC_TABLES
- `wallet.rs` functions — downstream phase 338 will update these to populate new columns
- `types.rs` WalletInfo struct — downstream phase 339 will add new fields

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. All wallet logic changes (phase 338), API changes (339), frontend changes (340-341), and E2E verification (342) are explicitly scoped to later phases.

</deferred>

---

*Phase: 337-db-schema-migration*
*Context gathered: 2026-04-07*
*[auto] All decisions derived from ROADMAP.md success criteria + codebase conventions. No gray areas required user input — pure infrastructure migration.*

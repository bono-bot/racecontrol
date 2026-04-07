---
phase: 337-db-schema-migration
verified: 2026-04-07T14:30:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 337: DB Schema Migration Verification Report

**Phase Goal:** Add wallet tracking columns for rupee/credit separation to SQLite schema without breaking existing functionality. Three new columns on wallets table, one new column on wallet_transactions table, backfill existing transactions with currency_type based on txn_type. Migration must be idempotent and work on both venue and cloud databases.
**Verified:** 2026-04-07T14:30:00+05:30
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                       | Status     | Evidence                                                                                               |
|----|---------------------------------------------------------------------------------------------|------------|--------------------------------------------------------------------------------------------------------|
| 1  | wallets table has rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise columns | VERIFIED   | All three ALTER TABLE statements present at lines 3898, 3902, 3906 of db/mod.rs                       |
| 2  | wallet_transactions table has currency_type column                                          | VERIFIED   | ALTER TABLE wallet_transactions ADD COLUMN currency_type TEXT NOT NULL DEFAULT 'credit' at line 3910  |
| 3  | Existing topup transactions are backfilled with currency_type = 'rupee'                     | VERIFIED   | UPDATE wallet_transactions SET currency_type = 'rupee' WHERE txn_type IN ('topup_cash',...) at line 3917 |
| 4  | Existing bonus/adjustment transactions keep currency_type = 'credit' (DEFAULT)             | VERIFIED   | DEFAULT 'credit' on ADD COLUMN + backfill only targets topup types, all others unchanged               |
| 5  | Existing wallets have rupee_deposited_paise backfilled from topup transaction sums          | VERIFIED   | UPDATE wallets SET rupee_deposited_paise = COALESCE(SUM subquery) WHERE = 0 at line 3926              |
| 6  | Existing wallets have bonus_credited_paise backfilled from bonus/adjustment transaction sums | VERIFIED  | UPDATE wallets SET bonus_credited_paise = COALESCE(SUM subquery) WHERE = 0 at line 3939               |
| 7  | balance_paise is untouched by the migration                                                 | VERIFIED   | grep for balance_paise in lines 3896-3957 returns zero matches                                        |
| 8  | Migration is idempotent — running twice produces the same result                            | VERIFIED   | `let _ =` pattern suppresses duplicate column errors; UPDATE WHERE guards prevent double-backfill      |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact                                      | Expected                                    | Status     | Details                                                                          |
|-----------------------------------------------|---------------------------------------------|------------|----------------------------------------------------------------------------------|
| `crates/racecontrol/src/db/mod.rs`            | ALTER TABLE migrations + backfill queries   | VERIFIED   | 59 lines inserted at line 3896; contains all 4 ALTER TABLE + 3 UPDATE + log     |

**Artifact level checks:**
- Level 1 (exists): File present at expected path
- Level 2 (substantive): 59-line migration block, not a stub. Contains exact SQL from plan.
- Level 3 (wired): migrate() is called at server startup. The block is inside the migrate() function body, before the final `tracing::info!("Database migrations complete")` and `Ok(())`.

---

### Key Link Verification

| From                           | To                        | Via                       | Status   | Details                                                                                    |
|--------------------------------|---------------------------|---------------------------|----------|--------------------------------------------------------------------------------------------|
| `crates/racecontrol/src/db/mod.rs` | wallets table         | ALTER TABLE ADD COLUMN    | VERIFIED | `grep "ALTER TABLE wallets ADD COLUMN"` returns 3 matches (the 3 new columns)              |
| `crates/racecontrol/src/db/mod.rs` | wallet_transactions   | ALTER TABLE ADD COLUMN    | VERIFIED | `grep "ALTER TABLE wallet_transactions ADD COLUMN currency_type"` returns 1 match          |
| `crates/racecontrol/src/db/mod.rs` | wallets table         | UPDATE backfill           | VERIFIED | `grep "UPDATE wallets SET"` returns 2 matches (rupee_deposited + bonus_credited backfill)  |

---

### Data-Flow Trace (Level 4)

Not applicable for this phase. Phase 337 is a pure schema migration — no components render this data yet. Data-flow verification is deferred to Phase 338 (Wallet Core Logic) which will populate these columns, and Phase 342 (E2E Verify).

---

### Behavioral Spot-Checks

| Behavior                                    | Command                                                             | Result                                          | Status |
|---------------------------------------------|---------------------------------------------------------------------|-------------------------------------------------|--------|
| Binary compiles with new migration code     | `cargo check --bin racecontrol`                                     | Finished dev profile with 1 warning (unrelated) | PASS   |
| rupee_deposited_paise has 4+ references     | `grep -c "rupee_deposited_paise" db/mod.rs`                         | 4                                               | PASS   |
| rupee_refunded_paise has 2+ references      | `grep -c "rupee_refunded_paise" db/mod.rs`                          | 2                                               | PASS   |
| bonus_credited_paise has 4+ references      | `grep -c "bonus_credited_paise" db/mod.rs`                          | 4                                               | PASS   |
| currency_type has 4+ references             | `grep -c "currency_type" db/mod.rs`                                 | 4                                               | PASS   |
| wallets ALTER TABLE count = 3               | `grep "ALTER TABLE wallets ADD COLUMN" \| wc -l`                    | 3                                               | PASS   |
| wallet_transactions ALTER TABLE count = 1   | `grep "ALTER TABLE wallet_transactions ADD COLUMN currency_type" \| wc -l` | 1                                        | PASS   |
| UPDATE wallets backfill count >= 2          | `grep "UPDATE wallets SET" \| wc -l`                                | 2                                               | PASS   |
| UPDATE wallet_transactions backfill count=1 | `grep "UPDATE wallet_transactions SET currency_type" \| wc -l`      | 1                                               | PASS   |
| Log ordering: v45.0 log before "complete"   | `grep -n "Database migrations complete\|v45.0 wallet separation"`   | Line 3953 then 3955 — correct order             | PASS   |
| balance_paise NOT in new block              | grep for balance_paise in lines 3896-3957                           | 0 matches                                       | PASS   |
| Commit 1dc0ec9b exists                      | `git show 1dc0ec9b --stat`                                          | 1 file changed, 59 insertions — confirmed       | PASS   |

---

### Requirements Coverage

| Requirement | Source Plan        | Description                                                                                                  | Status    | Evidence                                                                 |
|-------------|--------------------|--------------------------------------------------------------------------------------------------------------|-----------|--------------------------------------------------------------------------|
| WAL-01      | 337-01-PLAN.md     | Add rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise to wallets. Add currency_type to wallet_transactions. Backfill from existing txn_type. | SATISFIED | All 4 columns added, all 3 backfill queries present, idempotent, cargo check passes. ROADMAP.md Phase 337 checkbox marked [x]. |

**WAL-01 source note:** WAL-01 is defined inline in ROADMAP.md line 823 (not in a separate REQUIREMENTS.md section). This is consistent with the milestone planning pattern for v45.0. No orphaned requirements found.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | —    | —       | —        | —      |

No TODO, FIXME, placeholder, stub, hardcoded empty, or incomplete implementation patterns found in the new migration block (lines 3896-3957). All SQL is fully specified with real table names, column types, constraints, and WHERE guards.

The one compiler warning (`irrefutable_let_patterns`) is pre-existing in the codebase and unrelated to Phase 337 changes.

---

### Human Verification Required

#### 1. Runtime Migration Execution

**Test:** Deploy updated `racecontrol` binary to venue server (.23) or cloud (Bono VPS), then query the database:
```sql
SELECT rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise FROM wallets LIMIT 1;
SELECT currency_type FROM wallet_transactions LIMIT 1;
SELECT COUNT(*) FROM wallet_transactions WHERE txn_type LIKE 'topup_%' AND currency_type = 'rupee';
```
**Expected:** First two queries succeed (columns exist). Third query returns > 0 if any topup transactions exist in the database.
**Why human:** Migration logic is in `migrate()` which runs at server startup. Cannot verify actual column creation without a running server connected to a real SQLite database. Cargo check only verifies compilation.

---

### Gaps Summary

No gaps. All 8 must-have truths verified at code level. The only remaining item is runtime verification (human, above), which is expected to be confirmed during Phase 342 E2E verification per the plan.

**Cloud DB coverage:** `wallets` is present in `SYNC_TABLES` in `cloud_sync.rs` line 29. The migrate() function runs on startup of both venue and cloud racecontrol binaries. Cloud database will receive the migration on next server restart on Bono VPS — this is architectural, confirmed by code inspection.

---

_Verified: 2026-04-07T14:30:00+05:30 IST_
_Verifier: Claude (gsd-verifier)_

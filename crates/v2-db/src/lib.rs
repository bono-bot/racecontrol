// v2-db — V2-native SQLite-first database substrate.
//
// PACT-20260503-003 Phase 0.2. SQLite-first per Captain D4; Postgres pivot
// triggered by criteria documented in POSTGRES-PIVOT.md.
//
// Schema lives in migrations/. Apply with `migrate(&pool).await?` on boot.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;

pub mod cirs;
pub mod customers;
pub mod lobbies;
pub mod pods;
pub mod sessions;
pub mod wallets;

pub type DbPool = sqlx::SqlitePool;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// SAFETY (sqlx Display feature-flag audit — §S-247 iter9 AUDITED-SAFE for `v2_db::Error::Sqlx`):
    /// **Scope of this comment**: `v2_db::Error::Sqlx` ONLY (the variant directly above). The
    /// sibling type `v2_db::cirs::CirsError::Sqlx` is a separate wrapper with its own reach
    /// path — see disposition note below.
    /// sqlx 0.8.6 Display does NOT expose bind values in current workspace configuration —
    /// the `log-statements` feature was removed in sqlx 0.7+ structurally (query logging
    /// migrated to tracing-based subscribers). All 3 sqlx imports verified: `crates/v2-db`
    /// (`default-features = false` + minimal feature set), `crates/racecontrol` and
    /// `crates/weekly-report` (defaults are `any, macros, migrate, derive, json` — no
    /// log-statements). For `v2_db::Error::Sqlx` specifically: 0 HTTP Display reach paths
    /// (`grep -rn "v2_db::Error\b" /root/racecontrol/crates/racecontrol/src/api/` returns
    /// 0 hits; the wrapper is used only in the `open()` function at boot).
    /// **Sibling-wrapper reach disclosure**: `v2_db::cirs::CirsError::Sqlx` (defined at
    /// `v2-db/src/cirs.rs`) IS reached via `format!("{e}")` at `cirs_lookup.rs:294` →
    /// HTTP response "message" field at line 312. That path's risk is bounded — sqlx 0.8.6
    /// Display does not surface bind values (log-statements removed); SQLite driver Display
    /// content is structural metadata (table names, column names, SQL error codes), not
    /// bind values. The intentional-redaction comment at `cirs_lookup.rs:288-293` documents
    /// PII fingerprint redaction for the InvalidPhone + AmbiguousPhone variants via the
    /// CirsError Display impl (§S-241 iter5 fix at `cirs.rs:23-36`); the SQLX variant case
    /// relies on sqlx Display safety rather than the explicit fingerprint mechanism. Future
    /// audit class: SQLite error Display content fragments under adversarial query injection
    /// (separate from feature-flag class; out of §S-247 scope; track via §S-146 RCA if
    /// elevated).
    /// Future caller-site contract for `v2_db::Error::Sqlx` (this variant): any new handler
    /// matching on `Error::Sqlx` MUST NOT use `format!("{e}")` in JSON response — return
    /// generic internal_error per `cirs_lookup.rs:284` pattern.
    /// Audit trail: §S-241 Agent-2 recommendation #4; §S-247 iter9 verdict; MAOR Tier-1
    /// Finding #1 DISPOSITIONED-INLINE — initial draft conflated `v2_db::Error::Sqlx` audit
    /// scope with sibling-wrapper `CirsError::Sqlx` reach path (same G9 class as iter7
    /// wrong-struct-import sub-class `multi-defined-similar-type-incomplete-grep`); this
    /// revision separates the two wrapper types explicitly.
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// Open a SQLite pool at `path`. Creates the file if missing. WAL journal mode
/// for concurrent readers + a single writer (the SQLite-first concurrency floor
/// that POSTGRES-PIVOT.md flags as the >2-writer trigger).
pub async fn open(path: &str) -> Result<DbPool, Error> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

/// Apply all migrations in `migrations/` (compiled into the binary).
pub async fn migrate(pool: &DbPool) -> Result<(), Error> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_and_migrate_in_memory() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        // Smoke: customers table exists and is empty.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customers")
            .fetch_one(&pool)
            .await
            .expect("count customers");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn audit_trail_never_delete_triggers_block_topup_deletion() {
        // Addresses MMA VERIFY P0 (Grok adversarial 2026-05-03):
        // 8-year retention is structurally enforced by BEFORE DELETE triggers
        // on wallet_topups + wallet_redemptions that RAISE(ABORT).
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        // PACT-018: staff_id has FK to staff(id) — seed staff row first.
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-1', 'Test Staff', '0000000001', '0001', 'cashier', 1)",
        )
        .execute(&pool).await.expect("seed staff");

        // Seed minimum FK chain: customer + wallet + topup row.
        sqlx::query(
            "INSERT INTO customers (id, phone) VALUES ('cust-1', '+919999999999')",
        )
        .execute(&pool).await.expect("seed customer");
        sqlx::query(
            "INSERT INTO wallets (id, customer_id, balance_credits) VALUES ('w-1', 'cust-1', 1000)",
        )
        .execute(&pool).await.expect("seed wallet");
        sqlx::query(
            "INSERT INTO wallet_topups (id, wallet_id, credits_purchased, gst_collected_paise, \
             amount_paid_paise, payment_method, staff_id, pos_id, tax_invoice_no) \
             VALUES ('t-1', 'w-1', 1000, 18000, 118000, 'cash', 's-1', 'pos-1', 'INV-001')",
        )
        .execute(&pool).await.expect("seed topup");

        // Attempt deletion → trigger aborts.
        let res = sqlx::query("DELETE FROM wallet_topups WHERE id = 't-1'")
            .execute(&pool)
            .await;
        assert!(res.is_err(), "DELETE on wallet_topups must be blocked by trigger");

        // Row still present.
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallet_topups")
            .fetch_one(&pool).await.expect("count topups");
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn pact018_staff_fk_on_delete_restrict_blocks_orphan_creation() {
        // Addresses bono PR #58 review test-gap observation: "no test for FK
        // ON DELETE RESTRICT enforcement on wallets→customers, sessions→
        // customers/pods, wallet_redemptions→sessions". This test exercises
        // PACT-018's new staff_id FK on wallet_topups specifically.
        //
        // Two assertions:
        //   (a) INSERT with non-existent staff_id is blocked at FK-check time
        //   (b) DELETE FROM staff with referencing rows is blocked by RESTRICT
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        // (a) FK INSERT-time enforcement — try to insert wallet_topup with
        // staff_id that doesn't exist in staff table.
        sqlx::query(
            "INSERT INTO customers (id, phone) VALUES ('cust-fk', '+919999999998')",
        )
        .execute(&pool).await.expect("seed customer");
        sqlx::query(
            "INSERT INTO wallets (id, customer_id, balance_credits) VALUES ('w-fk', 'cust-fk', 0)",
        )
        .execute(&pool).await.expect("seed wallet");
        let bad_insert = sqlx::query(
            "INSERT INTO wallet_topups (id, wallet_id, credits_purchased, gst_collected_paise, \
             amount_paid_paise, payment_method, staff_id, pos_id, tax_invoice_no) \
             VALUES ('t-fk', 'w-fk', 100, 1800, 11800, 'cash', 'nonexistent-staff', 'pos-1', 'INV-FK')",
        )
        .execute(&pool).await;
        assert!(
            bad_insert.is_err(),
            "INSERT with nonexistent staff_id must fail FK constraint"
        );

        // (b) FK DELETE-time RESTRICT — seed valid staff + topup, then attempt
        // to delete staff row that's referenced by a wallet_topup.
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-fk', 'FK Test', '0000000099', '0099', 'cashier', 1)",
        )
        .execute(&pool).await.expect("seed staff");
        sqlx::query(
            "INSERT INTO wallet_topups (id, wallet_id, credits_purchased, gst_collected_paise, \
             amount_paid_paise, payment_method, staff_id, pos_id, tax_invoice_no) \
             VALUES ('t-fk2', 'w-fk', 200, 3600, 23600, 'upi', 's-fk', 'pos-1', 'INV-FK2')",
        )
        .execute(&pool).await.expect("seed topup with staff");
        let bad_delete = sqlx::query("DELETE FROM staff WHERE id = 's-fk'")
            .execute(&pool).await;
        assert!(
            bad_delete.is_err(),
            "DELETE FROM staff with referencing wallet_topups must be blocked by ON DELETE RESTRICT"
        );

        // Staff row still present (RESTRICT held).
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM staff WHERE id = 's-fk'")
            .fetch_one(&pool).await.expect("count staff");
        assert_eq!(n, 1, "staff row must remain after blocked DELETE");
    }

    #[tokio::test]
    async fn pact001_cirs_lookup_audit_table_smoke() {
        // Phase 0 substrate landing: cirs_lookup_audit table exists and is
        // empty after migration apply.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cirs_lookup_audit")
            .fetch_one(&pool)
            .await
            .expect("count cirs_lookup_audit");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn pact001_cirs_lookup_audit_staff_fk_blocks_orphan_insert() {
        // §3.2 staff_id FK to staff(id) — INSERT with bogus staff_id must fail.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        let bad = sqlx::query(
            "INSERT INTO cirs_lookup_audit (staff_id, input_method, result) \
             VALUES ('nonexistent-staff', 'phone', 'not_found')",
        )
        .execute(&pool)
        .await;
        assert!(
            bad.is_err(),
            "INSERT into cirs_lookup_audit with nonexistent staff_id must fail FK constraint"
        );
    }

    #[tokio::test]
    async fn pact001_cirs_record_lookup_helper_round_trip() {
        // record_lookup() inserts a row with hashed input + correct enum values.
        use crate::cirs::{record_lookup, LookupInput, LookupResult};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-cirs', 'CIRS Test Staff', '0000000050', '0050', 'cashier', 1)",
        )
        .execute(&pool)
        .await
        .expect("seed staff");

        let input = LookupInput::Phone { phone: "+919999999911".to_string() };
        record_lookup(&pool, "s-cirs", None, &input, &LookupResult::NotFound)
            .await
            .expect("record miss");

        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cirs_lookup_audit \
             WHERE staff_id = 's-cirs' AND input_method = 'phone' AND result = 'not_found'",
        )
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(n, 1);

        let hash: Option<String> = sqlx::query_scalar(
            "SELECT input_hash FROM cirs_lookup_audit WHERE staff_id = 's-cirs' LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("hash row");
        let h = hash.expect("hash present for phone input");
        assert_eq!(h.len(), 64, "SHA256 hex is 64 chars");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));

        // Walk-in input writes NULL hash + NULL customer_id (no PII to hash,
        // no customer record yet — fallback-account binding lands Phase 1).
        record_lookup(
            &pool,
            "s-cirs",
            None,
            &LookupInput::WalkInGuestId { guest_id: 1 },
            &LookupResult::NotFound,
        )
        .await
        .expect("walk-in audit row inserts with NULL hash + NULL customer_id");

        let walkin_hash: Option<String> = sqlx::query_scalar(
            "SELECT input_hash FROM cirs_lookup_audit \
             WHERE input_method = 'walk_in_guest_id' LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("walk-in row");
        assert!(walkin_hash.is_none(), "walk-in input has no PII to hash");
    }

    #[tokio::test]
    async fn pact001_cirs_lookup_by_phone_finds_existing_customer() {
        // M1 phone-lookup: canonical AND 10-digit auto-91 forms both hit
        // idx_customers_phone; missing returns NotFound.
        use crate::cirs::{lookup_by_phone, LookupResult};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        sqlx::query("INSERT INTO customers (id, phone) VALUES ('c-1', '+919999999500')")
            .execute(&pool)
            .await
            .expect("seed customer");

        let r = lookup_by_phone(&pool, "+919999999500").await.expect("lookup");
        match r {
            LookupResult::Found { customer_id } => assert_eq!(customer_id, "c-1"),
            other => panic!("expected Found, got {:?}", other),
        }

        let r2 = lookup_by_phone(&pool, "9999999500").await.expect("lookup-10d");
        assert!(matches!(r2, LookupResult::Found { .. }));

        let r3 = lookup_by_phone(&pool, "+919999999501").await.expect("lookup-miss");
        assert_eq!(r3, LookupResult::NotFound);
    }

    #[tokio::test]
    async fn pact018_staff_partial_unique_indexes_allow_inactive_orphan_collisions() {
        // PACT-018 §2.5 / CAVEAT-3 enforcement: backfill orphans use
        // phone='0000000000' + pin='0000' placeholder. The partial UNIQUE
        // indexes (WHERE active=1) MUST allow multiple inactive rows with
        // identical placeholder phone/pin (otherwise backfill-of-orphans
        // for >1 staff_id would collide).
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        // Two inactive orphan rows with same placeholder phone + pin: must succeed.
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('orphan-1', '<unknown>', '0000000000', '0000', 'inactive', 0)",
        )
        .execute(&pool).await.expect("seed orphan-1");
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('orphan-2', '<unknown>', '0000000000', '0000', 'inactive', 0)",
        )
        .execute(&pool).await.expect("seed orphan-2");

        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM staff WHERE phone = '0000000000' AND active = 0",
        )
        .fetch_one(&pool).await.expect("count orphans");
        assert_eq!(n, 2, "two inactive orphans with same placeholder must coexist");

        // But two ACTIVE rows with same phone must collide (partial UNIQUE
        // applies WHERE active=1).
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('alice-1', 'Alice', '+919876543210', '1234', 'cashier', 1)",
        )
        .execute(&pool).await.expect("seed alice-1");
        let dup_active = sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('alice-2', 'Alice2', '+919876543210', '5678', 'cashier', 1)",
        )
        .execute(&pool).await;
        assert!(
            dup_active.is_err(),
            "duplicate phone among active rows must be blocked by partial UNIQUE"
        );
    }

    #[tokio::test]
    async fn pact001_cirs_lookup_audit_customer_fk_blocks_orphan_insert() {
        // NF-james-A coverage: §3.2 customer_id FK to customers(id) should
        // reject inserts with a non-NULL bogus customer_id at FK-check time.
        // Symmetric to pact001_cirs_lookup_audit_staff_fk_blocks_orphan_insert.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");

        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-cust-fk', 'CIRS Cust FK Test', '0000000077', '0077', 'cashier', 1)",
        )
        .execute(&pool).await.expect("seed staff");

        let bad = sqlx::query(
            "INSERT INTO cirs_lookup_audit (staff_id, customer_id, input_method, result) \
             VALUES ('s-cust-fk', 'nonexistent-customer', 'phone', 'found')",
        )
        .execute(&pool).await;
        assert!(bad.is_err(),
            "INSERT into cirs_lookup_audit with nonexistent customer_id must fail FK constraint");

        // Sanity: NULL customer_id still works (walk-in / miss path).
        sqlx::query(
            "INSERT INTO cirs_lookup_audit (staff_id, customer_id, input_method, result) \
             VALUES ('s-cust-fk', NULL, 'phone', 'not_found')",
        )
        .execute(&pool).await.expect("NULL customer_id should be accepted");

        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cirs_lookup_audit WHERE customer_id IS NULL",
        )
        .fetch_one(&pool).await.expect("count");
        assert_eq!(n, 1, "NULL customer_id row must persist alongside FK rejection");
    }
}

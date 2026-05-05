// v2-db — V2-native SQLite-first database substrate.
//
// PACT-20260503-003 Phase 0.2. SQLite-first per Captain D4; Postgres pivot
// triggered by criteria documented in POSTGRES-PIVOT.md.
//
// Schema lives in migrations/. Apply with `migrate(&pool).await?` on boot.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;

pub mod customers;
pub mod lobbies;
pub mod pods;
pub mod sessions;
pub mod wallets;

pub type DbPool = sqlx::SqlitePool;

#[derive(Debug, thiserror::Error)]
pub enum Error {
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
}

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
}

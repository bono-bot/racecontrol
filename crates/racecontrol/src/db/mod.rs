use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

mod migrate_core;
mod migrate_core_columns;
mod migrate_billing;
mod migrate_game;
mod migrate_kiosk;
mod migrate_social;
mod migrate_marketing;
mod migrate_staff;
mod migrate_gamification;
mod migrate_cafe;
mod migrate_ops;
mod migrate_policy;
mod migrate_config;
mod migrate_cross_domain;
mod migrate_pii;

#[cfg(test)]
mod tests;

// Re-export public API from extracted modules
pub use migrate_pii::{check_pin_rotation, migrate_pii_encryption};

pub async fn init_pool(db_path: &str) -> anyhow::Result<SqlitePool> {
    // Ensure the parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = format!("sqlite:{}?mode=rwc", db_path);
    // RESIL-02: Pool sized for concurrent readers (dashboard, fleet health, leaderboard,
    // cloud sync) alongside the single SQLite writer. 10 connections = headroom for 8 pods'
    // dashboard queries + admin + POS without pool exhaustion. Writes are still serialized
    // by SQLite's single-writer — more connections help reads, not writes.
    //
    // C1 (ops-audit 2026-04-16): PRAGMA foreign_keys is PER-CONNECTION in SQLite. Running it
    // once on the pool only affects one connection; the other 9 pool connections silently
    // operate with FK OFF. Fix: after_connect hook runs the PRAGMAs on every new connection
    // the pool opens. Journal mode / busy timeout / sync are also per-connection and repeated
    // here for defense-in-depth, even though journal_mode=WAL is database-global once set.
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .max_lifetime(std::time::Duration::from_secs(300))
        .after_connect(|conn, _meta| Box::pin(async move {
            sqlx::query("PRAGMA foreign_keys=ON").execute(&mut *conn).await?;
            sqlx::query("PRAGMA journal_mode=WAL").execute(&mut *conn).await?;
            sqlx::query("PRAGMA busy_timeout=5000").execute(&mut *conn).await?;
            sqlx::query("PRAGMA synchronous=NORMAL").execute(&mut *conn).await?;
            // P2: Performance PRAGMAs — server has 64GB RAM, these use ~288MB
            sqlx::query("PRAGMA cache_size=-32000").execute(&mut *conn).await?;   // 32MB page cache (default ~2MB)
            sqlx::query("PRAGMA mmap_size=268435456").execute(&mut *conn).await?; // 256MB memory-mapped reads
            sqlx::query("PRAGMA temp_store=memory").execute(&mut *conn).await?;   // temp tables in RAM
            Ok(())
        }))
        .connect(&url)
        .await?;

    // Enable WAL mode — allows concurrent readers + single writer (vs default rollback journal
    // which blocks ALL reads during writes). This prevents debug_activity SELECT queries from
    // hanging when billing/WS handlers hold a write transaction.
    // busy_timeout gives SQLite 5s to retry instead of returning SQLITE_BUSY immediately.
    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
    sqlx::query("PRAGMA busy_timeout=5000").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await?;

    // RESIL-01: Verify WAL mode actually activated — fail-fast if not.
    // On a read-only filesystem or corrupted DB, PRAGMA journal_mode=WAL silently falls back
    // to DELETE mode. This bail! ensures the server will NOT start in that state.
    let wal_check: (String,) = sqlx::query_as("PRAGMA journal_mode").fetch_one(&pool).await?;
    if wal_check.0 != "wal" {
        anyhow::bail!("CRITICAL: SQLite WAL mode failed to activate — got '{}'. Cannot proceed safely with concurrent writes.", wal_check.0);
    }
    tracing::info!("SQLite WAL mode VERIFIED active (busy_timeout=5000ms, synchronous=NORMAL)");

    // Run migrations
    migrate(&pool).await?;

    tracing::info!("Database initialized at {}", db_path);
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await?;
    sqlx::query("PRAGMA foreign_keys=ON").execute(pool).await?;
    sqlx::query("PRAGMA wal_autocheckpoint=400").execute(pool).await?;
    sqlx::query("PRAGMA busy_timeout=5000").execute(pool).await?;

    // ─── Domain-specific migrations (FK-safe order) ───────────────────────
    migrate_core::migrate_core(pool).await?;
    migrate_billing::migrate_billing(pool).await?;
    migrate_game::migrate_game(pool).await?;
    migrate_kiosk::migrate_kiosk(pool).await?;
    migrate_social::migrate_social(pool).await?;
    migrate_marketing::migrate_marketing(pool).await?;
    migrate_staff::migrate_staff(pool).await?;
    migrate_gamification::migrate_gamification(pool).await?;
    migrate_cafe::migrate_cafe(pool).await?;
    migrate_ops::migrate_ops(pool).await?;
    migrate_policy::migrate_policy(pool).await?;
    migrate_config::migrate_config(pool).await?;

    // ─── Cross-domain migrations ──────────────────────────────────────────
    migrate_cross_domain::migrate_cross_domain(pool).await?;

    Ok(())
}

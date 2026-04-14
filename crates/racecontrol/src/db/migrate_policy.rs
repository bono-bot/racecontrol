//! Database migrations: policy domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_policy(pool: &SqlitePool) -> anyhow::Result<()> {
    // metrics_rollups column additions (SYNC-03): enable LWW conflict resolution for rollups
    // SQLite does not support IF NOT EXISTS on ALTER TABLE — use let _ = ignore pattern
    let _ = sqlx::query("ALTER TABLE metrics_rollups ADD COLUMN updated_at TEXT DEFAULT (datetime('now'))")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE metrics_rollups ADD COLUMN venue_id TEXT")
        .execute(pool)
        .await;


    // ─── Audit Log (tracks all config changes) ───────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id TEXT PRIMARY KEY,
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('create', 'update', 'delete')),
            old_values TEXT,
            new_values TEXT,
            staff_id TEXT,
            ip_address TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_table ON audit_log(table_name)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_row ON audit_log(table_name, row_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_at)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_staff ON audit_log(staff_id)")
        .execute(pool)
        .await?;


    // ─── audit_log: add action_type column (Phase 80 migration) ─────────────
    {
        let has_col: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pragma_table_info('audit_log') WHERE name = 'action_type'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0) > 0;

        if !has_col {
            sqlx::query("ALTER TABLE audit_log ADD COLUMN action_type TEXT")
                .execute(pool)
                .await?;
        }
    }

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_action_type ON audit_log(action_type)")
        .execute(pool)
        .await?;


    // ─── LEGAL-08: Data retention configuration ──────────────────────────────
    // data_retention_config: single-row config table seeded with DPDP Act defaults.
    // financial_records_years = 8: Income Tax Act requires 8-year financial record retention.
    // pii_inactive_months = 24: DPDP Act data minimization — anonymize PII after 2 years of inactivity.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS data_retention_config (
            id TEXT PRIMARY KEY,
            financial_records_years INTEGER NOT NULL DEFAULT 8,
            pii_inactive_months INTEGER NOT NULL DEFAULT 24,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("INSERT OR IGNORE INTO data_retention_config (id) VALUES ('default')")
        .execute(pool)
        .await?;


    // ─── Phase 299: Policy Rules Engine ─────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS policy_rules (
            id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
            name        TEXT NOT NULL,
            metric      TEXT NOT NULL,
            condition   TEXT NOT NULL CHECK(condition IN ('gt','lt','eq')),
            threshold   REAL NOT NULL,
            action      TEXT NOT NULL CHECK(action IN ('alert','config_change','flag_toggle','budget_adjust')),
            action_params TEXT NOT NULL DEFAULT '{}',
            enabled     INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            last_fired  TEXT,
            eval_count  INTEGER NOT NULL DEFAULT 0
        )"
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS policy_eval_log (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id     TEXT NOT NULL,
            rule_name   TEXT NOT NULL,
            fired       INTEGER NOT NULL,
            metric_value REAL NOT NULL,
            action_taken TEXT NOT NULL,
            evaluated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;


    // GLD-C-04 D-12: lap_rejections table.
    // Intentional: `session_id` column holds the billing_session_id value at runtime,
    // consistent with laps.session_id which also holds billing_session_id (per 363-RESEARCH.md
    // Open Question 1). Column name matches CONTEXT.md D-12 schema spec exactly.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS lap_rejections (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            lap_number INTEGER NOT NULL,
            rejected_at TEXT NOT NULL DEFAULT (datetime('now')),
            reason TEXT,
            grace_window_caught BOOLEAN NOT NULL DEFAULT 0,
            venue_id TEXT
        )",
    )
    .execute(pool)
    .await?;

    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_lap_rejections_session ON lap_rejections(session_id)",
    )
    .execute(pool)
    .await;


    Ok(())
}

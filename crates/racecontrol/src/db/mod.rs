use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use crate::crypto::encryption::FieldCipher;

mod migrate_core;
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
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .max_lifetime(std::time::Duration::from_secs(300))
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

    // Add updated_at column to multiple tables (cross-domain, idempotent)
    for table in &[
        "drivers", "wallets", "billing_sessions", "pricing_tiers",
        "kiosk_experiences", "reservations", "debit_intents",
        "kiosk_settings", "cafe_orders", "staff_members",
        "fleet_solutions", "metrics_rollups", "model_evaluations",
    ] {
        let _ = sqlx::query(&format!("ALTER TABLE {} ADD COLUMN updated_at TEXT", table))
            .execute(pool)
            .await;
    }

    // Backfill NULL updated_at with created_at where available
    let _ = sqlx::query("UPDATE drivers SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE billing_sessions SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE pricing_tiers SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE kiosk_experiences SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;

    // Sync indexes for updated_at
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_updated ON drivers(updated_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallets_updated ON wallets(updated_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_pricing_tiers_updated ON pricing_tiers(updated_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_kiosk_exp_updated ON kiosk_experiences(updated_at)")
        .execute(pool)
        .await?;

    // ─── Customer display ID ────────────────────────────────────────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN customer_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_customer_id ON drivers(customer_id)")
        .execute(pool)
        .await;

    // Backfill customer_id for existing drivers that don't have one
    let unassigned = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE customer_id IS NULL ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;

    if !unassigned.is_empty() {
        // Find the current max customer_id number
        let max_num = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT MAX(customer_id) FROM drivers WHERE customer_id IS NOT NULL",
        )
        .fetch_one(pool)
        .await?
        .0
        .and_then(|s| s.strip_prefix("RP").and_then(|n| n.parse::<u32>().ok()))
        .unwrap_or(0);

        for (i, (id,)) in unassigned.iter().enumerate() {
            let cid = format!("RP{:03}", max_num + 1 + i as u32);
            let _ = sqlx::query("UPDATE drivers SET customer_id = ? WHERE id = ?")
                .bind(&cid)
                .bind(id)
                .execute(pool)
                .await;
        }
        tracing::info!("Backfilled {} customer IDs", unassigned.len());
    }

    // Employee flag
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN is_employee BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    // ─── External module migrations ──────────────────────────────────────────
    // v26.0 Meshed Intelligence tables
    crate::fleet_kb::migrate(pool).await?;

    // v35.0 Model Evaluation Store (EVAL-03)
    crate::fleet_kb::migrate_eval_store(pool).await?;

    // v35.0 Model Reputation Store (MREP-04)
    crate::fleet_kb::migrate_reputation_store(pool).await?;

    // ─── Phase 303: venue_id column on all major operational tables ──────────
    for table in &[
        "billing_sessions", "billing_events", "billing_audit_log",
        "wallet_transactions", "wallets", "refunds", "invoices",
        "auth_tokens", "drivers", "laps", "sessions",
        "reservations", "debit_intents", "cafe_orders",
        "kiosk_experiences", "events", "event_entries",
        "hotlap_events", "hotlap_event_entries", "championships",
        "championship_standings", "championship_rounds",
        "tournaments", "tournament_registrations", "tournament_matches",
        "driver_ratings", "personal_bests", "track_records",
        "bookings", "group_sessions", "group_session_members",
        "coupon_redemptions", "pod_activity_log",
        "game_launch_events", "launch_events", "recovery_events",
        "billing_accuracy_events", "dispute_requests",
        "session_feedback", "memberships", "pod_reservations",
        "game_launch_requests", "system_events", "split_sessions",
        "virtual_queue", "review_nudges", "multiplayer_results",
        "pods",
    ] {
        let _ = sqlx::query(&format!(
            "ALTER TABLE {} ADD COLUMN venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'",
            table
        ))
        .execute(pool)
        .await;
    }

    // ─── Linked racers: parent account can add up to 3 guest racers ────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN linked_to TEXT REFERENCES drivers(id)")
        .execute(pool)
        .await;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_linked_to ON drivers(linked_to)")
        .execute(pool)
        .await?;

    // Wallet owner for billing sessions — tracks who gets refunded (parent for linked racers)
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN wallet_owner_id TEXT")
        .execute(pool)
        .await;

    // Acts 1-4: Bonus tracking flags on drivers (one-time per customer)
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN review_bonus_claimed BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN follow_bonus_claimed BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN registration_bonus_credited BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    // Acts 1-4: Link billing sessions to visits
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN visit_id TEXT REFERENCES visits(id)")
        .execute(pool)
        .await;

    // Acts 1-4: Per-minute hold tracking on billing sessions
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN hold_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    // ─── Leaderboard sim_type migration (Phase 88) ──────────────────────────
    migrate_leaderboard_sim_type(pool).await?;

    Ok(())
}

/// Idempotent migration: add `sim_type` column to personal_bests and track_records
/// and rebuild their PRIMARY KEYs to include sim_type.
///
/// SQLite does not support ALTER PRIMARY KEY, so we use the v2-table rebuild pattern.
/// The migration is guarded by a pragma_table_info check — it runs exactly once.
///
/// Default sim_type for existing rows: 'assettoCorsa'
/// (matching `format!("{:?}", SimType::AssettoCorsa).to_lowercase()` stored in laps.sim_type)
async fn migrate_leaderboard_sim_type(pool: &SqlitePool) -> anyhow::Result<()> {
    // Check if personal_bests already has sim_type column — if so, migration already done
    let pb_col_exists: bool = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pragma_table_info('personal_bests') WHERE name = 'sim_type'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0) > 0;

    if !pb_col_exists {
        tracing::info!("Phase 88: Migrating personal_bests to add sim_type to PRIMARY KEY");

        // Create new table with sim_type in PK
        sqlx::query(
            "CREATE TABLE personal_bests_v2 (
                driver_id TEXT REFERENCES drivers(id),
                track TEXT NOT NULL,
                car TEXT NOT NULL,
                sim_type TEXT NOT NULL DEFAULT 'assettoCorsa',
                best_lap_ms INTEGER NOT NULL,
                lap_id TEXT REFERENCES laps(id),
                achieved_at TEXT,
                PRIMARY KEY (driver_id, track, car, sim_type)
            )"
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("create personal_bests_v2: {}", e))?;

        // Copy existing rows, assigning 'assettoCorsa' as sim_type
        sqlx::query(
            "INSERT INTO personal_bests_v2 (driver_id, track, car, sim_type, best_lap_ms, lap_id, achieved_at)
             SELECT driver_id, track, car, 'assettoCorsa', best_lap_ms, lap_id, achieved_at
             FROM personal_bests"
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("copy personal_bests: {}", e))?;

        sqlx::query("DROP TABLE IF EXISTS personal_bests")
            .execute(pool)
            .await
            .map_err(|e| anyhow::anyhow!("drop personal_bests: {}", e))?;

        // Only rename if personal_bests_v2 exists and personal_bests doesn't
        let v2_exists: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='personal_bests_v2'"
        ).fetch_one(pool).await.unwrap_or(0) > 0;
        if v2_exists {
            sqlx::query("ALTER TABLE personal_bests_v2 RENAME TO personal_bests")
                .execute(pool)
                .await
                .map_err(|e| anyhow::anyhow!("rename personal_bests_v2: {}", e))?;
        }

        tracing::info!("Phase 88: personal_bests migration complete");
    }

    // Check if track_records already has sim_type column
    let tr_col_exists: bool = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pragma_table_info('track_records') WHERE name = 'sim_type'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0) > 0;

    if !tr_col_exists {
        tracing::info!("Phase 88: Migrating track_records to add sim_type to PRIMARY KEY");

        sqlx::query(
            "CREATE TABLE track_records_v2 (
                track TEXT NOT NULL,
                car TEXT NOT NULL,
                sim_type TEXT NOT NULL DEFAULT 'assettoCorsa',
                driver_id TEXT REFERENCES drivers(id),
                best_lap_ms INTEGER NOT NULL,
                lap_id TEXT REFERENCES laps(id),
                achieved_at TEXT,
                PRIMARY KEY (track, car, sim_type)
            )"
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("create track_records_v2: {}", e))?;

        sqlx::query(
            "INSERT INTO track_records_v2 (track, car, sim_type, driver_id, best_lap_ms, lap_id, achieved_at)
             SELECT track, car, 'assettoCorsa', driver_id, best_lap_ms, lap_id, achieved_at
             FROM track_records"
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("copy track_records: {}", e))?;

        sqlx::query("DROP TABLE IF EXISTS track_records")
            .execute(pool)
            .await
            .map_err(|e| anyhow::anyhow!("drop track_records: {}", e))?;

        let v2_exists: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='track_records_v2'"
        ).fetch_one(pool).await.unwrap_or(0) > 0;
        if v2_exists {
            sqlx::query("ALTER TABLE track_records_v2 RENAME TO track_records")
                .execute(pool)
                .await
                .map_err(|e| anyhow::anyhow!("rename track_records_v2: {}", e))?;
        }

        tracing::info!("Phase 88: track_records migration complete");
    }

    // Phase 285: TSDB tables for time-series metrics ring buffer
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS metrics_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_name TEXT NOT NULL,
            pod_id TEXT,
            value REAL NOT NULL,
            recorded_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_metrics_samples_lookup
         ON metrics_samples(metric_name, recorded_at)"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS metrics_rollups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            resolution TEXT NOT NULL CHECK(resolution IN ('hourly', 'daily')),
            metric_name TEXT NOT NULL,
            pod_id TEXT,
            min_value REAL NOT NULL,
            max_value REAL NOT NULL,
            avg_value REAL NOT NULL,
            sample_count INTEGER NOT NULL,
            period_start TEXT NOT NULL,
            UNIQUE(resolution, metric_name, pod_id, period_start)
        )"
    ).execute(pool).await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_metrics_rollups_lookup
         ON metrics_rollups(resolution, metric_name, period_start)"
    ).execute(pool).await?;

    tracing::info!("Phase 285: metrics TSDB tables migration complete");

    Ok(())
}

/// Track the admin PIN hash in system_settings for rotation alerting.
/// Called at startup after migrations. Records the SHA-256 of the current
/// admin_pin_hash so the alerter can detect when it was last changed.
pub async fn check_pin_rotation(
    pool: &SqlitePool,
    config: &crate::config::Config,
) -> Option<String> {
    let pin_hash = match config.auth.admin_pin_hash.as_deref() {
        Some(h) if !h.is_empty() => h,
        _ => {
            tracing::debug!("No admin_pin_hash configured, skipping PIN rotation tracking");
            return None;
        }
    };

    // Hash the current admin_pin_hash to detect changes without storing the actual hash
    use sha2::Digest;
    let current_hash = hex::encode(sha2::Sha256::digest(pin_hash.as_bytes()));

    // Check existing record
    let existing: Option<(String, String)> = sqlx::query_as(
        "SELECT value, updated_at FROM system_settings WHERE key = 'admin_pin_hash_sha256'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match existing {
        Some((stored_hash, updated_at)) if stored_hash == current_hash => {
            // PIN unchanged since last recorded
            tracing::info!(
                "Admin PIN unchanged since {}",
                &updated_at
            );
            Some(updated_at)
        }
        _ => {
            // PIN changed (or first run) -- upsert new hash with current timestamp
            if let Err(e) = sqlx::query(
                "INSERT INTO system_settings (key, value, updated_at) VALUES ('admin_pin_hash_sha256', ?, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            )
            .bind(&current_hash)
            .execute(pool)
            .await
            {
                tracing::error!("Failed to upsert admin_pin_hash_sha256: {}", e);
                return None;
            }

            let action = if existing.is_some() { "rotated" } else { "recorded" };
            tracing::info!("Admin PIN hash {} in system_settings", action);

            // Return the new updated_at
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT updated_at FROM system_settings WHERE key = 'admin_pin_hash_sha256'",
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            row.map(|r| r.0)
        }
    }
}

/// Migrate existing plaintext PII to encrypted columns.
/// Idempotent: rows with phone_hash already set are skipped.
/// Processes in batches of 100 rows per transaction.
pub async fn migrate_pii_encryption(db: &SqlitePool, cipher: &FieldCipher) -> Result<(), String> {
    // Count rows needing migration
    let rows: Vec<(String, Option<String>, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, phone, email, name, guardian_phone FROM drivers \
             WHERE phone_hash IS NULL AND phone IS NOT NULL"
        )
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to query drivers for PII migration: {e}"))?;

    if rows.is_empty() {
        tracing::info!("PII migration phase 1: no plaintext phones to encrypt");
    } else {
        let total = rows.len();
        let mut migrated = 0usize;

        for chunk in rows.chunks(100) {
            let mut tx = db.begin().await.map_err(|e| format!("Failed to begin transaction: {e}"))?;

            for (id, phone, email, name, guardian_phone) in chunk {
                let phone = match phone {
                    Some(p) if !p.is_empty() => p,
                    _ => continue,
                };

                let phone_hash = cipher.hash_phone(phone);
                let phone_enc = cipher.encrypt_field(phone).map_err(|e| format!("encrypt phone: {e}"))?;

                let email_enc: Option<String> = match email {
                    Some(e) if !e.is_empty() => Some(cipher.encrypt_field(e).map_err(|er| format!("encrypt email: {er}"))?),
                    _ => None,
                };

                let name_enc: Option<String> = match name.as_str() {
                    n if !n.is_empty() => Some(cipher.encrypt_field(n).map_err(|er| format!("encrypt name: {er}"))?),
                    _ => None,
                };

                let (gp_hash, gp_enc): (Option<String>, Option<String>) = match guardian_phone {
                    Some(gp) if !gp.is_empty() => {
                        let h = cipher.hash_phone(gp);
                        let e = cipher.encrypt_field(gp).map_err(|er| format!("encrypt guardian_phone: {er}"))?;
                        (Some(h), Some(e))
                    }
                    _ => (None, None),
                };

                sqlx::query(
                    "UPDATE drivers SET phone_hash=?, phone_enc=?, email_enc=?, name_enc=?, \
                     guardian_phone_hash=?, guardian_phone_enc=?, phone=NULL, email=NULL, \
                     guardian_phone=NULL WHERE id=?"
                )
                .bind(&phone_hash)
                .bind(&phone_enc)
                .bind(&email_enc)
                .bind(&name_enc)
                .bind(&gp_hash)
                .bind(&gp_enc)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update driver {id}: {e}"))?;

                migrated += 1;
            }

            tx.commit().await.map_err(|e| format!("Failed to commit transaction: {e}"))?;
        }

        tracing::info!("PII migration phase 1 complete: {migrated}/{total} rows encrypted");
    }

    // Phase 2: Backfill phone_hash for records that have phone_enc but lost phone_hash.
    // This happens when phone was cleared (phone=NULL) before phone_hash was set,
    // leaving orphaned records that send_otp can't find by hash.
    let orphans: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, phone_enc FROM drivers \
         WHERE (phone_hash IS NULL OR phone_hash = '') \
           AND phone_enc IS NOT NULL AND phone_enc != ''"
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to query orphaned phone_enc rows: {e}"))?;

    if !orphans.is_empty() {
        let orphan_count = orphans.len();
        let mut backfilled = 0usize;
        for (id, phone_enc) in &orphans {
            match cipher.decrypt_field(phone_enc) {
                Ok(plaintext_phone) => {
                    let phone_hash = cipher.hash_phone(&plaintext_phone);
                    sqlx::query("UPDATE drivers SET phone_hash = ? WHERE id = ?")
                        .bind(&phone_hash)
                        .bind(id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to backfill phone_hash for {id}: {e}"))?;
                    backfilled += 1;
                }
                Err(e) => {
                    tracing::warn!("PII migration: cannot decrypt phone_enc for driver {id}: {e}");
                }
            }
        }
        tracing::info!("PII migration phase 2: backfilled phone_hash for {backfilled}/{orphan_count} orphaned records");
    }

    Ok(())
}

// ─── Phase 303: venue_id migration tests ──────────────────────────────────────

#[cfg(test)]
mod venue_id_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Create a temporary file-based SQLite pool for testing.
    /// SQLite `:memory:` doesn't support WAL mode (init_pool requires it), so we use a temp file.
    async fn test_pool() -> (SqlitePool, String) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let tid = std::thread::current().id();
        let path = std::env::temp_dir()
            .join(format!("racecontrol_venue_id_test_{:?}_{}.db", tid, nonce))
            .to_string_lossy()
            .to_string();
        let pool = init_pool(&path).await.expect("test pool init failed");
        (pool, path)
    }

    /// Helper: query pragma_table_info for a given table and return column names.
    async fn column_names(pool: &SqlitePool, table: &str) -> Vec<String> {
        let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&format!("PRAGMA table_info({})", table))
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        rows.into_iter().map(|r| r.1).collect()
    }

    /// Test 1: billing_sessions has venue_id column after migration.
    #[tokio::test]
    async fn test_venue_id_migration_billing_sessions() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "billing_sessions").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(
            cols.contains(&"venue_id".to_string()),
            "billing_sessions missing venue_id column. Got: {:?}",
            cols
        );
    }

    /// Test 2: laps has venue_id column after migration.
    #[tokio::test]
    async fn test_venue_id_migration_laps() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "laps").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(
            cols.contains(&"venue_id".to_string()),
            "laps missing venue_id column. Got: {:?}",
            cols
        );
    }

    /// Test 3: drivers has venue_id column after migration.
    #[tokio::test]
    async fn test_venue_id_migration_drivers() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "drivers").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(
            cols.contains(&"venue_id".to_string()),
            "drivers missing venue_id column. Got: {:?}",
            cols
        );
    }

    /// Test 4: wallets has venue_id column after migration.
    #[tokio::test]
    async fn test_venue_id_migration_wallets() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "wallets").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(
            cols.contains(&"venue_id".to_string()),
            "wallets missing venue_id column. Got: {:?}",
            cols
        );
    }

    /// Test 5: system_events has venue_id column after migration.
    #[tokio::test]
    async fn test_venue_id_migration_system_events() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "system_events").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(
            cols.contains(&"venue_id".to_string()),
            "system_events missing venue_id column. Got: {:?}",
            cols
        );
    }

    /// Test: running migrate() twice does not error (idempotent).
    #[tokio::test]
    async fn test_venue_id_migration_idempotent() {
        let (pool, path) = test_pool().await;
        // Running migrate() again on the same pool must not panic or error.
        let result = migrate(&pool).await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok(), "Second migrate() call failed: {:?}", result.err());
    }

    /// Test (Phase 317): pod_game_inventory and combo_validation_flags tables exist after migrate().
    #[tokio::test]
    async fn test_game_intelligence_tables_exist() {
        let (pool, path) = test_pool().await;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('pod_game_inventory', 'combo_validation_flags') ORDER BY name"
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(
            tables.contains(&"combo_validation_flags".to_string()),
            "combo_validation_flags table missing. Got: {:?}",
            tables
        );
        assert!(
            tables.contains(&"pod_game_inventory".to_string()),
            "pod_game_inventory table missing. Got: {:?}",
            tables
        );
    }

    /// Test: VenueConfig deserializes from empty TOML with default venue_id.
    #[test]
    fn test_venue_config_default_venue_id() {
        use crate::config::VenueConfig;

        let toml_str = r#"name = "Racing Point""#;
        let cfg: VenueConfig = toml::from_str(toml_str).expect("toml parse failed");
        assert_eq!(
            cfg.venue_id,
            "racingpoint-hyd-001",
            "VenueConfig.venue_id default mismatch"
        );
    }

    /// Phase 318 (LAUNCH-05): launch_timeline_spans table exists after migrate().
    #[tokio::test]
    async fn test_launch_timeline_spans_table_exists() {
        let (pool, path) = test_pool().await;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='launch_timeline_spans'"
        )
        .fetch_optional(&pool)
        .await
        .expect("query failed");
        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert!(row.is_some(), "launch_timeline_spans table missing");
        assert_eq!(row.unwrap().0, "launch_timeline_spans");
    }

    /// Phase 318 (LAUNCH-05): launch_timeline_spans INSERT and SELECT by launch_id round-trips.
    #[tokio::test]
    async fn test_launch_timeline_spans_round_trip() {
        let (pool, path) = test_pool().await;
        let launch_id = "test-launch-abc123";
        let events_json = r#"[{"kind":"ws_sent","elapsed_ms":0,"timestamp":"2026-04-03T00:00:00Z"}]"#;
        sqlx::query(
            "INSERT INTO launch_timeline_spans (launch_id, pod_id, sim_type, outcome, total_duration_ms, started_at, events_json)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(launch_id)
        .bind("pod_3")
        .bind("AssettoCorsa")
        .bind("success")
        .bind(35000i64)
        .bind("2026-04-03T00:00:00Z")
        .bind(events_json)
        .execute(&pool)
        .await
        .expect("INSERT failed");

        let row: (String,) = sqlx::query_as(
            "SELECT events_json FROM launch_timeline_spans WHERE launch_id = ?"
        )
        .bind(launch_id)
        .fetch_one(&pool)
        .await
        .expect("SELECT failed");

        drop(pool);
        let _ = std::fs::remove_file(&path);
        assert_eq!(row.0, events_json);
    }
}

// ─── Phase 363: Data Recording Verification migration tests ───────────────────

#[cfg(test)]
mod phase363_migration_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Create a unique temp-file SQLite pool for each test (WAL mode requires file-based DB).
    async fn test_pool() -> (SqlitePool, String) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let tid = std::thread::current().id();
        let path = std::env::temp_dir()
            .join(format!("rc_phase363_test_{:?}_{}.db", tid, nonce))
            .to_string_lossy()
            .to_string();
        let pool = init_pool(&path).await.expect("test pool init failed");
        (pool, path)
    }

    /// Helper: return column names for the given table via PRAGMA.
    async fn column_names(pool: &SqlitePool, table: &str) -> Vec<String> {
        let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&format!("PRAGMA table_info({})", table))
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        rows.into_iter().map(|r| r.1).collect()
    }

    /// Test 1: all 8 new billing_sessions columns exist after migration.
    #[tokio::test]
    async fn test_phase363_columns_present() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "billing_sessions").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        for col in &[
            "lap_count_expected",
            "lap_count_actual",
            "lap_count_flag",
            "telemetry_coverage_pct",
            "suspect",
            "suspect_reasons",
            "csv_fallback_received_at",
            "lap_reject_grace_until",
        ] {
            assert!(
                cols.contains(&col.to_string()),
                "billing_sessions missing column '{}'. Got: {:?}",
                col,
                cols
            );
        }
    }

    /// Test 2: lap_rejections table exists with correct columns.
    #[tokio::test]
    async fn test_phase363_lap_rejections_table() {
        let (pool, path) = test_pool().await;
        let cols = column_names(&pool, "lap_rejections").await;
        drop(pool);
        let _ = std::fs::remove_file(&path);
        for col in &["id", "session_id", "pod_id", "reason", "raw_data", "created_at"] {
            assert!(
                cols.contains(&col.to_string()),
                "lap_rejections missing column '{}'. Got: {:?}",
                col,
                cols
            );
        }
    }
}

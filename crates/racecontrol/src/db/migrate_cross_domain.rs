use sqlx::sqlite::SqlitePool;

/// Cross-domain migrations that span multiple domain boundaries.
/// Called from the main `migrate()` function in mod.rs.
pub(super) async fn migrate_cross_domain(pool: &SqlitePool) -> anyhow::Result<()> {
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

//! Database migrations: kiosk domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_kiosk(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── Auth tokens (single-use session PINs + QR codes) ──────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS auth_tokens (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            pricing_tier_id TEXT NOT NULL REFERENCES pricing_tiers(id),
            auth_type TEXT NOT NULL CHECK(auth_type IN ('pin', 'qr')),
            token TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'consuming', 'consumed', 'expired', 'cancelled')),
            billing_session_id TEXT,
            custom_price_paise INTEGER,
            custom_duration_minutes INTEGER,
            created_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            consumed_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    // Auth token indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_auth_tokens_pod ON auth_tokens(pod_id, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_auth_tokens_token ON auth_tokens(token, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_auth_tokens_driver ON auth_tokens(driver_id)")
        .execute(pool)
        .await?;


    // ─── Kiosk tables ─────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS kiosk_experiences (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            game TEXT NOT NULL,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            car_class TEXT,
            duration_minutes INTEGER NOT NULL,
            start_type TEXT DEFAULT 'pitlane',
            ac_preset_id TEXT,
            sort_order INTEGER DEFAULT 0,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS kiosk_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;


    // Seed default kiosk experiences (Assetto Corsa — Spa)
    // Car IDs must match exact folder names under AC content/cars/ (Kunos cars use ks_ prefix)
    sqlx::query(
        "INSERT OR IGNORE INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, sort_order)
         VALUES
            ('exp_spa_f1_30', 'Spa Hot Lap — F1', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 'A', 30, 'pitlane', 1),
            ('exp_spa_f1_60', 'Spa Hot Lap — F1 (Long)', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 'A', 60, 'pitlane', 2),
            ('exp_spa_gt3_30', 'Spa Hot Lap — GT3', 'assetto_corsa', 'spa', 'ks_ferrari_488_gt3', 'B', 30, 'pitlane', 3),
            ('exp_spa_gt4_30', 'Spa Hot Lap — Track Car', 'assetto_corsa', 'spa', 'ks_lotus_3_eleven', 'C', 30, 'pitlane', 4),
            ('exp_spa_road_30', 'Spa Hot Lap — Supercar', 'assetto_corsa', 'spa', 'ks_lamborghini_aventador_sv', 'D', 30, 'pitlane', 5),
            ('exp_trial', 'Trial Lap', 'assetto_corsa', 'spa', 'ks_porsche_911_gt3_rs', 'A', 5, 'pitlane', 0)",
    )
    .execute(pool)
    .await?;


    // Seed new game experiences (AC Rally, AC EVO, Forza Horizon 5, LMU)
    sqlx::query(
        "INSERT OR IGNORE INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, sort_order)
         VALUES
            ('exp_rally_classic_30', 'Rally — Classic Cars', 'assetto_corsa_rally', 'default', 'default', 'A', 30, 'default', 20),
            ('exp_rally_modern_30', 'Rally — Modern Rally', 'assetto_corsa_rally', 'default', 'default', 'A', 30, 'default', 21),
            ('exp_evo_hotlap_30', 'AC EVO Hot Lap', 'assetto_corsa_evo', 'default', 'default', 'A', 30, 'default', 30),
            ('exp_evo_hotlap_60', 'AC EVO Hot Lap (Long)', 'assetto_corsa_evo', 'default', 'default', 'A', 60, 'default', 31),
            ('exp_fh5_freeroam_30', 'Forza Horizon 5', 'forza_horizon_5', 'mexico', 'default', 'A', 30, 'default', 40),
            ('exp_fh5_freeroam_60', 'Forza Horizon 5 (Long)', 'forza_horizon_5', 'mexico', 'default', 'A', 60, 'default', 41),
            ('exp_lmu_lemans_30', 'Le Mans Ultimate', 'le_mans_ultimate', 'le_mans', 'default', 'A', 30, 'default', 50),
            ('exp_lmu_lemans_60', 'Le Mans Ultimate (Long)', 'le_mans_ultimate', 'le_mans', 'default', 'A', 60, 'default', 51)",
    )
    .execute(pool)
    .await?;


    // Fix existing rows that were seeded without the ks_ prefix
    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_ferrari_sf15t' WHERE car = 'ferrari_sf15t'"
    ).execute(pool).await?;

    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_mclaren_p1_gtr' WHERE car = 'mclaren_p1_gtr'"
    ).execute(pool).await?;

    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_audi_r8_lms' WHERE car = 'audi_r8_lms'"
    ).execute(pool).await?;

    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_lotus_3_eleven' WHERE car = 'lotus_3_eleven'"
    ).execute(pool).await?;


    // Fix mislabeled car classes: GT3 preset had a hypercar (P1 GTR), GT4 preset had a GT3 car (R8 LMS)
    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_ferrari_488_gt3', name = 'Spa Hot Lap — GT3' WHERE id = 'exp_spa_gt3_30'"
    ).execute(pool).await?;

    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_lotus_3_eleven', name = 'Spa Hot Lap — Track Car' WHERE id = 'exp_spa_gt4_30'"
    ).execute(pool).await?;

    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_lamborghini_aventador_sv', name = 'Spa Hot Lap — Supercar' WHERE id = 'exp_spa_road_30'"
    ).execute(pool).await?;


    // Fix rally game ID: ea_wrc → assetto_corsa_rally (renamed to Assetto Corsa Rally)
    sqlx::query(
        "UPDATE kiosk_experiences SET game = 'assetto_corsa_rally', name = 'Rally — Classic Cars' WHERE id = 'exp_rally_classic_30'"
    ).execute(pool).await?;

    sqlx::query(
        "UPDATE kiosk_experiences SET game = 'assetto_corsa_rally', name = 'Rally — Modern Rally' WHERE id = 'exp_rally_modern_30'"
    ).execute(pool).await?;


    // Fix trial car: F1 car too harsh for beginners → use a GT road car
    sqlx::query(
        "UPDATE kiosk_experiences SET car = 'ks_porsche_911_gt3_rs', name = 'Trial Lap' WHERE id = 'exp_trial'"
    ).execute(pool).await?;


    // Fix Spa Road label: Lotus 3-Eleven is a track car, not a road car
    sqlx::query(
        "UPDATE kiosk_experiences SET name = 'Spa Hot Lap — Track Car' WHERE id = 'exp_spa_gt4_30'"
    ).execute(pool).await?;


    // Seed default kiosk settings
    sqlx::query(
        "INSERT OR IGNORE INTO kiosk_settings (key, value)
         VALUES
            ('venue_name', 'Racing Point'),
            ('tagline', 'May the Fastest Win.'),
            ('business_hours_start', '10:00'),
            ('business_hours_end', '22:00'),
            ('spectator_auto_rotate', 'true'),
            ('spectator_show_leaderboard', 'true')",
    )
    .execute(pool)
    .await?;


    // Kiosk indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_kiosk_exp_game ON kiosk_experiences(game)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_kiosk_exp_active ON kiosk_experiences(is_active, sort_order)")
        .execute(pool)
        .await?;


    let _ = sqlx::query("ALTER TABLE auth_tokens ADD COLUMN experience_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE auth_tokens ADD COLUMN custom_launch_args TEXT")
        .execute(pool)
        .await;


    // Migration: add 'consuming' to auth_tokens status CHECK constraint
    // SQLite can't ALTER CHECK constraints, so we rebuild the table
    let needs_rebuild: bool = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='auth_tokens'"
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|sql| !sql.contains("consuming"))
    .unwrap_or(false);

    if needs_rebuild {
        tracing::info!("Migrating auth_tokens table to add 'consuming' status");
        sqlx::query("ALTER TABLE auth_tokens RENAME TO auth_tokens_old")
            .execute(pool).await.map_err(|e| anyhow::anyhow!("rename: {}", e))?;
        sqlx::query(
            "CREATE TABLE auth_tokens (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                driver_id TEXT NOT NULL REFERENCES drivers(id),
                pricing_tier_id TEXT NOT NULL REFERENCES pricing_tiers(id),
                auth_type TEXT NOT NULL CHECK(auth_type IN ('pin', 'qr')),
                token TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'consuming', 'consumed', 'expired', 'cancelled')),
                billing_session_id TEXT,
                custom_price_paise INTEGER,
                custom_duration_minutes INTEGER,
                created_at TEXT DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL DEFAULT '2099-01-01T00:00:00',
                consumed_at TEXT,
                experience_id TEXT,
                custom_launch_args TEXT
            )"
        ).execute(pool).await.map_err(|e| anyhow::anyhow!("create: {}", e))?;
        sqlx::query(
            "INSERT INTO auth_tokens SELECT id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, billing_session_id, custom_price_paise, custom_duration_minutes, created_at, expires_at, consumed_at, experience_id, custom_launch_args FROM auth_tokens_old"
        ).execute(pool).await.map_err(|e| anyhow::anyhow!("copy: {}", e))?;
        sqlx::query("DROP TABLE auth_tokens_old")
            .execute(pool).await.map_err(|e| anyhow::anyhow!("drop old: {}", e))?;
        // Recreate indexes
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_auth_tokens_pod ON auth_tokens(pod_id, status)")
            .execute(pool).await;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_auth_tokens_token ON auth_tokens(token, status)")
            .execute(pool).await;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_auth_tokens_driver ON auth_tokens(driver_id)")
            .execute(pool).await;

        tracing::info!("auth_tokens migration complete");
    }

    // Fixup: ensure expires_at column exists (may be missing from earlier migration)
    let _ = sqlx::query("ALTER TABLE auth_tokens ADD COLUMN expires_at TEXT NOT NULL DEFAULT '2099-01-01T00:00:00'")
        .execute(pool)
        .await;


    // NOTE: updated_at column, backfill, and index for kiosk_experiences
    // are handled by migrate_cross_domain which runs after all domain migrations.


    // ─── pricing_tier_id for kiosk experiences (links experience → billing tier) ──
    let _ = sqlx::query("ALTER TABLE kiosk_experiences ADD COLUMN pricing_tier_id TEXT DEFAULT ''")
        .execute(pool)
        .await;


    // ─── Terminal commands table ─────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS terminal_commands (
            id TEXT PRIMARY KEY,
            cmd TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            exit_code INTEGER,
            stdout TEXT,
            stderr TEXT,
            timeout_ms INTEGER DEFAULT 30000,
            created_at TEXT DEFAULT (datetime('now')),
            started_at TEXT,
            completed_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_terminal_cmd_status ON terminal_commands(status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_terminal_cmd_created ON terminal_commands(created_at)")
        .execute(pool)
        .await?;


    // ─── Kiosk Allowlist (Phase 48) ───────────────────────────────────────────
    // Staff-added process names that rc-agent should allow through the kiosk
    // lock screen. The hardcoded baseline (~70 entries) lives in rc-agent;
    // this table holds only admin-managed additions.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS kiosk_allowlist (
            id TEXT PRIMARY KEY,
            process_name TEXT NOT NULL UNIQUE,
            added_by TEXT NOT NULL DEFAULT 'staff',
            notes TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_kiosk_allowlist_name ON kiosk_allowlist(process_name)",
    )
    .execute(pool)
    .await?;


    // ─── Phase 260: Leaderboard integrity — kiosk assist config (UX-06) ─────────
    // assist_config: JSON object with per-experience assist settings.
    // Used as fallback assist evidence for laps until telemetry sends per-lap config.
    // Format: {"traction_control":0,"stability_control":0,"abs":0,"ideal_line":false,"autoclutch":false}
    let _ = sqlx::query("ALTER TABLE kiosk_experiences ADD COLUMN assist_config TEXT")
        .execute(pool)
        .await;


    Ok(())
}

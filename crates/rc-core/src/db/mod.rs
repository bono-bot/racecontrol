use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

pub async fn init_pool(db_path: &str) -> anyhow::Result<SqlitePool> {
    // Ensure the parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = format!("sqlite:{}?mode=rwc", db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    // Run migrations
    migrate(&pool).await?;

    tracing::info!("Database initialized at {}", db_path);
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await?;
    sqlx::query("PRAGMA foreign_keys=ON").execute(pool).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS drivers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT,
            phone TEXT,
            steam_guid TEXT,
            iracing_id TEXT,
            avatar_url TEXT,
            total_laps INTEGER DEFAULT 0,
            total_time_ms INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pods (
            id TEXT PRIMARY KEY,
            number INTEGER NOT NULL UNIQUE,
            name TEXT NOT NULL,
            ip_address TEXT,
            sim_type TEXT NOT NULL,
            status TEXT DEFAULT 'offline',
            current_driver_id TEXT REFERENCES drivers(id),
            current_session_id TEXT REFERENCES sessions(id),
            last_seen TEXT,
            config_json TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            track TEXT NOT NULL,
            car_class TEXT,
            status TEXT DEFAULT 'pending',
            max_drivers INTEGER,
            laps_or_minutes INTEGER,
            started_at TEXT,
            ended_at TEXT,
            config_json TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS laps (
            id TEXT PRIMARY KEY,
            session_id TEXT REFERENCES sessions(id),
            driver_id TEXT REFERENCES drivers(id),
            pod_id TEXT REFERENCES pods(id),
            sim_type TEXT NOT NULL,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            lap_number INTEGER,
            lap_time_ms INTEGER NOT NULL,
            sector1_ms INTEGER,
            sector2_ms INTEGER,
            sector3_ms INTEGER,
            valid BOOLEAN DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS personal_bests (
            driver_id TEXT REFERENCES drivers(id),
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            best_lap_ms INTEGER NOT NULL,
            lap_id TEXT REFERENCES laps(id),
            achieved_at TEXT,
            PRIMARY KEY (driver_id, track, car)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS track_records (
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            driver_id TEXT REFERENCES drivers(id),
            best_lap_ms INTEGER NOT NULL,
            lap_id TEXT REFERENCES laps(id),
            achieved_at TEXT,
            PRIMARY KEY (track, car)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            status TEXT DEFAULT 'upcoming',
            sim_type TEXT,
            track TEXT,
            car_class TEXT,
            max_entries INTEGER,
            config_json TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS event_entries (
            event_id TEXT REFERENCES events(id),
            driver_id TEXT REFERENCES drivers(id),
            registered_at TEXT DEFAULT (datetime('now')),
            result_position INTEGER,
            result_time_ms INTEGER,
            PRIMARY KEY (event_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bookings (
            id TEXT PRIMARY KEY,
            driver_id TEXT REFERENCES drivers(id),
            pod_id TEXT,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            status TEXT DEFAULT 'confirmed',
            payment_status TEXT DEFAULT 'pending',
            notes TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS telemetry_samples (
            lap_id TEXT REFERENCES laps(id),
            offset_ms INTEGER NOT NULL,
            speed REAL,
            throttle REAL,
            brake REAL,
            steering REAL,
            gear INTEGER,
            rpm INTEGER,
            pos_x REAL,
            pos_y REAL,
            pos_z REAL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ─── Billing tables ──────────────────────────────────────────────────────

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pricing_tiers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            is_trial BOOLEAN DEFAULT 0,
            is_active BOOLEAN DEFAULT 1,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Seed default pricing tiers
    sqlx::query(
        "INSERT OR IGNORE INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES
            ('tier_30min', '30 Minutes', 30, 70000, 0, 1),
            ('tier_60min', '1 Hour', 60, 90000, 0, 2),
            ('tier_trial', 'Free Trial', 5, 0, 1, 0)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_sessions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            pod_id TEXT NOT NULL,
            pricing_tier_id TEXT NOT NULL REFERENCES pricing_tiers(id),
            allocated_seconds INTEGER NOT NULL,
            driving_seconds INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending',
            custom_price_paise INTEGER,
            notes TEXT,
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_events (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            event_type TEXT NOT NULL,
            driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
            metadata TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ─── Game launcher tables ─────────────────────────────────────────────────

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS game_launch_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            event_type TEXT NOT NULL,
            pid INTEGER,
            error_message TEXT,
            ai_suggestion TEXT,
            metadata TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ─── AC LAN tables ──────────────────────────────────────────────────────

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ac_presets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            config_json TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ac_sessions (
            id TEXT PRIMARY KEY,
            preset_id TEXT,
            config_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'starting',
            pod_ids TEXT,
            pid INTEGER,
            join_url TEXT,
            error_message TEXT,
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ac_sessions_status ON ac_sessions(status)")
        .execute(pool)
        .await?;

    // Add trial tracking column to drivers (ignore error if already exists)
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN has_used_trial BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    // ─── Customer auth columns on drivers ───────────────────────────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN pin_hash TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN phone_verified BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN otp_code TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN otp_expires_at TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN last_login_at TEXT")
        .execute(pool)
        .await;

    // ─── Customer registration & waiver columns on drivers ──────────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN dob TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN waiver_signed BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN waiver_signed_at TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN waiver_version TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_name TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_phone TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN registration_completed BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN signature_data TEXT")
        .execute(pool)
        .await;

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

    // ─── Customer sessions (PWA JWT tracking) ───────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS customer_sessions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            token_hash TEXT NOT NULL,
            device_info TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            revoked_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    // ─── Sync log (change data capture for cloud replication) ───────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sync_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            operation TEXT NOT NULL CHECK(operation IN ('insert', 'update', 'delete')),
            payload TEXT NOT NULL,
            synced BOOLEAN DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Indexes for common queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_session ON laps(session_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_driver ON laps(driver_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_track_car ON laps(track, car)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_lap ON telemetry_samples(lap_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_driver ON billing_sessions(driver_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_pod ON billing_sessions(pod_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_status ON billing_sessions(status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_events_session ON billing_events(billing_session_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_game_events_pod ON game_launch_events(pod_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_game_events_type ON game_launch_events(event_type)")
        .execute(pool)
        .await?;

    // Driver phone index (used by OTP lookups)
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_phone ON drivers(phone)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_waiver ON drivers(waiver_signed)")
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

    // Customer session indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_customer_sessions_driver ON customer_sessions(driver_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_customer_sessions_token ON customer_sessions(token_hash)")
        .execute(pool)
        .await?;

    // Sync log index
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sync_log_unsynced ON sync_log(synced, created_at)")
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
            ('exp_spa_gt3_30', 'Spa Hot Lap — GT3', 'assetto_corsa', 'spa', 'ks_mclaren_p1_gtr', 'B', 30, 'pitlane', 3),
            ('exp_spa_gt4_30', 'Spa Hot Lap — GT4', 'assetto_corsa', 'spa', 'ks_audi_r8_lms', 'C', 30, 'pitlane', 4),
            ('exp_spa_road_30', 'Spa Hot Lap — Road', 'assetto_corsa', 'spa', 'ks_lotus_3_eleven', 'D', 30, 'pitlane', 5),
            ('exp_trial', 'Trial Lap', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 'A', 5, 'pitlane', 0)",
    )
    .execute(pool)
    .await?;

    // Seed new game experiences (AC Rally, AC EVO, Forza Horizon 5, LMU)
    sqlx::query(
        "INSERT OR IGNORE INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, sort_order)
         VALUES
            ('exp_rally_classic_30', 'Rally Classic', 'assetto_corsa_rally', 'stage_default', 'default', 'A', 30, 'default', 20),
            ('exp_rally_modern_30', 'Rally Modern', 'assetto_corsa_rally', 'stage_default', 'default', 'A', 30, 'default', 21),
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

    // ─── AI suggestions table ─────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_suggestions (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            error_context TEXT,
            suggestion TEXT NOT NULL,
            model TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'crash',
            dismissed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_suggestions_pod ON ai_suggestions(pod_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_suggestions_created ON ai_suggestions(created_at)")
        .execute(pool)
        .await?;

    // ─── AI training pairs (Ollama learning from Claude CLI) ─────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_training_pairs (
            id TEXT PRIMARY KEY,
            query_hash TEXT NOT NULL,
            query_text TEXT NOT NULL,
            query_keywords TEXT NOT NULL,
            response_text TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'unknown',
            model TEXT NOT NULL,
            quality_score INTEGER NOT NULL DEFAULT 1,
            use_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_training_hash ON ai_training_pairs(query_hash)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_training_keywords ON ai_training_pairs(query_keywords)")
        .execute(pool)
        .await?;

    // ─── Link experience to billing session ──────────────────────────────────
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN experience_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN car TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN track TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN sim_type TEXT")
        .execute(pool)
        .await;
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

    // ─── Session feedback ──────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS session_feedback (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            rating INTEGER NOT NULL CHECK(rating BETWEEN 1 AND 5),
            comment TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ─── Wallet tables ──────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wallets (
            driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
            balance_paise INTEGER NOT NULL DEFAULT 0,
            total_credited_paise INTEGER NOT NULL DEFAULT 0,
            total_debited_paise INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wallet_transactions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            amount_paise INTEGER NOT NULL,
            balance_after_paise INTEGER NOT NULL,
            txn_type TEXT NOT NULL CHECK(txn_type IN (
                'topup_cash','topup_card','topup_upi','topup_online',
                'debit_session','debit_cafe','debit_merchandise','debit_penalty',
                'refund_session','refund_manual',
                'bonus','adjustment'
            )),
            reference_id TEXT,
            notes TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_reservations (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            pod_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active'
                CHECK(status IN ('active','completed','expired','cancelled')),
            created_at TEXT DEFAULT (datetime('now')),
            ended_at TEXT,
            last_activity_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Wallet indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_txn_driver ON wallet_transactions(driver_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_txn_created ON wallet_transactions(created_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_pod_res_driver ON pod_reservations(driver_id, status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_pod_res_pod ON pod_reservations(pod_id, status)")
        .execute(pool)
        .await?;

    // Add wallet columns to billing_sessions
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN reservation_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN wallet_debit_paise INTEGER")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN wallet_txn_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN staff_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN split_count INTEGER DEFAULT 1")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN split_duration_minutes INTEGER")
        .execute(pool)
        .await;

    // ─── Discount columns on billing_sessions ────────────────────────────────
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN discount_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN coupon_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN original_price_paise INTEGER")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN discount_reason TEXT")
        .execute(pool)
        .await;

    // ─── Cloud sync tables ───────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sync_state (
            table_name TEXT PRIMARY KEY,
            last_synced_at TEXT NOT NULL,
            last_sync_count INTEGER DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Add updated_at to tables that lack it
    let _ = sqlx::query("ALTER TABLE pricing_tiers ADD COLUMN updated_at TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE kiosk_experiences ADD COLUMN updated_at TEXT")
        .execute(pool)
        .await;

    // Backfill NULL updated_at with created_at
    let _ = sqlx::query("UPDATE drivers SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE pricing_tiers SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE kiosk_experiences SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;

    // Sync indexes
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

    // ─── Friends & Social ────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS friend_requests (
            id TEXT PRIMARY KEY,
            sender_id TEXT NOT NULL,
            receiver_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT DEFAULT (datetime('now')),
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_sender ON friend_requests(sender_id, status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_receiver ON friend_requests(receiver_id, status)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS friendships (
            id TEXT PRIMARY KEY,
            driver_a_id TEXT NOT NULL,
            driver_b_id TEXT NOT NULL,
            request_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            UNIQUE(driver_a_id, driver_b_id)
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friendships_a ON friendships(driver_a_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friendships_b ON friendships(driver_b_id)")
        .execute(pool)
        .await?;

    // ─── Multiplayer Group Sessions ───────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_sessions (
            id TEXT PRIMARY KEY,
            host_driver_id TEXT NOT NULL,
            experience_id TEXT NOT NULL,
            pricing_tier_id TEXT NOT NULL,
            shared_pin TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'forming',
            ac_session_id TEXT,
            total_members INTEGER NOT NULL DEFAULT 1,
            validated_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            started_at TEXT,
            completed_at TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_sessions_host ON group_sessions(host_driver_id, status)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_session_members (
            id TEXT PRIMARY KEY,
            group_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'invitee',
            status TEXT NOT NULL DEFAULT 'pending',
            pod_id TEXT,
            reservation_id TEXT,
            auth_token_id TEXT,
            billing_session_id TEXT,
            wallet_txn_id TEXT,
            invited_at TEXT DEFAULT (datetime('now')),
            accepted_at TEXT,
            validated_at TEXT,
            UNIQUE(group_session_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_session_members_driver ON group_session_members(driver_id, status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_session_members_group ON group_session_members(group_session_id)")
        .execute(pool)
        .await?;

    // Add presence column to drivers (idempotent)
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN presence TEXT DEFAULT 'hidden'")
        .execute(pool)
        .await;

    // ─── AI messaging table (Bono ↔ James) ───────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_messages (
            id TEXT PRIMARY KEY,
            sender TEXT NOT NULL,
            recipient TEXT NOT NULL,
            content TEXT NOT NULL,
            message_type TEXT NOT NULL DEFAULT 'text',
            metadata TEXT,
            channel TEXT NOT NULL DEFAULT 'http',
            status TEXT NOT NULL DEFAULT 'pending',
            in_reply_to TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            delivered_at TEXT,
            read_at TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_msg_recipient_status ON ai_messages(recipient, status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_msg_created ON ai_messages(created_at)")
        .execute(pool)
        .await?;

    // ─── Smart Scheduler events table ──────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS scheduler_events (
            id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            pod_id TEXT,
            details TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_scheduler_events_type ON scheduler_events(event_type)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_scheduler_events_created ON scheduler_events(created_at)")
        .execute(pool)
        .await?;

    // Seed default scheduler settings
    sqlx::query(
        "INSERT OR IGNORE INTO settings (key, value)
         VALUES
            ('scheduler_enabled', 'true'),
            ('scheduler_pre_wake_minutes', '15'),
            ('scheduler_pre_open_minutes', '10'),
            ('scheduler_post_close_minutes', '15')",
    )
    .execute(pool)
    .await?;

    // ─── Referral system ─────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS referrals (
            id TEXT PRIMARY KEY,
            referrer_id TEXT NOT NULL,
            referee_id TEXT,
            code TEXT NOT NULL UNIQUE,
            reward_credited INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            redeemed_at TEXT
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_referrals_code ON referrals(code)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_referrals_referrer ON referrals(referrer_id)")
        .execute(pool)
        .await?;

    // Add referral_code column to drivers
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN referral_code TEXT")
        .execute(pool)
        .await;

    // Nickname & leaderboard display preference
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN nickname TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN show_nickname_on_leaderboard BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    // Unique constraint on (name, dob) to prevent duplicate registrations
    let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_name_dob ON drivers(name, dob) WHERE registration_completed = 1")
        .execute(pool)
        .await;

    // ─── Coupons & Promo Codes ───────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS coupons (
            id TEXT PRIMARY KEY,
            code TEXT NOT NULL UNIQUE,
            coupon_type TEXT NOT NULL DEFAULT 'flat' CHECK(coupon_type IN ('percent', 'flat', 'free_minutes')),
            value INTEGER NOT NULL,
            max_uses INTEGER,
            used_count INTEGER DEFAULT 0,
            valid_from TEXT,
            valid_until TEXT,
            min_spend_paise INTEGER DEFAULT 0,
            first_session_only INTEGER DEFAULT 0,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_coupons_code ON coupons(code)")
        .execute(pool)
        .await?;

    // ─── Coupon redemptions ──────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS coupon_redemptions (
            id TEXT PRIMARY KEY,
            coupon_id TEXT NOT NULL REFERENCES coupons(id),
            driver_id TEXT NOT NULL,
            billing_session_id TEXT,
            discount_paise INTEGER NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_coupon_redemptions_driver ON coupon_redemptions(driver_id)")
        .execute(pool)
        .await?;

    // ─── Dynamic Pricing Rules ───────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pricing_rules (
            id TEXT PRIMARY KEY,
            rule_name TEXT NOT NULL,
            rule_type TEXT NOT NULL CHECK(rule_type IN ('peak', 'off_peak', 'group', 'custom')),
            day_of_week TEXT,
            hour_start INTEGER,
            hour_end INTEGER,
            multiplier REAL DEFAULT 1.0,
            flat_adjustment_paise INTEGER DEFAULT 0,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Seed default pricing rules
    sqlx::query(
        "INSERT OR IGNORE INTO pricing_rules (id, rule_name, rule_type, day_of_week, hour_start, hour_end, multiplier)
         VALUES
            ('rule_weekday_offpeak', 'Weekday Off-Peak', 'off_peak', '1,2,3,4,5', 10, 15, 0.78),
            ('rule_weekend_peak', 'Weekend Peak', 'peak', '0,6', 0, 24, 1.22),
            ('rule_group_4plus', 'Group 4+', 'group', NULL, NULL, NULL, 0.89)"
    )
    .execute(pool)
    .await?;

    // ─── Packages (occasion-based) ───────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS packages (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            num_rigs INTEGER NOT NULL,
            duration_minutes INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            includes_cafe INTEGER DEFAULT 0,
            cafe_budget_paise INTEGER DEFAULT 0,
            day_restriction TEXT,
            hour_start INTEGER,
            hour_end INTEGER,
            is_active INTEGER DEFAULT 1,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Seed default packages
    sqlx::query(
        "INSERT OR IGNORE INTO packages (id, name, description, num_rigs, duration_minutes, price_paise, includes_cafe, cafe_budget_paise, sort_order)
         VALUES
            ('pkg_date', 'Date Night', '2 rigs + 2 drinks from cafe', 2, 60, 180000, 1, 20000, 1),
            ('pkg_squad', 'Squad (4 Friends)', '4 rigs, group discount applied', 4, 60, 320000, 0, 0, 2),
            ('pkg_birthday', 'Birthday Bash', '6 rigs + cake + drinks for 2 hours', 6, 120, 800000, 1, 100000, 3),
            ('pkg_corporate', 'Corporate Team Building', '8 rigs + tournament + lunch for 2 hours', 8, 120, 1500000, 1, 200000, 4),
            ('pkg_student', 'Student Special', '1 rig, weekday 10am-3pm only', 1, 60, 60000, 0, 0, 5)"
    )
    .execute(pool)
    .await?;

    // ─── Refunds ───────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS refunds (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            amount_paise INTEGER NOT NULL,
            method TEXT NOT NULL CHECK(method IN ('wallet', 'cash', 'upi')),
            reason TEXT NOT NULL,
            notes TEXT,
            staff_id TEXT,
            wallet_txn_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ─── Membership tiers & subscriptions ────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS membership_tiers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            hours_included INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            perks TEXT,
            is_active INTEGER DEFAULT 1,
            sort_order INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO membership_tiers (id, name, hours_included, price_paise, perks, sort_order)
         VALUES
            ('mem_rookie', 'Rookie', 4, 300000, '{\"priority_booking\":true}', 1),
            ('mem_pro', 'Pro', 8, 500000, '{\"priority_booking\":true,\"league_entry\":true,\"telemetry_coaching\":true}', 2),
            ('mem_champion', 'Champion', 0, 800000, '{\"unlimited_offpeak\":true,\"all_leagues\":true,\"merch\":true}', 3)"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS memberships (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            tier_id TEXT NOT NULL REFERENCES membership_tiers(id),
            hours_used_minutes INTEGER DEFAULT 0,
            price_paise INTEGER NOT NULL,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            auto_renew INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'expired', 'cancelled')),
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_memberships_driver ON memberships(driver_id, status)")
        .execute(pool)
        .await?;

    // ─── Session highlights (clip URLs) ──────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS session_highlights (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL,
            clip_type TEXT DEFAULT 'best_lap',
            file_path TEXT,
            cloud_url TEXT,
            duration_secs INTEGER,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_highlights_session ON session_highlights(billing_session_id)")
        .execute(pool)
        .await?;

    // ─── Time trials (weekly challenges) ─────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS time_trials (
            id TEXT PRIMARY KEY,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            week_start TEXT NOT NULL,
            week_end TEXT NOT NULL,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ─── Google review tracking ──────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS review_nudges (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            billing_session_id TEXT NOT NULL,
            sent_at TEXT DEFAULT (datetime('now')),
            review_credited INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    // ─── Tournaments ─────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tournaments (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            format TEXT NOT NULL DEFAULT 'time_attack' CHECK(format IN ('time_attack', 'bracket', 'round_robin')),
            max_participants INTEGER DEFAULT 16,
            entry_fee_paise INTEGER DEFAULT 0,
            prize_pool_paise INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'upcoming' CHECK(status IN ('upcoming', 'registration', 'in_progress', 'completed', 'cancelled')),
            registration_start TEXT,
            registration_end TEXT,
            event_date TEXT,
            rules TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tournament_registrations (
            id TEXT PRIMARY KEY,
            tournament_id TEXT NOT NULL REFERENCES tournaments(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            seed INTEGER,
            status TEXT DEFAULT 'registered' CHECK(status IN ('registered', 'checked_in', 'eliminated', 'winner')),
            best_time_ms INTEGER,
            created_at TEXT DEFAULT (datetime('now')),
            UNIQUE(tournament_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tourney_reg ON tournament_registrations(tournament_id, driver_id)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tournament_matches (
            id TEXT PRIMARY KEY,
            tournament_id TEXT NOT NULL REFERENCES tournaments(id),
            round INTEGER NOT NULL,
            match_number INTEGER NOT NULL,
            driver_a TEXT REFERENCES drivers(id),
            driver_b TEXT REFERENCES drivers(id),
            time_a_ms INTEGER,
            time_b_ms INTEGER,
            winner_id TEXT REFERENCES drivers(id),
            status TEXT DEFAULT 'pending' CHECK(status IN ('pending', 'in_progress', 'completed')),
            completed_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    // ─── Staff members ──────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_members (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            phone TEXT NOT NULL UNIQUE,
            pin TEXT NOT NULL UNIQUE,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            last_login_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    // Action queue — cloud queues actions for venue to pick up
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS action_queue (
            id TEXT PRIMARY KEY,
            action_type TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','processing','completed','failed')),
            error TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            processed_at TEXT,
            acked_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    // ─── Debug system tables ──────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debug_playbooks (
            id TEXT PRIMARY KEY,
            category TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            steps TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debug_incidents (
            id TEXT PRIMARY KEY,
            pod_id TEXT,
            category TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            context_snapshot TEXT,
            playbook_id TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_inc_status ON debug_incidents(status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_inc_category ON debug_incidents(category)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_inc_created ON debug_incidents(created_at)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debug_resolutions (
            id TEXT PRIMARY KEY,
            incident_id TEXT NOT NULL,
            category TEXT NOT NULL,
            resolution_text TEXT NOT NULL,
            effectiveness INTEGER NOT NULL DEFAULT 3,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_res_category ON debug_resolutions(category)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_res_incident ON debug_resolutions(incident_id)")
        .execute(pool)
        .await?;

    // Seed debug playbooks
    let playbooks = [
        ("pb_pod_offline", "pod_offline", "Pod Offline / Not Responding", r#"[{"step_number":1,"action":"Ping the pod IP address","expected_result":"Reply from pod IP","timeout_seconds":5},{"step_number":2,"action":"Check pod-agent on port 8090 (curl http://<ip>:8090/ping)","expected_result":"pong response","timeout_seconds":10},{"step_number":3,"action":"Check Windows Firewall (all profiles: Domain, Private, Public)","expected_result":"Firewall disabled or port 8090 allowed","timeout_seconds":30},{"step_number":4,"action":"TCP scan subnet for DHCP drift (port 8090 across 192.168.31.2-254)","expected_result":"Find pod on new IP","timeout_seconds":60},{"step_number":5,"action":"Send Wake-on-LAN magic packet","expected_result":"Pod powers on and responds within 30s","timeout_seconds":45}]"#),
        ("pb_game_crash", "game_crash", "Game Crash / Won't Launch", r#"[{"step_number":1,"action":"Check if acs.exe process is running on the pod","expected_result":"Process listed or confirmed dead","timeout_seconds":10},{"step_number":2,"action":"Verify race.ini has AUTOSPAWN=1","expected_result":"AUTOSPAWN=1 present in race.ini","timeout_seconds":15},{"step_number":3,"action":"Check CSP gui.ini for FORCE_START=1 and HIDE_MAIN_MENU=1","expected_result":"Both settings enabled","timeout_seconds":15},{"step_number":4,"action":"Check disk space on pod (must have >1GB free)","expected_result":"Sufficient disk space available","timeout_seconds":10},{"step_number":5,"action":"Kill acs.exe and relaunch AC with correct working directory","expected_result":"AC launches successfully","timeout_seconds":30}]"#),
        ("pb_billing_stuck", "billing_stuck", "Billing / Timer Stuck", r#"[{"step_number":1,"action":"Check billing_sessions table for session status","expected_result":"Session found with correct status","timeout_seconds":10},{"step_number":2,"action":"Verify WebSocket connection between agent and core","expected_result":"WebSocket connected and receiving messages","timeout_seconds":10},{"step_number":3,"action":"Check billing tick loop is running (look for BillingTick events)","expected_result":"Tick events arriving every second","timeout_seconds":15},{"step_number":4,"action":"Restart billing session via API if stuck","expected_result":"Billing resumes with correct remaining time","timeout_seconds":20}]"#),
        ("pb_screen_stuck", "screen_stuck", "Blank / Stuck Screen", r#"[{"step_number":1,"action":"Check if Edge kiosk browser process is running","expected_result":"msedge.exe process found","timeout_seconds":10},{"step_number":2,"action":"Verify lock screen server on port 18923","expected_result":"HTTP 200 from localhost:18923","timeout_seconds":10},{"step_number":3,"action":"Kill and restart lock screen browser (msedge.exe)","expected_result":"Lock screen reappears","timeout_seconds":15},{"step_number":4,"action":"Check Windows screen blanking / power settings","expected_result":"Screen never turns off","timeout_seconds":10}]"#),
        ("pb_no_steering", "no_steering_input", "No Steering / Pedal Input", r#"[{"step_number":1,"action":"Check USB wheelbase connection (VID:1209 PID:FFB0)","expected_result":"Device visible in Device Manager","timeout_seconds":15},{"step_number":2,"action":"Verify Conspit Link 2.0 is running","expected_result":"ConspitLink2.0.exe process found","timeout_seconds":10},{"step_number":3,"action":"Restart ConspitLink2.0.exe","expected_result":"Wheel display shows telemetry data","timeout_seconds":15},{"step_number":4,"action":"Check Device Manager for USB errors or disabled devices","expected_result":"No errors on HID devices","timeout_seconds":15}]"#),
        ("pb_high_idle", "high_idle_time", "High Idle Time / Not Counting", r#"[{"step_number":1,"action":"Check driving_state for the pod","expected_result":"Should be 'active' during gameplay","timeout_seconds":5},{"step_number":2,"action":"Verify UDP telemetry arriving on port 9996","expected_result":"Packets received from AC","timeout_seconds":10},{"step_number":3,"action":"Check 10-second idle threshold configuration","expected_result":"Threshold set correctly in config","timeout_seconds":5},{"step_number":4,"action":"Inspect game state — is AC actually running and in a session?","expected_result":"AC running with active driving session","timeout_seconds":10}]"#),
        ("pb_sync_failure", "sync_failure", "Cloud Sync Failure", r#"[{"step_number":1,"action":"Check cloud reachability (ping 72.60.101.58)","expected_result":"Cloud server responds","timeout_seconds":10},{"step_number":2,"action":"Verify sync_log for recent errors","expected_result":"No errors in last sync cycle","timeout_seconds":10},{"step_number":3,"action":"Check internet connectivity (ping 8.8.8.8)","expected_result":"Internet reachable","timeout_seconds":5},{"step_number":4,"action":"Restart cloud_sync module","expected_result":"Sync resumes and pushes pending changes","timeout_seconds":30}]"#),
        ("pb_kiosk_bypass", "kiosk_bypass", "Kiosk Bypass / Desktop Access", r#"[{"step_number":1,"action":"Check kiosk lockdown setting in rc-agent config","expected_result":"Kiosk mode enabled","timeout_seconds":5},{"step_number":2,"action":"Verify keyboard hook is active (blocks Alt+Tab, Ctrl+Esc)","expected_result":"System shortcuts blocked","timeout_seconds":10},{"step_number":3,"action":"Check that taskbar is hidden","expected_result":"Taskbar not visible","timeout_seconds":5},{"step_number":4,"action":"Re-enable kiosk mode and restart lock screen","expected_result":"Kiosk fully locked down","timeout_seconds":15}]"#),
    ];

    for (id, category, title, steps) in &playbooks {
        sqlx::query(
            "INSERT OR IGNORE INTO debug_playbooks (id, category, title, steps) VALUES (?, ?, ?, ?)"
        )
        .bind(id)
        .bind(category)
        .bind(title)
        .bind(steps)
        .execute(pool)
        .await?;
    }

    // ─── Pod Activity Log (unified event stream) ─────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_activity_log (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            pod_number INTEGER DEFAULT 0,
            timestamp TEXT DEFAULT (datetime('now')),
            category TEXT NOT NULL,
            action TEXT NOT NULL,
            details TEXT DEFAULT '',
            source TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_pod ON pod_activity_log (pod_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_ts ON pod_activity_log (timestamp)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_cat ON pod_activity_log (category)")
        .execute(pool)
        .await?;

    // ─── Unlimited trials flag for test/demo drivers ──────────────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN unlimited_trials BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    // Seed test driver with unlimited trials for demos
    // Use UPSERT so existing driver always gets unlimited_trials=1 and has_used_trial reset
    let _ = sqlx::query(
        "INSERT INTO drivers (id, name, phone, has_used_trial, unlimited_trials, created_at, updated_at)
         VALUES ('driver_test_trial', 'Test Driver (Unlimited)', '0000000000', 0, 1, datetime('now'), datetime('now'))
         ON CONFLICT(id) DO UPDATE SET unlimited_trials = 1, has_used_trial = 0, updated_at = datetime('now')",
    )
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

    // ─── Double-Entry Bookkeeping: Chart of Accounts ──────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            code INTEGER NOT NULL UNIQUE,
            name TEXT NOT NULL,
            account_type TEXT NOT NULL CHECK(account_type IN ('asset', 'liability', 'equity', 'revenue', 'expense')),
            parent_id TEXT REFERENCES accounts(id),
            description TEXT,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(account_type)")
        .execute(pool)
        .await?;

    // Seed chart of accounts for RacingPoint
    let accounts = [
        // Assets (1xxx)
        ("acc_cash", 1000, "Cash", "asset", "", "Physical cash received"),
        ("acc_bank", 1100, "Bank Account", "asset", "", "Bank deposits (UPI, card)"),
        ("acc_ar", 1200, "Accounts Receivable", "asset", "", "Outstanding customer payments"),
        // Liabilities (2xxx)
        ("acc_wallet", 2000, "Customer Wallet Credits", "liability", "", "Prepaid credits owed to customers"),
        ("acc_gst_payable", 2100, "GST Payable", "liability", "", "Tax collected pending remittance"),
        // Equity (3xxx)
        ("acc_owner_equity", 3000, "Owner's Equity", "equity", "", "Owner investment"),
        ("acc_retained", 3100, "Retained Earnings", "equity", "", "Accumulated net profit"),
        // Revenue (4xxx)
        ("acc_racing_rev", 4000, "Racing Revenue", "revenue", "", "Sim racing session fees"),
        ("acc_cafe_rev", 4100, "Cafe Revenue", "revenue", "", "Food & beverage sales"),
        ("acc_merch_rev", 4200, "Merchandise Revenue", "revenue", "", "Merchandise sales"),
        ("acc_membership_rev", 4300, "Membership Revenue", "revenue", "", "Membership subscription fees"),
        ("acc_tournament_rev", 4400, "Tournament Revenue", "revenue", "", "Tournament entry fees"),
        // Expenses (5xxx)
        ("acc_refunds", 5000, "Refunds Issued", "expense", "", "Session & manual refunds"),
        ("acc_promo_bonus", 5100, "Promotional Bonuses", "expense", "", "Wallet topup bonus credits given"),
        ("acc_cafe_cogs", 5200, "Cafe Cost of Goods", "expense", "", "Food & beverage purchase costs"),
        ("acc_ops_expense", 5300, "Operating Expenses", "expense", "", "Rent, utilities, equipment"),
        ("acc_penalty_adj", 5400, "Penalties & Adjustments", "expense", "", "Manual wallet adjustments"),
    ];

    for (id, code, name, acct_type, _parent, desc) in &accounts {
        sqlx::query(
            "INSERT OR IGNORE INTO accounts (id, code, name, account_type, description)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(acct_type)
        .bind(desc)
        .execute(pool)
        .await?;
    }

    // ─── Double-Entry Bookkeeping: Journal Entries ─────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS journal_entries (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL DEFAULT (date('now')),
            description TEXT NOT NULL,
            reference_type TEXT,
            reference_id TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_journal_date ON journal_entries(date)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_journal_ref ON journal_entries(reference_type, reference_id)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS journal_entry_lines (
            id TEXT PRIMARY KEY,
            journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            debit_paise INTEGER NOT NULL DEFAULT 0,
            credit_paise INTEGER NOT NULL DEFAULT 0,
            CHECK(debit_paise >= 0 AND credit_paise >= 0),
            CHECK(NOT (debit_paise > 0 AND credit_paise > 0))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_jel_entry ON journal_entry_lines(journal_entry_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_jel_account ON journal_entry_lines(account_id)")
        .execute(pool)
        .await?;

    // ─── Billing pause-on-disconnect columns ────────────────────────────────
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN pause_count INTEGER DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN total_paused_seconds INTEGER DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN last_paused_at TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN refund_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    // ─── Dynamic port allocation columns on ac_sessions ──────────────────────
    let _ = sqlx::query("ALTER TABLE ac_sessions ADD COLUMN udp_port INTEGER")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE ac_sessions ADD COLUMN tcp_port INTEGER")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE ac_sessions ADD COLUMN http_port INTEGER")
        .execute(pool)
        .await;

    // ─── Bonus tiers table (configurable topup bonus percentages) ────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bonus_tiers (
            id TEXT PRIMARY KEY,
            min_amount_paise INTEGER NOT NULL,
            bonus_percent INTEGER NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // Seed default bonus tiers
    sqlx::query(
        "INSERT OR IGNORE INTO bonus_tiers (id, min_amount_paise, bonus_percent, sort_order)
         VALUES ('bt_2000', 200000, 10, 1), ('bt_4000', 400000, 20, 2)"
    )
    .execute(pool)
    .await?;

    // ─── Multiplayer Results ──────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS multiplayer_results (
            id TEXT PRIMARY KEY,
            group_session_id TEXT NOT NULL,
            ac_session_id TEXT,
            driver_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            best_lap_ms INTEGER,
            total_time_ms INTEGER,
            laps_completed INTEGER DEFAULT 0,
            dnf INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_multiplayer_results_group ON multiplayer_results(group_session_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_multiplayer_results_driver ON multiplayer_results(driver_id)")
        .execute(pool)
        .await?;

    tracing::info!("Database migrations complete");
    Ok(())
}

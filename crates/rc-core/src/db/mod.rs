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
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'consumed', 'expired', 'cancelled')),
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

    tracing::info!("Database migrations complete");
    Ok(())
}

//! Database migrations: core domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_core(pool: &SqlitePool) -> anyhow::Result<()> {
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
            sim_type TEXT NOT NULL DEFAULT 'assettoCorsa',
            best_lap_ms INTEGER NOT NULL,
            lap_id TEXT REFERENCES laps(id),
            achieved_at TEXT,
            PRIMARY KEY (driver_id, track, car, sim_type)
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS track_records (
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            sim_type TEXT NOT NULL DEFAULT 'assettoCorsa',
            driver_id TEXT REFERENCES drivers(id),
            best_lap_ms INTEGER NOT NULL,
            lap_id TEXT REFERENCES laps(id),
            achieved_at TEXT,
            PRIMARY KEY (track, car, sim_type)
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


    // ─── Guardian OTP columns (LEGAL-04/05) ──────────────────────────────────
    // Stored argon2-hashed OTP for guardian consent verification (SEC-08 compliant).
    // guardian_otp_verified is reset to 0 when a new OTP is sent (send_guardian_otp).
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_otp_code TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_otp_expires_at TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_otp_verified BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_otp_verified_at TEXT")
        .execute(pool)
        .await;


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

    // Driver phone index (used by OTP lookups)
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_phone ON drivers(phone)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_waiver ON drivers(waiver_signed)")
        .execute(pool)
        .await?;


    // Sync log index
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sync_log_unsynced ON sync_log(synced, created_at)")
        .execute(pool)
        .await?;


    // Additional columns, indexes, and seed data live in migrate_core_columns.rs
    super::migrate_core_columns::migrate_core_columns(pool).await?;

    Ok(())
}

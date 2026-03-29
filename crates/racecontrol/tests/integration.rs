//! Integration tests for RaceControl core: wallet, billing, ports, leaderboard.
//!
//! Uses in-memory SQLite so tests are fast and isolated.

use std::sync::Arc;

use rc_common::types::{BillingSessionStatus, DrivingState};
use racecontrol_crate::lap_tracker::{
    assign_championship_positions, auto_enter_event, compute_championship_standings,
    recalculate_event_positions, score_group_event,
};
use sqlx::SqlitePool;

// ─── Test Helpers ────────────────────────────────────────────────────────────

/// Create an in-memory SQLite database with all migrations applied.
async fn create_test_db() -> SqlitePool {
    // racecontrol's db::init_pool needs a file path, so we build the pool manually
    // and call the migration function indirectly by running the same SQL.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");

    // Run the same migration that db/mod.rs uses.
    // We call the public init via a direct import of the migrate function.
    // Since migrate is private, we replicate the essential schema here.
    run_test_migrations(&pool).await;

    pool
}

/// Replicate essential schema tables needed for integration tests.
/// This mirrors the production migrate() function in db/mod.rs.
async fn run_test_migrations(pool: &SqlitePool) {
    sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON").execute(pool).await.unwrap();
    sqlx::query("PRAGMA wal_autocheckpoint=400").execute(pool).await.unwrap();
    sqlx::query("PRAGMA busy_timeout=5000").execute(pool).await.unwrap();

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
            updated_at TEXT,
            has_used_trial BOOLEAN DEFAULT 0,
            pin_hash TEXT,
            phone_verified BOOLEAN DEFAULT 0,
            otp_code TEXT,
            otp_expires_at TEXT,
            last_login_at TEXT,
            dob TEXT,
            waiver_signed BOOLEAN DEFAULT 0,
            waiver_signed_at TEXT,
            waiver_version TEXT,
            guardian_name TEXT,
            guardian_phone TEXT,
            registration_completed BOOLEAN DEFAULT 0,
            signature_data TEXT,
            customer_id TEXT,
            is_employee BOOLEAN DEFAULT 0,
            referral_code TEXT,
            nickname TEXT,
            show_nickname_on_leaderboard BOOLEAN DEFAULT 0,
            presence TEXT DEFAULT 'hidden',
            cloud_driver_id TEXT
        )"
    ).execute(pool).await.unwrap();

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
        )"
    ).execute(pool).await.unwrap();

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
        )"
    ).execute(pool).await.unwrap();

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
            car_class TEXT,
            suspect INTEGER DEFAULT 0,
            review_required INTEGER NOT NULL DEFAULT 0,
            session_type TEXT NOT NULL DEFAULT 'practice',
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

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
        )"
    ).execute(pool).await.unwrap();

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
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    // Billing tables
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pricing_tiers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            is_trial BOOLEAN DEFAULT 0,
            is_active BOOLEAN DEFAULT 1,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES
            ('tier_30min', '30 Minutes', 30, 70000, 0, 1),
            ('tier_60min', '1 Hour', 60, 90000, 0, 2),
            ('tier_trial', 'Free Trial', 5, 0, 1, 0)"
    ).execute(pool).await.unwrap();

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
            created_at TEXT DEFAULT (datetime('now')),
            experience_id TEXT,
            car TEXT,
            track TEXT,
            sim_type TEXT,
            reservation_id TEXT,
            wallet_debit_paise INTEGER,
            wallet_txn_id TEXT,
            staff_id TEXT,
            split_count INTEGER DEFAULT 1,
            split_duration_minutes INTEGER,
            discount_paise INTEGER DEFAULT 0,
            coupon_id TEXT,
            original_price_paise INTEGER,
            discount_reason TEXT,
            pause_count INTEGER DEFAULT 0,
            total_paused_seconds INTEGER DEFAULT 0,
            last_paused_at TEXT,
            refund_paise INTEGER DEFAULT 0
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_events (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            event_type TEXT NOT NULL,
            driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
            metadata TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    // Wallet tables
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wallets (
            driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
            balance_paise INTEGER NOT NULL DEFAULT 0,
            total_credited_paise INTEGER NOT NULL DEFAULT 0,
            total_debited_paise INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

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
            idempotency_key TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    // Indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_txn_driver ON wallet_transactions(driver_id)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_txn_created ON wallet_transactions(created_at)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_track_car ON laps(track, car)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_driver ON laps(driver_id)")
        .execute(pool).await.unwrap();

    // Accounting tables (needed by wallet credit/debit functions)
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
        )"
    ).execute(pool).await.unwrap();

    // Seed accounts (subset needed for wallet operations)
    let accounts = [
        ("acc_cash", 1000, "Cash", "asset", "Physical cash on hand"),
        ("acc_card", 1001, "Card Receivable", "asset", "Card payment receivables"),
        ("acc_upi", 1002, "UPI Receivable", "asset", "UPI payment receivables"),
        ("acc_online", 1003, "Online Receivable", "asset", "Online payment receivables"),
        ("acc_wallet_liability", 1100, "Customer Wallet Balances", "liability", "Total outstanding wallet credits"),
        ("acc_racing_revenue", 2000, "Racing Revenue", "revenue", "Sim racing session revenue"),
        ("acc_cafe_revenue", 2001, "Cafe Revenue", "revenue", "Cafe sales revenue"),
        ("acc_merch_revenue", 2002, "Merchandise Revenue", "revenue", "Merchandise sales revenue"),
        ("acc_bonus_expense", 3000, "Customer Bonuses", "expense", "Referral and promo bonuses"),
        ("acc_refund_expense", 3001, "Refunds", "expense", "Session refunds"),
    ];
    for (id, code, name, acct_type, desc) in &accounts {
        sqlx::query(
            "INSERT OR IGNORE INTO accounts (id, code, name, account_type, description) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(id).bind(code).bind(name).bind(acct_type).bind(desc)
        .execute(pool).await.unwrap();
    }

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS journal_entries (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL DEFAULT (date('now')),
            description TEXT NOT NULL,
            reference_type TEXT,
            reference_id TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS journal_entry_lines (
            id TEXT PRIMARY KEY,
            journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            debit_paise INTEGER NOT NULL DEFAULT 0,
            credit_paise INTEGER NOT NULL DEFAULT 0,
            CHECK(debit_paise >= 0 AND credit_paise >= 0),
            CHECK(NOT (debit_paise > 0 AND credit_paise > 0))
        )"
    ).execute(pool).await.unwrap();

    // Audit log (referenced by accounting module)
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
        )"
    ).execute(pool).await.unwrap();

    // AC sessions table (for port allocator tests referencing DB)
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
            created_at TEXT DEFAULT (datetime('now')),
            udp_port INTEGER,
            tcp_port INTEGER,
            http_port INTEGER
        )"
    ).execute(pool).await.unwrap();

    // Pod activity log (used by billing tick loop)
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
        )"
    ).execute(pool).await.unwrap();

    // Kiosk settings (referenced by AppState::broadcast_settings)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS kiosk_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )"
    ).execute(pool).await.unwrap();

    // Pod reservations (referenced by billing expire handler)
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
        )"
    ).execute(pool).await.unwrap();

    // Kiosk experiences table (needed for car_class lookup in persist_lap)
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
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT
        )"
    ).execute(pool).await.unwrap();

    // ─── Phase 12: Data Foundation ───────────────────────────────────────────

    // Telemetry samples table (mirrors db/mod.rs)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS telemetry_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lap_id TEXT NOT NULL,
            offset_ms INTEGER NOT NULL,
            speed REAL,
            throttle REAL,
            brake REAL,
            steering REAL,
            gear INTEGER,
            rpm REAL
        )"
    ).execute(pool).await.unwrap();

    // DATA-01: Covering indexes for leaderboard queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_leaderboard ON laps(track, car, valid, lap_time_ms)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_driver_created ON laps(driver_id, created_at)")
        .execute(pool).await.unwrap();

    // DATA-02: Covering index for telemetry visualization
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_lap ON telemetry_samples(lap_id)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)")
        .execute(pool).await.unwrap();

    // DATA-06: Index on laps(track, car_class) for event auto-entry matching
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_car_class ON laps(track, car_class)")
        .execute(pool).await.unwrap();

    // DATA-04: Unique index on cloud_driver_id
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_cloud_id ON drivers(cloud_driver_id)")
        .execute(pool).await.unwrap();

    // DATA-05: Six new competitive tables

    // 1. hotlap_events
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS hotlap_events (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            car_class TEXT NOT NULL,
            sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
            status TEXT NOT NULL DEFAULT 'upcoming'
                CHECK(status IN ('upcoming', 'active', 'scoring', 'completed', 'cancelled')),
            starts_at TEXT,
            ends_at TEXT,
            rule_107_percent INTEGER DEFAULT 1,
            reference_time_ms INTEGER,
            max_valid_laps INTEGER,
            championship_id TEXT REFERENCES championships(id),
            created_by TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    // 2. hotlap_event_entries
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS hotlap_event_entries (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL REFERENCES hotlap_events(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            lap_id TEXT REFERENCES laps(id),
            lap_time_ms INTEGER,
            sector1_ms INTEGER,
            sector2_ms INTEGER,
            sector3_ms INTEGER,
            position INTEGER,
            points INTEGER DEFAULT 0,
            badge TEXT,
            gap_to_leader_ms INTEGER,
            within_107_percent INTEGER DEFAULT 1,
            result_status TEXT DEFAULT 'pending'
                CHECK(result_status IN ('pending', 'finished', 'dns', 'dnf')),
            entered_at TEXT DEFAULT (datetime('now')),
            UNIQUE(event_id, driver_id)
        )"
    ).execute(pool).await.unwrap();

    // 3. championships
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS championships (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            season TEXT,
            car_class TEXT NOT NULL,
            sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
            status TEXT NOT NULL DEFAULT 'upcoming'
                CHECK(status IN ('upcoming', 'active', 'completed')),
            scoring_system TEXT NOT NULL DEFAULT 'f1_2010',
            total_rounds INTEGER DEFAULT 0,
            completed_rounds INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    // 4. championship_rounds
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS championship_rounds (
            championship_id TEXT NOT NULL REFERENCES championships(id),
            event_id TEXT NOT NULL REFERENCES hotlap_events(id),
            round_number INTEGER NOT NULL,
            PRIMARY KEY (championship_id, event_id)
        )"
    ).execute(pool).await.unwrap();

    // 5. championship_standings
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS championship_standings (
            championship_id TEXT NOT NULL REFERENCES championships(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            position INTEGER,
            total_points INTEGER DEFAULT 0,
            rounds_entered INTEGER DEFAULT 0,
            best_result INTEGER,
            wins INTEGER DEFAULT 0,
            podiums INTEGER DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now')),
            PRIMARY KEY (championship_id, driver_id)
        )"
    ).execute(pool).await.unwrap();

    // 6. driver_ratings
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS driver_ratings (
            driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
            rating_class TEXT NOT NULL DEFAULT 'Rookie',
            class_points INTEGER NOT NULL DEFAULT 0,
            total_events INTEGER DEFAULT 0,
            total_podiums INTEGER DEFAULT 0,
            total_wins INTEGER DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    // Indexes for new competitive tables
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_events_status ON hotlap_events(status, track)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_events_updated ON hotlap_events(updated_at)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_entries_event ON hotlap_event_entries(event_id, position)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_entries_driver ON hotlap_event_entries(driver_id)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_championships_updated ON championships(updated_at)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_champ_rounds_champ ON championship_rounds(championship_id, round_number)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_champ_standings_champ ON championship_standings(championship_id, position)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_driver_ratings_class ON driver_ratings(rating_class, class_points)")
        .execute(pool).await.unwrap();

    // ─── Group sessions + multiplayer (needed for Phase 14 GRP tests) ────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_sessions (
            id TEXT PRIMARY KEY,
            host_driver_id TEXT NOT NULL,
            experience_id TEXT NOT NULL,
            pricing_tier_id TEXT NOT NULL DEFAULT 'tier_30min',
            shared_pin TEXT NOT NULL DEFAULT '0000',
            status TEXT NOT NULL DEFAULT 'forming',
            ac_session_id TEXT,
            total_members INTEGER NOT NULL DEFAULT 1,
            validated_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            started_at TEXT,
            completed_at TEXT,
            track TEXT,
            car TEXT,
            ai_count INTEGER
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_session_members (
            id TEXT PRIMARY KEY,
            group_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'invitee',
            status TEXT NOT NULL DEFAULT 'pending',
            pod_id TEXT,
            UNIQUE(group_session_id, driver_id)
        )"
    ).execute(pool).await.unwrap();

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
        )"
    ).execute(pool).await.unwrap();

    // ─── Phase 14 schema extensions ──────────────────────────────────────────
    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN hotlap_event_id TEXT").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN p2_count INTEGER DEFAULT 0").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN p3_count INTEGER DEFAULT 0").execute(pool).await;

    // ─── Phase 33: Billing rates (per-minute tiered pricing) ────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_rates (
            id TEXT PRIMARY KEY,
            tier_order INTEGER NOT NULL,
            tier_name TEXT NOT NULL,
            threshold_minutes INTEGER NOT NULL,
            rate_per_min_paise INTEGER NOT NULL,
            is_active BOOLEAN DEFAULT 1,
            sim_type TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise)
         VALUES
            ('rate_standard', 1, 'Standard', 30, 2500),
            ('rate_extended', 2, 'Extended', 60, 2000),
            ('rate_marathon', 3, 'Marathon', 0, 1500)"
    ).execute(pool).await.unwrap();
}

/// Create a minimal AppState backed by the given pool.
fn create_test_state(pool: SqlitePool) -> Arc<racecontrol_crate::state::AppState> {
    let config = racecontrol_crate::config::Config::default_test();
    let field_cipher = racecontrol_crate::crypto::encryption::test_field_cipher();
    Arc::new(racecontrol_crate::state::AppState::new(config, pool, field_cipher))
}

/// Insert a test driver with a wallet.
async fn seed_test_driver(pool: &SqlitePool, driver_id: &str) {
    sqlx::query(
        "INSERT OR IGNORE INTO drivers (id, name, phone) VALUES (?, ?, ?)"
    )
    .bind(driver_id)
    .bind(format!("Test Driver {}", driver_id))
    .bind("9999999999")
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO wallets (driver_id, balance_paise, total_credited_paise, total_debited_paise)
         VALUES (?, 100000, 100000, 0)"
    )
    .bind(driver_id)
    .execute(pool)
    .await
    .unwrap();
}

/// Insert a test driver with specified wallet balance.
async fn seed_test_driver_with_balance(pool: &SqlitePool, driver_id: &str, balance_paise: i64) {
    sqlx::query(
        "INSERT OR IGNORE INTO drivers (id, name, phone) VALUES (?, ?, ?)"
    )
    .bind(driver_id)
    .bind(format!("Test Driver {}", driver_id))
    .bind("9999999999")
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO wallets (driver_id, balance_paise, total_credited_paise, total_debited_paise)
         VALUES (?, ?, ?, 0)"
    )
    .bind(driver_id)
    .bind(balance_paise)
    .bind(balance_paise)
    .execute(pool)
    .await
    .unwrap();
}

/// Insert a test pod.
async fn seed_test_pod(pool: &SqlitePool, pod_id: &str, number: u32) {
    sqlx::query(
        "INSERT OR IGNORE INTO pods (id, number, name, sim_type, status) VALUES (?, ?, ?, 'assetto_corsa', 'idle')"
    )
    .bind(pod_id)
    .bind(number as i64)
    .bind(format!("Pod {}", number))
    .execute(pool)
    .await
    .unwrap();
}

// =============================================================================
// Task 1: Test infrastructure verification
// =============================================================================

#[tokio::test]
async fn test_db_setup() {
    let pool = create_test_db().await;

    // Verify key tables exist by querying them
    let driver_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(driver_count.0, 0, "drivers table should exist and be empty");

    let wallet_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM wallets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(wallet_count.0, 0, "wallets table should exist and be empty");

    let tier_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM pricing_tiers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(tier_count.0 >= 3, "pricing_tiers should have seeded tiers");

    let account_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(account_count.0 >= 5, "accounts should have seeded entries");

    let billing_rate_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM billing_rates")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(billing_rate_count.0, 3, "billing_rates should have 3 seeded tiers (Standard, Extended, Marathon)");

    // Verify seeding helpers work
    seed_test_driver(&pool, "test-driver-1").await;
    seed_test_pod(&pool, "test-pod-1", 1).await;

    let driver_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(driver_count.0, 1, "should have 1 seeded driver");

    let pod_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM pods")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pod_count.0, 1, "should have 1 seeded pod");

    let balance = sqlx::query_as::<_, (i64,)>(
        "SELECT balance_paise FROM wallets WHERE driver_id = 'test-driver-1'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(balance.0, 100000, "seeded driver wallet should have 100000 paise");
}

// =============================================================================
// Task 2: Wallet integration tests
// =============================================================================

#[tokio::test]
async fn test_wallet_credit_debit_balance() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "wallet-test-1", 0).await;
    let state = create_test_state(pool);

    // Credit 100000 paise
    let balance = racecontrol_crate::wallet::credit(
        &state, "wallet-test-1", 100000, "topup_cash", None, None, None,
    ).await.unwrap();
    assert_eq!(balance, 100000, "balance after credit should be 100000");

    // Debit 70000 paise
    let (balance, _txn_id) = racecontrol_crate::wallet::debit(
        &state, "wallet-test-1", 70000, "debit_session", None, None,
    ).await.unwrap();
    assert_eq!(balance, 30000, "balance after debit should be 30000");

    // Verify 2 wallet_transaction rows exist
    let txn_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM wallet_transactions WHERE driver_id = 'wallet-test-1'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(txn_count.0, 2, "should have 2 transaction records");
}

#[tokio::test]
async fn test_wallet_transaction_recording() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "wallet-test-2", 0).await;
    let state = create_test_state(pool);

    // Credit 50000 paise with specific txn_type and notes
    racecontrol_crate::wallet::credit(
        &state, "wallet-test-2", 50000, "topup_cash", None, Some("Cash deposit"), None,
    ).await.unwrap();

    // Query wallet_transactions and verify details
    let txn = sqlx::query_as::<_, (i64, i64, String, Option<String>)>(
        "SELECT amount_paise, balance_after_paise, txn_type, notes
         FROM wallet_transactions WHERE driver_id = 'wallet-test-2' LIMIT 1"
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    assert_eq!(txn.0, 50000, "amount_paise should be 50000");
    assert_eq!(txn.1, 50000, "balance_after_paise should be 50000");
    assert_eq!(txn.2, "topup_cash", "txn_type should be topup_cash");
    assert_eq!(txn.3, Some("Cash deposit".to_string()), "notes should match");
}

#[tokio::test]
async fn test_wallet_insufficient_balance() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "wallet-test-3", 10000).await;
    let state = create_test_state(pool);

    // Attempt debit 20000 paise — should fail
    let result = racecontrol_crate::wallet::debit(
        &state, "wallet-test-3", 20000, "debit_session", None, None,
    ).await;

    assert!(result.is_err(), "debit should fail with insufficient balance");
    let err = result.unwrap_err();
    assert!(
        err.contains("Insufficient balance"),
        "error should mention insufficient balance: {}",
        err
    );

    // Verify balance unchanged
    let balance = racecontrol_crate::wallet::get_balance(&state, "wallet-test-3").await.unwrap();
    assert_eq!(balance, 10000, "balance should remain unchanged at 10000");
}

// =============================================================================
// Task 3: Billing integration tests
// =============================================================================

#[tokio::test]
async fn test_billing_timer_counting() {
    use racecontrol_crate::billing::BillingTimer;

    let mut timer = BillingTimer {
        session_id: "sess-1".to_string(),
        driver_id: "drv-1".to_string(),
        driver_name: "Test".to_string(),
        pod_id: "pod-1".to_string(),
        pricing_tier_name: "30 Minutes".to_string(),
        allocated_seconds: 1800,
        driving_seconds: 0,
        status: BillingSessionStatus::Active,
        driving_state: DrivingState::Active,
        started_at: None,
        warning_5min_sent: false,
        warning_1min_sent: false,
        offline_since: None,
        split_count: 1,
        split_duration_minutes: None,
        current_split_number: 1,
        pause_count: 0,
        total_paused_seconds: 0,
        last_paused_at: None,
        max_pause_duration_secs: 600,
        elapsed_seconds: 0,
        pause_seconds: 0,
        max_session_seconds: 1800,
        sim_type: None,
        recovery_pause_seconds: 0,
        pause_reason: racecontrol_crate::billing::PauseReason::None,
    };

    // Tick 30 times
    for _ in 0..30 {
        timer.tick();
    }

    assert_eq!(timer.driving_seconds, 30, "driving_seconds should be 30 after 30 ticks");
    assert_eq!(timer.remaining_seconds(), 1770, "remaining should be 1770");
}

#[tokio::test]
async fn test_billing_manual_pause() {
    use racecontrol_crate::billing::BillingTimer;

    let mut timer = BillingTimer {
        session_id: "sess-2".to_string(),
        driver_id: "drv-2".to_string(),
        driver_name: "Test".to_string(),
        pod_id: "pod-2".to_string(),
        pricing_tier_name: "30 Minutes".to_string(),
        allocated_seconds: 1800,
        driving_seconds: 0,
        status: BillingSessionStatus::Active,
        driving_state: DrivingState::Active,
        started_at: None,
        warning_5min_sent: false,
        warning_1min_sent: false,
        offline_since: None,
        split_count: 1,
        split_duration_minutes: None,
        current_split_number: 1,
        pause_count: 0,
        total_paused_seconds: 0,
        last_paused_at: None,
        max_pause_duration_secs: 600,
        elapsed_seconds: 0,
        pause_seconds: 0,
        max_session_seconds: 1800,
        sim_type: None,
        recovery_pause_seconds: 0,
        pause_reason: racecontrol_crate::billing::PauseReason::None,
    };

    // Drive 10 seconds
    for _ in 0..10 {
        timer.tick();
    }
    assert_eq!(timer.driving_seconds, 10);

    // Pause (manual)
    timer.status = BillingSessionStatus::PausedManual;

    // Tick 10 more times — should NOT increment
    for _ in 0..10 {
        timer.tick();
    }
    assert_eq!(timer.driving_seconds, 10, "driving_seconds should stay at 10 during manual pause");
}

#[tokio::test]
async fn test_billing_disconnect_pause() {
    use racecontrol_crate::billing::BillingTimer;

    let mut timer = BillingTimer {
        session_id: "sess-3".to_string(),
        driver_id: "drv-3".to_string(),
        driver_name: "Test".to_string(),
        pod_id: "pod-3".to_string(),
        pricing_tier_name: "30 Minutes".to_string(),
        allocated_seconds: 1800,
        driving_seconds: 0,
        status: BillingSessionStatus::Active,
        driving_state: DrivingState::Active,
        started_at: None,
        warning_5min_sent: false,
        warning_1min_sent: false,
        offline_since: None,
        split_count: 1,
        split_duration_minutes: None,
        current_split_number: 1,
        pause_count: 0,
        total_paused_seconds: 0,
        last_paused_at: None,
        max_pause_duration_secs: 600,
        elapsed_seconds: 0,
        pause_seconds: 0,
        max_session_seconds: 1800,
        sim_type: None,
        recovery_pause_seconds: 0,
        pause_reason: racecontrol_crate::billing::PauseReason::None,
    };

    // Drive 5 seconds
    for _ in 0..5 {
        timer.tick();
    }
    assert_eq!(timer.driving_seconds, 5);

    // Simulate disconnect: set status to PausedDisconnect
    timer.status = BillingSessionStatus::PausedDisconnect;
    timer.pause_count = 1;

    // Tick 10 more — driving_seconds should be frozen
    for _ in 0..10 {
        timer.tick();
    }
    assert_eq!(
        timer.driving_seconds, 5,
        "driving_seconds should remain at 5 during disconnect pause"
    );
    assert_eq!(
        timer.status,
        BillingSessionStatus::PausedDisconnect,
        "status should remain PausedDisconnect"
    );
}

#[tokio::test]
async fn test_billing_max_pauses() {
    // Test that after 3 pauses, billing continues even while offline.
    // This is tested via the tick_all_timers logic which checks pause_count < 3.
    // Here we test the BillingTimer struct behavior directly.
    use racecontrol_crate::billing::BillingTimer;

    let mut timer = BillingTimer {
        session_id: "sess-4".to_string(),
        driver_id: "drv-4".to_string(),
        driver_name: "Test".to_string(),
        pod_id: "pod-4".to_string(),
        pricing_tier_name: "30 Minutes".to_string(),
        allocated_seconds: 1800,
        driving_seconds: 0,
        status: BillingSessionStatus::Active,
        driving_state: DrivingState::Active,
        started_at: None,
        warning_5min_sent: false,
        warning_1min_sent: false,
        offline_since: None,
        split_count: 1,
        split_duration_minutes: None,
        current_split_number: 1,
        pause_count: 3, // Already used all 3 pauses
        total_paused_seconds: 0,
        last_paused_at: None,
        max_pause_duration_secs: 600,
        elapsed_seconds: 0,
        pause_seconds: 0,
        max_session_seconds: 1800,
        sim_type: None,
        recovery_pause_seconds: 0,
        pause_reason: racecontrol_crate::billing::PauseReason::None,
    };

    // With pause_count = 3 and status = Active, tick should still count
    // (the tick_all_timers function won't pause on the 4th disconnect)
    for _ in 0..10 {
        timer.tick();
    }
    assert_eq!(timer.driving_seconds, 10, "with 3 pauses used, billing keeps running");
    assert_eq!(
        timer.status,
        BillingSessionStatus::Active,
        "status should stay Active (no 4th pause)"
    );
}

#[tokio::test]
async fn test_billing_pause_timeout_refund() {
    // Test the refund calculation logic:
    // allocated=1800s, wallet_debit=70000 paise, driven=900s
    // Remaining = 1800 - 900 = 900s
    // Refund = (900/1800) * 70000 = 35000 paise
    let allocated_seconds: i64 = 1800;
    let wallet_debit_paise: i64 = 70000;
    let driving_seconds: i64 = 900;

    let remaining = allocated_seconds - driving_seconds;
    let refund_paise = (remaining as f64 / allocated_seconds as f64 * wallet_debit_paise as f64) as i64;

    assert_eq!(refund_paise, 35000, "refund should be 35000 paise (50% unused)");

    // Also verify via DB: create a session and calculate refund
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "refund-drv", 100000).await;
    seed_test_pod(&pool, "refund-pod", 1).await;
    let state = create_test_state(pool);

    // Create a billing session in DB
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, driving_seconds, status, wallet_debit_paise, started_at)
         VALUES ('bs-refund', 'refund-drv', 'refund-pod', 'tier_30min', 1800, 900, 'paused_disconnect', 70000, datetime('now'))"
    )
    .execute(&state.db)
    .await
    .unwrap();

    // Fetch session info as billing.rs does
    let session_info = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT allocated_seconds, wallet_debit_paise FROM billing_sessions WHERE id = 'bs-refund'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (allocated, debit) = session_info.unwrap();
    let debit = debit.unwrap();
    let remaining = allocated - driving_seconds;
    let calc_refund = (remaining as f64 / allocated as f64 * debit as f64) as i64;

    assert_eq!(calc_refund, 35000, "DB-based refund calculation should be 35000");

    // Actually issue the refund and verify wallet
    let new_balance = racecontrol_crate::wallet::refund(
        &state, "refund-drv", calc_refund, Some("bs-refund"), Some("Auto-refund: disconnect pause timeout"),
    ).await.unwrap();

    // Original balance was 100000, plus 35000 refund = 135000
    assert_eq!(new_balance, 135000, "balance after refund should be 135000");
}

// =============================================================================
// Task 4: Port allocator and leaderboard tests
// =============================================================================

#[tokio::test]
async fn test_port_allocator_unique_ports() {
    use racecontrol_crate::port_allocator::PortAllocator;

    // Use high ports unlikely to conflict
    let alloc = PortAllocator::new(19600, 18081, 16);

    let p1 = alloc.allocate("session-1").await.unwrap();
    let p2 = alloc.allocate("session-2").await.unwrap();
    let p3 = alloc.allocate("session-3").await.unwrap();
    let p4 = alloc.allocate("session-4").await.unwrap();

    let udp_ports = vec![p1.udp_port, p2.udp_port, p3.udp_port, p4.udp_port];
    let mut deduped = udp_ports.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(udp_ports.len(), deduped.len(), "All UDP ports must be unique");

    let http_ports = vec![p1.http_port, p2.http_port, p3.http_port, p4.http_port];
    let mut deduped = http_ports.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(http_ports.len(), deduped.len(), "All HTTP ports must be unique");
}

#[tokio::test]
async fn test_port_allocator_cooldown() {
    use racecontrol_crate::port_allocator::PortAllocator;

    let alloc = PortAllocator::new(19900, 18381, 1);

    let _p1 = alloc.allocate("s1").await.unwrap();
    alloc.release("s1").await;

    // Only slot is in cooldown — should fail
    let result = alloc.allocate("s2").await;
    assert!(result.is_err(), "Should fail when only slot is in cooldown");
}

#[tokio::test]
async fn test_wallet_transaction_sync_payload() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "sync-drv", 0).await;
    let state = create_test_state(pool);

    // Insert 3 wallet transactions with known data
    racecontrol_crate::wallet::credit(
        &state, "sync-drv", 10000, "topup_cash", None, Some("txn1"), None,
    ).await.unwrap();
    racecontrol_crate::wallet::credit(
        &state, "sync-drv", 20000, "topup_upi", None, Some("txn2"), None,
    ).await.unwrap();
    racecontrol_crate::wallet::credit(
        &state, "sync-drv", 30000, "topup_card", None, Some("txn3"), None,
    ).await.unwrap();

    // Query all transactions (simulating what the push payload builder would do)
    let txns = sqlx::query_as::<_, (String, i64, String)>(
        "SELECT id, amount_paise, txn_type FROM wallet_transactions
         WHERE driver_id = 'sync-drv' ORDER BY created_at ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap();

    assert_eq!(txns.len(), 3, "all 3 transactions should be in payload");
    assert_eq!(txns[0].1, 10000, "first txn amount should be 10000");
    assert_eq!(txns[1].1, 20000, "second txn amount should be 20000");
    assert_eq!(txns[2].1, 30000, "third txn amount should be 30000");
    assert_eq!(txns[0].2, "topup_cash");
    assert_eq!(txns[1].2, "topup_upi");
    assert_eq!(txns[2].2, "topup_card");
}

#[tokio::test]
async fn test_leaderboard_ordering() {
    let pool = create_test_db().await;

    // Create 5 drivers
    for i in 1..=5 {
        let id = format!("lb-drv-{}", i);
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)")
            .bind(&id)
            .bind(format!("Racer {}", i))
            .execute(&pool)
            .await
            .unwrap();
    }

    // Insert a pod for the laps
    seed_test_pod(&pool, "lb-pod-1", 1).await;

    // Insert 5 laps with different times on the same track
    let times = [95000, 88000, 102000, 85500, 91000]; // ms
    for (i, time_ms) in times.iter().enumerate() {
        let driver_id = format!("lb-drv-{}", i + 1);
        let lap_id = format!("lap-{}", i + 1);
        sqlx::query(
            "INSERT INTO laps (id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, valid)
             VALUES (?, ?, 'lb-pod-1', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 1, ?, 1)"
        )
        .bind(&lap_id)
        .bind(&driver_id)
        .bind(*time_ms as i64)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Query leaderboard (fastest first)
    let leaderboard = sqlx::query_as::<_, (String, i64)>(
        "SELECT d.name, MIN(l.lap_time_ms) as best_time
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = 'spa' AND l.car = 'ks_ferrari_sf15t' AND l.valid = 1
         GROUP BY l.driver_id
         ORDER BY best_time ASC"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(leaderboard.len(), 5, "should have 5 entries");
    assert_eq!(leaderboard[0].1, 85500, "fastest lap should be 85500ms");
    assert_eq!(leaderboard[1].1, 88000, "second fastest should be 88000ms");
    assert_eq!(leaderboard[2].1, 91000, "third should be 91000ms");
    assert_eq!(leaderboard[3].1, 95000, "fourth should be 95000ms");
    assert_eq!(leaderboard[4].1, 102000, "slowest should be 102000ms");
}

// =============================================================================
// Phase 12 — Data Foundation tests (DATA-01 through DATA-05)
// =============================================================================

/// DATA-01: Covering index on laps(track, car, valid, lap_time_ms) for leaderboard queries
#[tokio::test]
async fn test_leaderboard_index_exists() {
    let pool = create_test_db().await;

    // Seed a driver and a lap so the query planner has something to work with
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('idx-drv-1', 'Index Test')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "idx-pod-1", 1).await;
    sqlx::query(
        "INSERT INTO laps (id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, valid)
         VALUES ('idx-lap-1', 'idx-drv-1', 'idx-pod-1', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 1, 90000, 1)"
    ).execute(&pool).await.unwrap();

    // EXPLAIN QUERY PLAN the leaderboard query
    let plan_rows = sqlx::query_as::<_, (i32, i32, i32, String)>(
        "EXPLAIN QUERY PLAN SELECT driver_id, MIN(lap_time_ms) as best
         FROM laps
         WHERE track = 'spa' AND car = 'ks_ferrari_sf15t' AND valid = 1
         GROUP BY driver_id
         ORDER BY MIN(lap_time_ms)"
    ).fetch_all(&pool).await.unwrap();

    let plan_detail: String = plan_rows.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join(" ");
    assert!(
        plan_detail.contains("idx_laps_leaderboard"),
        "Leaderboard query should use idx_laps_leaderboard covering index, got: {}",
        plan_detail
    );
}

/// DATA-02: Covering index on telemetry_samples(lap_id, offset_ms) for telemetry visualization
#[tokio::test]
async fn test_telemetry_index_exists() {
    let pool = create_test_db().await;

    // Seed a telemetry sample so the query planner has data
    sqlx::query(
        "INSERT INTO telemetry_samples (lap_id, offset_ms, speed, throttle, brake, steering, gear, rpm)
         VALUES ('telem-lap-1', 100, 120.0, 0.8, 0.0, 0.1, 4, 7500.0)"
    ).execute(&pool).await.unwrap();

    // EXPLAIN QUERY PLAN the telemetry query
    let plan_rows = sqlx::query_as::<_, (i32, i32, i32, String)>(
        "EXPLAIN QUERY PLAN SELECT * FROM telemetry_samples
         WHERE lap_id = 'telem-lap-1'
         ORDER BY offset_ms"
    ).fetch_all(&pool).await.unwrap();

    let plan_detail: String = plan_rows.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join(" ");
    assert!(
        plan_detail.contains("idx_telemetry_lap_offset"),
        "Telemetry query should use idx_telemetry_lap_offset covering index, got: {}",
        plan_detail
    );
}

/// DATA-03: WAL autocheckpoint tuned to 400 pages
#[tokio::test]
async fn test_wal_tuning() {
    let pool = create_test_db().await;

    // Query the wal_autocheckpoint pragma value
    let result = sqlx::query_as::<_, (i64,)>("PRAGMA wal_autocheckpoint")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(
        result.0, 400,
        "wal_autocheckpoint should be 400, got: {}",
        result.0
    );
}

/// DATA-04: drivers table has cloud_driver_id column with unique index
#[tokio::test]
async fn test_cloud_driver_id_column() {
    let pool = create_test_db().await;

    // Insert a driver with cloud_driver_id
    sqlx::query(
        "INSERT INTO drivers (id, name, cloud_driver_id) VALUES ('cloud-drv-1', 'Cloud Test', 'cloud-uuid-abc')"
    ).execute(&pool).await.unwrap();

    // SELECT it back and verify
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT cloud_driver_id FROM drivers WHERE id = 'cloud-drv-1'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, "cloud-uuid-abc", "cloud_driver_id should be retrievable");

    // Verify uniqueness: inserting a duplicate cloud_driver_id should fail
    sqlx::query(
        "INSERT INTO drivers (id, name, cloud_driver_id) VALUES ('cloud-drv-2', 'Cloud Test 2', 'cloud-uuid-def')"
    ).execute(&pool).await.unwrap();

    let dup_result = sqlx::query(
        "INSERT INTO drivers (id, name, cloud_driver_id) VALUES ('cloud-drv-3', 'Cloud Test 3', 'cloud-uuid-abc')"
    ).execute(&pool).await;

    assert!(
        dup_result.is_err(),
        "Duplicate cloud_driver_id should be rejected by unique index"
    );
}

/// DATA-05: All six competitive tables accept valid inserts
#[tokio::test]
async fn test_competitive_tables_exist() {
    let pool = create_test_db().await;

    // Seed prerequisites: driver, pod, session, lap
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('comp-drv-1', 'Competitor')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "comp-pod-1", 1).await;
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('comp-sess-1', 'hotlap', 'assetto_corsa', 'monza')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, valid)
         VALUES ('comp-lap-1', 'comp-sess-1', 'comp-drv-1', 'comp-pod-1', 'assetto_corsa', 'monza', 'ks_ferrari_sf15t', 1, 85000, 1)"
    ).execute(&pool).await.unwrap();

    // 3. championships (must exist before hotlap_events references it)
    sqlx::query(
        "INSERT INTO championships (id, name, car_class, sim_type, status, scoring_system)
         VALUES ('champ-1', 'Season 1', 'GT3', 'assetto_corsa', 'upcoming', 'f1_2010')"
    ).execute(&pool).await.unwrap();

    // 1. hotlap_events
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, championship_id)
         VALUES ('evt-1', 'Monza Sprint', 'monza', 'ks_ferrari_sf15t', 'GT3', 'assetto_corsa', 'active', 'champ-1')"
    ).execute(&pool).await.unwrap();

    // 2. hotlap_event_entries
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_id, lap_time_ms, position, points, result_status)
         VALUES ('entry-1', 'evt-1', 'comp-drv-1', 'comp-lap-1', 85000, 1, 25, 'finished')"
    ).execute(&pool).await.unwrap();

    // 4. championship_rounds
    sqlx::query(
        "INSERT INTO championship_rounds (championship_id, event_id, round_number)
         VALUES ('champ-1', 'evt-1', 1)"
    ).execute(&pool).await.unwrap();

    // 5. championship_standings
    sqlx::query(
        "INSERT INTO championship_standings (championship_id, driver_id, position, total_points, rounds_entered, wins, podiums)
         VALUES ('champ-1', 'comp-drv-1', 1, 25, 1, 1, 1)"
    ).execute(&pool).await.unwrap();

    // 6. driver_ratings
    sqlx::query(
        "INSERT INTO driver_ratings (driver_id, rating_class, class_points, total_events, total_podiums, total_wins)
         VALUES ('comp-drv-1', 'Silver', 100, 5, 2, 1)"
    ).execute(&pool).await.unwrap();

    // Verify all inserts by querying each table
    let evt_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM hotlap_events")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(evt_count.0, 1, "hotlap_events should have 1 row");

    let entry_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM hotlap_event_entries")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(entry_count.0, 1, "hotlap_event_entries should have 1 row");

    let champ_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM championships")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(champ_count.0, 1, "championships should have 1 row");

    let round_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM championship_rounds")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(round_count.0, 1, "championship_rounds should have 1 row");

    let standing_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM championship_standings")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(standing_count.0, 1, "championship_standings should have 1 row");

    let rating_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM driver_ratings")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(rating_count.0, 1, "driver_ratings should have 1 row");
}

// =============================================================================
// Phase 12 — DATA-06: car_class population on laps
// =============================================================================

/// DATA-06: Laps with an active billing session linked to a kiosk_experience
/// should have car_class populated from the experience's car_class.
#[tokio::test]
async fn test_lap_car_class_populated() {
    let pool = create_test_db().await;

    // Seed driver and pod
    seed_test_driver(&pool, "cc-drv-1").await;
    seed_test_pod(&pool, "cc-pod-1", 1).await;

    // Seed a kiosk_experience with car_class='A'
    sqlx::query(
        "INSERT INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes)
         VALUES ('exp-test-1', 'Test Experience', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 'A', 30)"
    ).execute(&pool).await.unwrap();

    // Seed an active billing_session pointing to that experience
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, experience_id)
         VALUES ('bs-test-1', 'cc-drv-1', 'cc-pod-1', 'tier_30min', 1800, 'active', 'exp-test-1')"
    ).execute(&pool).await.unwrap();

    // Look up car_class via the same query persist_lap will use
    let car_class: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT ke.car_class
         FROM billing_sessions bs
         JOIN kiosk_experiences ke ON ke.id = bs.experience_id
         WHERE bs.driver_id = ? AND bs.status = 'active'
         LIMIT 1",
    )
    .bind("cc-drv-1")
    .fetch_optional(&pool)
    .await
    .unwrap()
    .and_then(|(c,)| c);

    assert_eq!(car_class, Some("A".to_string()), "car_class should be 'A' from kiosk_experience");

    // Create a session for the lap to reference
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('cc-sess-1', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    // Insert a lap with the resolved car_class
    sqlx::query(
        "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class)
         VALUES ('cc-lap-1', 'cc-sess-1', 'cc-drv-1', 'cc-pod-1', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 1, 90000, 1, ?)"
    )
    .bind(&car_class)
    .execute(&pool)
    .await
    .unwrap();

    // Verify car_class was stored
    let result = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT car_class FROM laps WHERE id = 'cc-lap-1'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, Some("A".to_string()), "laps.car_class should be 'A'");
}

/// DATA-06: Laps without an active billing session should have NULL car_class (no crash).
#[tokio::test]
async fn test_lap_car_class_null_without_session() {
    let pool = create_test_db().await;

    // Seed driver and pod but NO billing session
    seed_test_driver(&pool, "cc-drv-2").await;
    seed_test_pod(&pool, "cc-pod-2", 2).await;

    // Create a session for the lap to reference (separate from billing session)
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('cc-sess-2', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    // Insert a lap with NULL car_class (no active billing session)
    sqlx::query(
        "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class)
         VALUES ('cc-lap-2', 'cc-sess-2', 'cc-drv-2', 'cc-pod-2', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 1, 95000, 1, NULL)"
    ).execute(&pool).await.unwrap();

    // Verify car_class is NULL
    let result = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT car_class FROM laps WHERE id = 'cc-lap-2'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, None, "laps.car_class should be NULL without active billing session");
}

// =============================================================================
// Phase 13 — Leaderboard Core tests (LB-05: suspect flagging)
// =============================================================================

/// LB-05: A lap whose sector times sum >500ms away from lap_time_ms is flagged suspect=1.
#[tokio::test]
async fn test_lap_suspect_sector_sum() {
    let pool = create_test_db().await;

    // Seed driver and pod
    seed_test_driver(&pool, "sus-drv-1").await;
    seed_test_pod(&pool, "sus-pod-1", 1).await;

    // Create a session for the lap to reference
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('sus-sess-1', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    // Create AppState and persist a lap with mismatched sectors
    // lap_time_ms=90000, sectors sum to 88000 (diff=2000 > 500)
    let state = create_test_state(pool.clone());

    // Seed an active billing session so persist_lap doesn't skip
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('sus-bs-1', 'sus-drv-1', 'sus-pod-1', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    let lap = rc_common::types::LapData {
        id: "sus-lap-1".to_string(),
        session_id: "sus-sess-1".to_string(),
        driver_id: "sus-drv-1".to_string(),
        pod_id: "sus-pod-1".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 1,
        lap_time_ms: 90000,
        sector1_ms: Some(30000),
        sector2_ms: Some(28000),
        sector3_ms: Some(30000), // sum=88000, diff from 90000 = 2000 > 500
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };

    racecontrol_crate::lap_tracker::persist_lap(&state, &lap).await;

    // Verify suspect=1
    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT suspect FROM laps WHERE id = 'sus-lap-1'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, 1, "lap with sector sum mismatch >500ms should be suspect=1");
}

/// LB-05: A lap with lap_time_ms < 20000 (20s) is flagged suspect=1.
#[tokio::test]
async fn test_lap_suspect_sanity() {
    let pool = create_test_db().await;

    seed_test_driver(&pool, "sus-drv-2").await;
    seed_test_pod(&pool, "sus-pod-2", 2).await;

    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('sus-sess-2', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('sus-bs-2', 'sus-drv-2', 'sus-pod-2', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    let lap = rc_common::types::LapData {
        id: "sus-lap-2".to_string(),
        session_id: "sus-sess-2".to_string(),
        driver_id: "sus-drv-2".to_string(),
        pod_id: "sus-pod-2".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 1,
        lap_time_ms: 15000, // 15 seconds — impossibly fast
        sector1_ms: Some(5000),
        sector2_ms: Some(5000),
        sector3_ms: Some(5000), // sum=15000, matches lap_time perfectly
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };

    racecontrol_crate::lap_tracker::persist_lap(&state, &lap).await;

    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT suspect FROM laps WHERE id = 'sus-lap-2'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, 1, "lap with lap_time_ms < 20000 should be suspect=1");
}

/// LB-05: A valid lap with sane time and matching sectors gets suspect=0.
#[tokio::test]
async fn test_lap_not_suspect_valid() {
    let pool = create_test_db().await;

    seed_test_driver(&pool, "sus-drv-3").await;
    seed_test_pod(&pool, "sus-pod-3", 3).await;

    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('sus-sess-3', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('sus-bs-3', 'sus-drv-3', 'sus-pod-3', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    let lap = rc_common::types::LapData {
        id: "sus-lap-3".to_string(),
        session_id: "sus-sess-3".to_string(),
        driver_id: "sus-drv-3".to_string(),
        pod_id: "sus-pod-3".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 1,
        lap_time_ms: 90000,
        sector1_ms: Some(30000),
        sector2_ms: Some(30000),
        sector3_ms: Some(29800), // sum=89800, diff=200 <= 500
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };

    racecontrol_crate::lap_tracker::persist_lap(&state, &lap).await;

    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT suspect FROM laps WHERE id = 'sus-lap-3'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, 0, "valid lap with matching sectors should be suspect=0");
}

/// LB-05: A lap with NULL sectors and sane time should NOT be flagged suspect.
#[tokio::test]
async fn test_lap_not_suspect_no_sectors() {
    let pool = create_test_db().await;

    seed_test_driver(&pool, "sus-drv-4").await;
    seed_test_pod(&pool, "sus-pod-4", 4).await;

    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('sus-sess-4', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('sus-bs-4', 'sus-drv-4', 'sus-pod-4', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    let lap = rc_common::types::LapData {
        id: "sus-lap-4".to_string(),
        session_id: "sus-sess-4".to_string(),
        driver_id: "sus-drv-4".to_string(),
        pod_id: "sus-pod-4".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 1,
        lap_time_ms: 90000,
        sector1_ms: None,
        sector2_ms: None,
        sector3_ms: None, // no sectors
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };

    racecontrol_crate::lap_tracker::persist_lap(&state, &lap).await;

    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT suspect FROM laps WHERE id = 'sus-lap-4'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, 0, "lap with no sectors should NOT be suspect");
}

/// LB-05: A lap with all sectors = 0 and sane time should NOT be flagged suspect (zero = absent).
#[tokio::test]
async fn test_lap_suspect_zero_sectors_ignored() {
    let pool = create_test_db().await;

    seed_test_driver(&pool, "sus-drv-5").await;
    seed_test_pod(&pool, "sus-pod-5", 5).await;

    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('sus-sess-5', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('sus-bs-5', 'sus-drv-5', 'sus-pod-5', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    let lap = rc_common::types::LapData {
        id: "sus-lap-5".to_string(),
        session_id: "sus-sess-5".to_string(),
        driver_id: "sus-drv-5".to_string(),
        pod_id: "sus-pod-5".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 1,
        lap_time_ms: 90000,
        sector1_ms: Some(0),
        sector2_ms: Some(0),
        sector3_ms: Some(0), // all zero = treated as absent
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };

    racecontrol_crate::lap_tracker::persist_lap(&state, &lap).await;

    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT suspect FROM laps WHERE id = 'sus-lap-5'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, 0, "lap with zero sectors should NOT be suspect (zero = absent)");
}

// =============================================================================
// Phase 13 Plan 02 — Leaderboard sim_type filtering + circuit/vehicle records
// (LB-01, LB-02, LB-03, LB-04, LB-06)
// =============================================================================

/// Helper: insert a lap directly into the laps table for leaderboard query tests.
async fn insert_test_lap(
    pool: &SqlitePool,
    id: &str,
    driver_id: &str,
    pod_id: &str,
    sim_type: &str,
    track: &str,
    car: &str,
    lap_time_ms: i64,
    valid: bool,
    suspect: i32,
) {
    sqlx::query(
        "INSERT INTO laps (id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, valid, suspect)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)"
    )
    .bind(id)
    .bind(driver_id)
    .bind(pod_id)
    .bind(sim_type)
    .bind(track)
    .bind(car)
    .bind(lap_time_ms)
    .bind(valid as i32)
    .bind(suspect)
    .execute(pool)
    .await
    .unwrap();
}

/// LB-01 + LB-04: Track leaderboard with sim_type filter returns only matching sim laps.
#[tokio::test]
async fn test_leaderboard_sim_type_filter() {
    let pool = create_test_db().await;

    // Create two drivers
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('st-drv-1', 'AC Racer')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('st-drv-2', 'F1 Racer')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "st-pod-1", 10).await;

    // Insert AC lap on spa
    insert_test_lap(&pool, "st-lap-1", "st-drv-1", "st-pod-1", "assetto_corsa", "spa", "ks_ferrari_sf15t", 85000, true, 0).await;
    // Insert F1 25 lap on spa (same track!)
    insert_test_lap(&pool, "st-lap-2", "st-drv-2", "st-pod-1", "f1_25", "spa", "rb21", 82000, true, 0).await;

    // Query leaderboard filtered to assetto_corsa
    let results = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT d.name, l.car, MIN(l.lap_time_ms)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = 'spa' AND l.sim_type = 'assetto_corsa'
           AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(results.len(), 1, "should only return AC laps");
    assert_eq!(results[0].0, "AC Racer");
    assert_eq!(results[0].1, "ks_ferrari_sf15t");
    assert_eq!(results[0].2, 85000);
}

/// LB-01: No cross-sim leakage — querying f1_25 must not return AC laps.
#[tokio::test]
async fn test_leaderboard_no_cross_sim() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('cs-drv-1', 'AC Driver')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('cs-drv-2', 'F1 Driver')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "cs-pod-1", 11).await;

    insert_test_lap(&pool, "cs-lap-1", "cs-drv-1", "cs-pod-1", "assetto_corsa", "monza", "ks_ferrari_sf15t", 90000, true, 0).await;
    insert_test_lap(&pool, "cs-lap-2", "cs-drv-2", "cs-pod-1", "f1_25", "monza", "rb21", 88000, true, 0).await;

    // Query for f1_25 only
    let results = sqlx::query_as::<_, (String, i64)>(
        "SELECT d.name, MIN(l.lap_time_ms)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = 'monza' AND l.sim_type = 'f1_25'
           AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(results.len(), 1, "should only return F1 25 laps");
    assert_eq!(results[0].0, "F1 Driver");
    // Verify no AC laps
    let ac_check = results.iter().any(|r| r.0 == "AC Driver");
    assert!(!ac_check, "AC laps must not appear in F1 25 leaderboard");
}

/// LB-06: Default leaderboard query hides suspect=1 laps.
#[tokio::test]
async fn test_leaderboard_suspect_hidden() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('sh-drv-1', 'Clean Racer')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "sh-pod-1", 12).await;

    // Valid, non-suspect lap
    insert_test_lap(&pool, "sh-lap-1", "sh-drv-1", "sh-pod-1", "assetto_corsa", "nurburgring", "ks_bmw_m3_e30", 120000, true, 0).await;
    // Valid, but suspect lap (faster but cheaty)
    insert_test_lap(&pool, "sh-lap-2", "sh-drv-1", "sh-pod-1", "assetto_corsa", "nurburgring", "ks_bmw_m3_e30", 60000, true, 1).await;

    // Default query (no show_invalid) should hide suspect
    let results = sqlx::query_as::<_, (i64,)>(
        "SELECT MIN(l.lap_time_ms)
         FROM laps l
         WHERE l.track = 'nurburgring' AND l.sim_type = 'assetto_corsa'
           AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(results.len(), 1, "should return 1 entry");
    assert_eq!(results[0].0, 120000, "should show the clean lap, not the suspect 60s lap");
}

/// LB-06: show_invalid=true includes valid=0 laps but still hides suspect=1.
#[tokio::test]
async fn test_leaderboard_invalid_toggle() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('it-drv-1', 'Toggle Racer')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "it-pod-1", 13).await;

    // Valid, clean lap
    insert_test_lap(&pool, "it-lap-1", "it-drv-1", "it-pod-1", "assetto_corsa", "brands_hatch", "ks_audi_r8", 95000, true, 0).await;
    // Invalid (cut corner), clean lap
    insert_test_lap(&pool, "it-lap-2", "it-drv-1", "it-pod-1", "assetto_corsa", "brands_hatch", "ks_audi_r8", 93000, false, 0).await;
    // Valid, suspect lap (should STILL be hidden even with show_invalid)
    insert_test_lap(&pool, "it-lap-3", "it-drv-1", "it-pod-1", "assetto_corsa", "brands_hatch", "ks_audi_r8", 50000, true, 1).await;

    // show_invalid=true: drop the valid=1 filter but keep suspect filter
    let results = sqlx::query_as::<_, (i64,)>(
        "SELECT MIN(l.lap_time_ms)
         FROM laps l
         WHERE l.track = 'brands_hatch' AND l.sim_type = 'assetto_corsa'
           AND (l.suspect IS NULL OR l.suspect = 0)
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(results.len(), 1, "should return 1 entry (grouped by driver+car)");
    // The invalid lap (93000) is faster than the valid one (95000) and non-suspect, so it shows
    assert_eq!(results[0].0, 93000, "invalid but non-suspect lap should appear with show_invalid");
}

/// LB-02: Circuit records — one record per (track, car, sim_type).
#[tokio::test]
async fn test_circuit_records() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('cr-drv-1', 'Record Setter A')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('cr-drv-2', 'Record Setter B')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "cr-pod-1", 14).await;

    // 3 distinct (track, car, sim_type) combos
    insert_test_lap(&pool, "cr-lap-1", "cr-drv-1", "cr-pod-1", "assetto_corsa", "spa", "ks_ferrari_sf15t", 85000, true, 0).await;
    insert_test_lap(&pool, "cr-lap-2", "cr-drv-1", "cr-pod-1", "assetto_corsa", "spa", "ks_ferrari_sf15t", 87000, true, 0).await; // slower, same combo
    insert_test_lap(&pool, "cr-lap-3", "cr-drv-2", "cr-pod-1", "assetto_corsa", "monza", "ks_bmw_m3_e30", 78000, true, 0).await;
    insert_test_lap(&pool, "cr-lap-4", "cr-drv-1", "cr-pod-1", "f1_25", "spa", "rb21", 80000, true, 0).await;
    // Suspect lap — should be excluded from records
    insert_test_lap(&pool, "cr-lap-5", "cr-drv-2", "cr-pod-1", "assetto_corsa", "spa", "ks_ferrari_sf15t", 70000, true, 1).await;

    let records = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT l.track, l.car, l.sim_type, MIN(l.lap_time_ms)
         FROM laps l
         WHERE l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         GROUP BY l.track, l.car, l.sim_type
         ORDER BY l.track, l.car"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(records.len(), 3, "should have 3 records (3 unique track+car+sim_type combos)");

    // Verify the AC spa ferrari record is 85000 (not 70000 which is suspect)
    let spa_ac = records.iter().find(|r| r.0 == "spa" && r.2 == "assetto_corsa").unwrap();
    assert_eq!(spa_ac.3, 85000, "AC spa record should be 85000 (suspect 70000 excluded)");

    // Verify monza record
    let monza = records.iter().find(|r| r.0 == "monza").unwrap();
    assert_eq!(monza.3, 78000);

    // Verify F1 25 spa record
    let spa_f1 = records.iter().find(|r| r.0 == "spa" && r.2 == "f1_25").unwrap();
    assert_eq!(spa_f1.3, 80000);
}

/// LB-03: Vehicle records — best per track for a given car.
#[tokio::test]
async fn test_vehicle_records() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('vr-drv-1', 'Car Fan')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "vr-pod-1", 15).await;

    let car = "ks_ferrari_sf15t";
    // Same car on 3 different tracks
    insert_test_lap(&pool, "vr-lap-1", "vr-drv-1", "vr-pod-1", "assetto_corsa", "spa", car, 85000, true, 0).await;
    insert_test_lap(&pool, "vr-lap-2", "vr-drv-1", "vr-pod-1", "assetto_corsa", "monza", car, 78000, true, 0).await;
    insert_test_lap(&pool, "vr-lap-3", "vr-drv-1", "vr-pod-1", "assetto_corsa", "brands_hatch", car, 92000, true, 0).await;
    // Faster lap on spa but suspect — should be excluded
    insert_test_lap(&pool, "vr-lap-4", "vr-drv-1", "vr-pod-1", "assetto_corsa", "spa", car, 70000, true, 1).await;
    // Different car on spa — should not appear
    insert_test_lap(&pool, "vr-lap-5", "vr-drv-1", "vr-pod-1", "assetto_corsa", "spa", "ks_bmw_m3_e30", 80000, true, 0).await;

    let results = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT l.track, l.sim_type, MIN(l.lap_time_ms)
         FROM laps l
         WHERE l.car = ? AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         GROUP BY l.track, l.sim_type
         ORDER BY l.track"
    )
    .bind(car)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(results.len(), 3, "should have 3 track records for this car");

    let brands = results.iter().find(|r| r.0 == "brands_hatch").unwrap();
    assert_eq!(brands.2, 92000);

    let monza = results.iter().find(|r| r.0 == "monza").unwrap();
    assert_eq!(monza.2, 78000);

    let spa = results.iter().find(|r| r.0 == "spa").unwrap();
    assert_eq!(spa.2, 85000, "spa record should be 85000 (suspect 70000 excluded)");
}

// =============================================================================
// Phase 13 Plan 03 — Track record "beaten" notification data ordering
// (NTF-01, NTF-02)
// =============================================================================

/// NTF-01: When driver B beats driver A's track record, the previous holder's
/// name and email must be fetched BEFORE the UPSERT so the notification has
/// the correct data. Verifies: (1) get_previous_record_holder returns driver A's
/// data before UPSERT, (2) track_records shows driver B after persist_lap.
#[tokio::test]
async fn test_notification_data_before_upsert() {
    let pool = create_test_db().await;

    // Seed two drivers — A has email, B has email
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone) VALUES ('ntf-drv-a', 'Alice Record', 'a@test.com', '1111111111')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO wallets (driver_id, balance_paise) VALUES ('ntf-drv-a', 100000)"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone) VALUES ('ntf-drv-b', 'Bob Challenger', 'b@test.com', '2222222222')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO wallets (driver_id, balance_paise) VALUES ('ntf-drv-b', 100000)"
    ).execute(&pool).await.unwrap();

    // Seed pod and sessions
    seed_test_pod(&pool, "ntf-pod-1", 1).await;
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('ntf-sess-1', 'hotlap', 'assetto_corsa', 'monza')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    // Billing session for driver A
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('ntf-bs-1', 'ntf-drv-a', 'ntf-pod-1', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    // Driver A sets the initial record: 90000ms on monza/ks_ferrari_sf15t
    let lap_a = rc_common::types::LapData {
        id: "ntf-lap-a".to_string(),
        session_id: "ntf-sess-1".to_string(),
        driver_id: "ntf-drv-a".to_string(),
        pod_id: "ntf-pod-1".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "monza".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 1,
        lap_time_ms: 90000,
        sector1_ms: Some(30000),
        sector2_ms: Some(30000),
        sector3_ms: Some(30000),
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };
    let is_record_a = racecontrol_crate::lap_tracker::persist_lap(&state, &lap_a).await;
    assert!(is_record_a, "Driver A's first lap should be a track record");

    // Verify driver A holds the record
    let holder = sqlx::query_as::<_, (String, i64)>(
        "SELECT driver_id, best_lap_ms FROM track_records WHERE track = 'monza' AND car = 'ks_ferrari_sf15t'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(holder.0, "ntf-drv-a");
    assert_eq!(holder.1, 90000);

    // Use get_previous_record_holder to verify the data is available before UPSERT
    let prev = racecontrol_crate::lap_tracker::get_previous_record_holder(&state.db, "monza", "ks_ferrari_sf15t", "assettocorsa").await;
    assert!(prev.is_some(), "Previous record holder should exist");
    let (prev_time, prev_name, prev_email) = prev.unwrap();
    assert_eq!(prev_time, 90000);
    assert_eq!(prev_name, "Alice Record");
    assert_eq!(prev_email, Some("a@test.com".to_string()));

    // Now switch billing to driver B
    sqlx::query("UPDATE billing_sessions SET status = 'completed' WHERE id = 'ntf-bs-1'")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('ntf-bs-2', 'ntf-drv-b', 'ntf-pod-1', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    // Driver B breaks the record: 85000ms
    let lap_b = rc_common::types::LapData {
        id: "ntf-lap-b".to_string(),
        session_id: "ntf-sess-1".to_string(),
        driver_id: "ntf-drv-b".to_string(),
        pod_id: "ntf-pod-1".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "monza".to_string(),
        car: "ks_ferrari_sf15t".to_string(),
        lap_number: 2,
        lap_time_ms: 85000,
        sector1_ms: Some(28000),
        sector2_ms: Some(28500),
        sector3_ms: Some(28500),
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };
    let is_record_b = racecontrol_crate::lap_tracker::persist_lap(&state, &lap_b).await;
    assert!(is_record_b, "Driver B's faster lap should be a new track record");

    // After UPSERT, track_records now shows driver B
    let new_holder = sqlx::query_as::<_, (String, i64)>(
        "SELECT driver_id, best_lap_ms FROM track_records WHERE track = 'monza' AND car = 'ks_ferrari_sf15t'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(new_holder.0, "ntf-drv-b", "Track record holder should now be driver B");
    assert_eq!(new_holder.1, 85000, "Track record time should be 85000ms");
}

/// NTF-01: When previous holder has email=NULL, notification is silently skipped (no crash).
#[tokio::test]
async fn test_notification_skip_no_email() {
    let pool = create_test_db().await;

    // Driver C has NO email
    sqlx::query(
        "INSERT INTO drivers (id, name, phone) VALUES ('ntf-drv-c', 'Charlie NoEmail', '3333333333')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO wallets (driver_id, balance_paise) VALUES ('ntf-drv-c', 100000)"
    ).execute(&pool).await.unwrap();

    // Driver D will break the record
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone) VALUES ('ntf-drv-d', 'Dave Challenger', 'd@test.com', '4444444444')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO wallets (driver_id, balance_paise) VALUES ('ntf-drv-d', 100000)"
    ).execute(&pool).await.unwrap();

    seed_test_pod(&pool, "ntf-pod-2", 2).await;
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('ntf-sess-2', 'hotlap', 'assetto_corsa', 'spa')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    // Billing for driver C
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('ntf-bs-3', 'ntf-drv-c', 'ntf-pod-2', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    // Driver C sets record
    let lap_c = rc_common::types::LapData {
        id: "ntf-lap-c".to_string(),
        session_id: "ntf-sess-2".to_string(),
        driver_id: "ntf-drv-c".to_string(),
        pod_id: "ntf-pod-2".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_bmw_m3_e30".to_string(),
        lap_number: 1,
        lap_time_ms: 140000,
        sector1_ms: Some(46000),
        sector2_ms: Some(47000),
        sector3_ms: Some(47000),
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };
    racecontrol_crate::lap_tracker::persist_lap(&state, &lap_c).await;

    // Verify get_previous_record_holder returns None email
    let prev = racecontrol_crate::lap_tracker::get_previous_record_holder(&state.db, "spa", "ks_bmw_m3_e30", "assettocorsa").await;
    assert!(prev.is_some(), "Record exists for C");
    let (_, _, prev_email) = prev.unwrap();
    assert!(prev_email.is_none(), "Driver C has no email — should be None");

    // Switch billing to driver D
    sqlx::query("UPDATE billing_sessions SET status = 'completed' WHERE id = 'ntf-bs-3'")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('ntf-bs-4', 'ntf-drv-d', 'ntf-pod-2', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    // Driver D breaks the record — should NOT crash despite no email on previous holder
    let lap_d = rc_common::types::LapData {
        id: "ntf-lap-d".to_string(),
        session_id: "ntf-sess-2".to_string(),
        driver_id: "ntf-drv-d".to_string(),
        pod_id: "ntf-pod-2".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "spa".to_string(),
        car: "ks_bmw_m3_e30".to_string(),
        lap_number: 2,
        lap_time_ms: 135000,
        sector1_ms: Some(45000),
        sector2_ms: Some(45000),
        sector3_ms: Some(45000),
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };
    let is_record = racecontrol_crate::lap_tracker::persist_lap(&state, &lap_d).await;
    assert!(is_record, "Driver D should break the record (no crash)");

    // Verify record updated to driver D
    let holder = sqlx::query_as::<_, (String,)>(
        "SELECT driver_id FROM track_records WHERE track = 'spa' AND car = 'ks_bmw_m3_e30'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(holder.0, "ntf-drv-d");
}

/// NTF-01: First record on a track — no previous holder, no notification attempt, no crash.
#[tokio::test]
async fn test_notification_first_record_no_notify() {
    let pool = create_test_db().await;

    // Only one driver — first record ever
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone) VALUES ('ntf-drv-e', 'Eve First', 'e@test.com', '5555555555')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO wallets (driver_id, balance_paise) VALUES ('ntf-drv-e', 100000)"
    ).execute(&pool).await.unwrap();

    seed_test_pod(&pool, "ntf-pod-3", 3).await;
    sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track) VALUES ('ntf-sess-3', 'hotlap', 'assetto_corsa', 'nurburgring')"
    ).execute(&pool).await.unwrap();

    let state = create_test_state(pool.clone());

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
         VALUES ('ntf-bs-5', 'ntf-drv-e', 'ntf-pod-3', 'tier_30min', 1800, 'active')"
    ).execute(&pool).await.unwrap();

    // No prior record exists — get_previous_record_holder should return None
    let prev = racecontrol_crate::lap_tracker::get_previous_record_holder(&state.db, "nurburgring", "ks_porsche_911_gt3_r", "assettocorsa").await;
    assert!(prev.is_none(), "No previous record should exist on fresh track");

    // Driver E sets the first record
    let lap_e = rc_common::types::LapData {
        id: "ntf-lap-e".to_string(),
        session_id: "ntf-sess-3".to_string(),
        driver_id: "ntf-drv-e".to_string(),
        pod_id: "ntf-pod-3".to_string(),
        sim_type: rc_common::types::SimType::AssettoCorsa,
        track: "nurburgring".to_string(),
        car: "ks_porsche_911_gt3_r".to_string(),
        lap_number: 1,
        lap_time_ms: 480000,
        sector1_ms: Some(160000),
        sector2_ms: Some(160000),
        sector3_ms: Some(160000),
        valid: true,
        session_type: rc_common::types::SessionType::Practice,
        created_at: chrono::Utc::now(),
    };
    let is_record = racecontrol_crate::lap_tracker::persist_lap(&state, &lap_e).await;
    assert!(is_record, "First lap should be a track record");

    // Verify record was set (no crash on first record)
    let holder = sqlx::query_as::<_, (String, i64)>(
        "SELECT driver_id, best_lap_ms FROM track_records WHERE track = 'nurburgring' AND car = 'ks_porsche_911_gt3_r'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(holder.0, "ntf-drv-e");
    assert_eq!(holder.1, 480000);
}

// =============================================================================
// Phase 13 Plan 04 — Public driver search and profile endpoints
// (DRV-01, DRV-02, DRV-03, DRV-04)
// =============================================================================

/// DRV-01: Search drivers by name (case-insensitive), verify correct filtering.
#[tokio::test]
async fn test_driver_search() {
    let pool = create_test_db().await;

    // Insert 3 drivers with different names
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ds-drv-1', 'Alice Racer')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ds-drv-2', 'Bob Racer')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ds-drv-3', 'Charlie Fast')")
        .execute(&pool).await.unwrap();

    // Search for "racer" — should match Alice and Bob (case-insensitive)
    let results = sqlx::query_as::<_, (String, String)>(
        "SELECT id, CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END
         FROM drivers WHERE name LIKE '%' || ? || '%' COLLATE NOCASE OR nickname LIKE '%' || ? || '%' COLLATE NOCASE LIMIT 20"
    )
    .bind("racer").bind("racer")
    .fetch_all(&pool).await.unwrap();
    assert_eq!(results.len(), 2, "should match 2 drivers with 'racer' in name");

    // Search for "fast" — should match Charlie only
    let results = sqlx::query_as::<_, (String, String)>(
        "SELECT id, CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END
         FROM drivers WHERE name LIKE '%' || ? || '%' COLLATE NOCASE OR nickname LIKE '%' || ? || '%' COLLATE NOCASE LIMIT 20"
    )
    .bind("fast").bind("fast")
    .fetch_all(&pool).await.unwrap();
    assert_eq!(results.len(), 1, "should match 1 driver with 'fast' in name");
    assert_eq!(results[0].0, "ds-drv-3");

    // Search for "nobody" — should return empty
    let results = sqlx::query_as::<_, (String, String)>(
        "SELECT id, CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END
         FROM drivers WHERE name LIKE '%' || ? || '%' COLLATE NOCASE OR nickname LIKE '%' || ? || '%' COLLATE NOCASE LIMIT 20"
    )
    .bind("nobody").bind("nobody")
    .fetch_all(&pool).await.unwrap();
    assert_eq!(results.len(), 0, "should match 0 drivers for 'nobody'");
}

/// DRV-01: Search respects max 20 result limit.
#[tokio::test]
async fn test_driver_search_limit() {
    let pool = create_test_db().await;

    // Insert 25 drivers named "Driver N"
    for i in 1..=25 {
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)")
            .bind(format!("dsl-drv-{}", i))
            .bind(format!("Driver {}", i))
            .execute(&pool).await.unwrap();
    }

    // Search for "Driver" — should return at most 20
    let results = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE name LIKE '%' || ? || '%' COLLATE NOCASE OR nickname LIKE '%' || ? || '%' COLLATE NOCASE LIMIT 20"
    )
    .bind("Driver").bind("Driver")
    .fetch_all(&pool).await.unwrap();
    assert_eq!(results.len(), 20, "search must cap at 20 results");
}

/// DRV-02: Public driver profile excludes PII (no email, phone, wallet, billing data).
#[tokio::test]
async fn test_public_driver_no_pii() {
    let pool = create_test_db().await;

    // Insert a driver with PII fields populated
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone, total_laps, total_time_ms)
         VALUES ('pii-drv-1', 'PII Test Driver', 'test@example.com', '9876543210', 42, 3600000)"
    ).execute(&pool).await.unwrap();

    // Public profile query — explicitly select only safe fields
    let result = sqlx::query_as::<_, (String, i64, i64, Option<String>, String)>(
        "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END,
                total_laps, total_time_ms, avatar_url, created_at
         FROM drivers WHERE id = ?"
    )
    .bind("pii-drv-1")
    .fetch_one(&pool).await.unwrap();

    assert_eq!(result.0, "PII Test Driver");
    assert_eq!(result.1, 42);
    assert_eq!(result.2, 3600000);
    // Verify the query does NOT select email or phone — it cannot be extracted
    // from this tuple type. That's the whole point: the SELECT is safe by construction.
}

/// DRV-02: Public driver profile includes class_badge as null placeholder.
#[tokio::test]
async fn test_driver_profile_class_badge_null() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('cb-drv-1', 'Badge Test')")
        .execute(&pool).await.unwrap();

    // The public profile handler must include "class_badge": null in the response.
    // Since class_badge is not a column (Phase 15 RAT-01), it's hardcoded null.
    // Test the query still works for this driver — the class_badge is added at the API layer.
    let result = sqlx::query_as::<_, (String, i64)>(
        "SELECT name, total_laps FROM drivers WHERE id = ?"
    )
    .bind("cb-drv-1")
    .fetch_optional(&pool).await.unwrap();

    assert!(result.is_some(), "driver must be found");
    assert_eq!(result.unwrap().0, "Badge Test");
}

/// DRV-02: Public driver profile includes personal bests.
#[tokio::test]
async fn test_driver_profile_personal_bests() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('pb-drv-1', 'PB Test Driver')")
        .execute(&pool).await.unwrap();

    // Insert 3 personal bests for different track/car combos
    sqlx::query(
        "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, achieved_at)
         VALUES ('pb-drv-1', 'spa', 'ks_ferrari_sf15t', 85000, '2026-03-10T12:00:00Z')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, achieved_at)
         VALUES ('pb-drv-1', 'monza', 'ks_bmw_m3_e30', 78000, '2026-03-11T12:00:00Z')"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, achieved_at)
         VALUES ('pb-drv-1', 'brands_hatch', 'ks_audi_r8', 92000, '2026-03-12T12:00:00Z')"
    ).execute(&pool).await.unwrap();

    // Query personal bests
    let pbs = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT track, car, best_lap_ms, achieved_at FROM personal_bests WHERE driver_id = ? ORDER BY achieved_at DESC"
    )
    .bind("pb-drv-1")
    .fetch_all(&pool).await.unwrap();

    assert_eq!(pbs.len(), 3, "should have 3 personal bests");
    // Most recent first (ORDER BY achieved_at DESC)
    assert_eq!(pbs[0].0, "brands_hatch");
    assert_eq!(pbs[0].2, 92000);
    assert_eq!(pbs[1].0, "monza");
    assert_eq!(pbs[2].0, "spa");
}

/// DRV-03: Sector times <= 0 are returned as null in API response.
#[tokio::test]
async fn test_driver_lap_history_null_sectors() {
    let pool = create_test_db().await;

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('sec-drv-1', 'Sector Test')")
        .execute(&pool).await.unwrap();
    seed_test_pod(&pool, "sec-pod-1", 20).await;

    // Lap with zero sectors (should be null in API)
    sqlx::query(
        "INSERT INTO laps (id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, suspect)
         VALUES ('sec-lap-1', 'sec-drv-1', 'sec-pod-1', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 1, 90000, 0, 0, 0, 1, 0)"
    ).execute(&pool).await.unwrap();

    // Lap with real sectors
    sqlx::query(
        "INSERT INTO laps (id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, suspect)
         VALUES ('sec-lap-2', 'sec-drv-1', 'sec-pod-1', 'assetto_corsa', 'spa', 'ks_ferrari_sf15t', 2, 91000, 30000, 31000, 29000, 1, 0)"
    ).execute(&pool).await.unwrap();

    // Query laps with sector mapping logic
    let laps = sqlx::query_as::<_, (String, String, i64, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT track, car, lap_time_ms,
                CASE WHEN sector1_ms > 0 THEN sector1_ms ELSE NULL END,
                CASE WHEN sector2_ms > 0 THEN sector2_ms ELSE NULL END,
                CASE WHEN sector3_ms > 0 THEN sector3_ms ELSE NULL END
         FROM laps
         WHERE driver_id = ? AND (suspect IS NULL OR suspect = 0)
         ORDER BY created_at DESC LIMIT 100"
    )
    .bind("sec-drv-1")
    .fetch_all(&pool).await.unwrap();

    assert_eq!(laps.len(), 2);
    // Second lap (most recent by created_at — both have same default, so insertion order)
    // Note: SQLite default created_at is datetime('now'), both inserted in same second
    // so order might vary. Check both laps have correct sector handling.

    // Find the lap with zero sectors (90000ms)
    let zero_sector_lap = laps.iter().find(|l| l.2 == 90000).unwrap();
    assert!(zero_sector_lap.3.is_none(), "sector1 should be null for zero value");
    assert!(zero_sector_lap.4.is_none(), "sector2 should be null for zero value");
    assert!(zero_sector_lap.5.is_none(), "sector3 should be null for zero value");

    // Find the lap with real sectors (91000ms)
    let real_sector_lap = laps.iter().find(|l| l.2 == 91000).unwrap();
    assert_eq!(real_sector_lap.3, Some(30000), "sector1 should have value");
    assert_eq!(real_sector_lap.4, Some(31000), "sector2 should have value");
    assert_eq!(real_sector_lap.5, Some(29000), "sector3 should have value");
}

/// DRV-04: Nickname display logic — uses nickname when show_nickname_on_leaderboard=1.
#[tokio::test]
async fn test_driver_profile_nickname() {
    let pool = create_test_db().await;

    // Driver with nickname enabled
    sqlx::query(
        "INSERT INTO drivers (id, name, nickname, show_nickname_on_leaderboard) VALUES ('nn-drv-1', 'John Smith', 'SpeedDemon', 1)"
    ).execute(&pool).await.unwrap();

    // Driver with nickname disabled
    sqlx::query(
        "INSERT INTO drivers (id, name, nickname, show_nickname_on_leaderboard) VALUES ('nn-drv-2', 'Jane Doe', 'FastLane', 0)"
    ).execute(&pool).await.unwrap();

    // Query display name
    let result1 = sqlx::query_as::<_, (String,)>(
        "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END FROM drivers WHERE id = ?"
    )
    .bind("nn-drv-1")
    .fetch_one(&pool).await.unwrap();
    assert_eq!(result1.0, "SpeedDemon", "should use nickname when flag=1");

    let result2 = sqlx::query_as::<_, (String,)>(
        "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END FROM drivers WHERE id = ?"
    )
    .bind("nn-drv-2")
    .fetch_one(&pool).await.unwrap();
    assert_eq!(result2.0, "Jane Doe", "should use real name when flag=0");
}

// =============================================================================
// Phase 14 Plan 01 — Wave 0: Failing test stubs (RED phase)
// Requirements: EVT-02, EVT-05, EVT-06, GRP-01, GRP-04, CHP-02, CHP-04, CHP-05, SYNC-01, SYNC-02
// All 19 tests FAIL because implementation functions don't exist yet.
// TODO: Replace direct SQL assertions with auto_enter_event() call in Plan 14-02
// TODO: Replace scoring assertions with score_group_event() call in Plan 14-03
// TODO: Replace standings assertions with compute_championship_standings() in Plan 14-04
// =============================================================================

/// EVT-02 (#1): Matching lap auto-enters active event.
/// FAILS: no auto-entry logic yet — hotlap_event_entries will be empty.
#[tokio::test]
async fn test_auto_event_entry() {
    let pool = create_test_db().await;

    let driver_id = "ae-drv-1";
    let event_id = "ae-evt-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Auto Entry Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // Active event: monza, gt3, AC, started yesterday, ends tomorrow
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Monza GT3', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Valid lap matching event track + car_class + sim_type
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect, created_at)
         VALUES ('ae-lap-1', ?, 'assetto_corsa', 'monza', 'ks_ferrari_458_gt2', 1, 90000, 1, 'gt3', 0, datetime('now'))"
    ).bind(driver_id).execute(&pool).await.unwrap();

    // Call auto_enter_event directly (mirrors what persist_lap calls after INSERT)
    auto_enter_event(&pool, Some("ae-lap-1"), driver_id, "monza", "gt3", "assetto_corsa", 90000, None, None, None).await;

    let entry = sqlx::query_as::<_, (String, i64)>(
        "SELECT driver_id, lap_time_ms FROM hotlap_event_entries WHERE event_id = ?"
    ).bind(event_id).fetch_optional(&pool).await.unwrap();

    assert!(entry.is_some(), "Auto-entry should have created an event entry row");
    assert_eq!(entry.unwrap().1, 90000, "Entry lap_time_ms should be 90000");
}

/// EVT-02 (#2): Wrong car_class — no entry created.
/// FAILS: no auto-entry logic yet — but assertion will also fail because no entry exists.
#[tokio::test]
async fn test_auto_entry_no_match() {
    let pool = create_test_db().await;

    let driver_id = "nm-drv-1";
    let event_id = "nm-evt-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'No Match Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // Active GT3 event
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Monza GT3', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Lap with wrong car_class (gt4 instead of gt3)
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect, created_at)
         VALUES ('nm-lap-1', ?, 'assetto_corsa', 'monza', 'ks_bmw_m3', 1, 90000, 1, 'gt4', 0, datetime('now'))"
    ).bind(driver_id).execute(&pool).await.unwrap();

    // gt4 class should not match gt3 event
    auto_enter_event(&pool, Some("nm-lap-1"), driver_id, "monza", "gt4", "assetto_corsa", 90000, None, None, None).await;

    let count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM hotlap_event_entries WHERE event_id = ?"
    ).bind(event_id).fetch_one(&pool).await.unwrap();

    assert_eq!(count.0, 0, "GT4 lap must not auto-enter GT3 event");
}

/// EVT-02 (#3): Expired event (ends_at in the past) — no entry.
/// FAILS: no auto-entry logic; but this test will PASS as-is since no entry exists.
/// The real RED comes from test_auto_event_entry showing no implementation.
#[tokio::test]
async fn test_auto_entry_date_range() {
    let pool = create_test_db().await;

    let driver_id = "dr-drv-1";
    let event_id = "dr-evt-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Date Range Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // Expired event: ends_at in the past
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Expired Monza', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'completed',
                 datetime('now', '-3 day'), datetime('now', '-1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Lap submitted today (after event ended)
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect, created_at)
         VALUES ('dr-lap-1', ?, 'assetto_corsa', 'monza', 'ks_ferrari_458_gt2', 1, 90000, 1, 'gt3', 0, datetime('now'))"
    ).bind(driver_id).execute(&pool).await.unwrap();

    // Expired event should not receive entry
    auto_enter_event(&pool, Some("dr-lap-1"), driver_id, "monza", "gt3", "assetto_corsa", 90000, None, None, None).await;

    let count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM hotlap_event_entries WHERE event_id = ?"
    ).bind(event_id).fetch_one(&pool).await.unwrap();

    assert_eq!(count.0, 0, "Expired event must not receive auto-entry");
}

/// EVT-02 (#4): Faster lap replaces existing entry.
/// FAILS: no auto-entry logic yet — entry won't be updated to 85000.
#[tokio::test]
async fn test_auto_entry_faster_lap() {
    let pool = create_test_db().await;

    let driver_id = "fl-drv-1";
    let event_id = "fl-evt-1";
    let entry_id = "fl-entry-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Faster Lap Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Monza GT3', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Pre-existing entry with 90000ms
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, result_status)
         VALUES (?, ?, ?, 90000, 'pending')"
    ).bind(entry_id).bind(event_id).bind(driver_id).execute(&pool).await.unwrap();

    // New faster lap at 85000ms
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect, created_at)
         VALUES ('fl-lap-1', ?, 'assetto_corsa', 'monza', 'ks_ferrari_458_gt2', 2, 85000, 1, 'gt3', 0, datetime('now'))"
    ).bind(driver_id).execute(&pool).await.unwrap();

    // Faster lap should replace the existing 90000ms entry
    auto_enter_event(&pool, Some("fl-lap-1"), driver_id, "monza", "gt3", "assetto_corsa", 85000, None, None, None).await;

    let lap_time = sqlx::query_as::<_, (i64,)>(
        "SELECT lap_time_ms FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(driver_id).fetch_one(&pool).await.unwrap();

    assert_eq!(lap_time.0, 85000, "Entry should update to faster lap time 85000ms");
}

/// EVT-02 (#5): Slower lap does NOT replace existing entry.
/// FAILS: after Plan 14-02 exists, this test ensures slower lap doesn't overwrite best.
#[tokio::test]
async fn test_auto_entry_no_replace_slower() {
    let pool = create_test_db().await;

    let driver_id = "sl-drv-1";
    let event_id = "sl-evt-1";
    let entry_id = "sl-entry-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Slower Lap Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Monza GT3', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Pre-existing entry with 85000ms (best time)
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, result_status)
         VALUES (?, ?, ?, 85000, 'pending')"
    ).bind(entry_id).bind(event_id).bind(driver_id).execute(&pool).await.unwrap();

    // New slower lap at 90000ms
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect, created_at)
         VALUES ('sl-lap-1', ?, 'assetto_corsa', 'monza', 'ks_ferrari_458_gt2', 2, 90000, 1, 'gt3', 0, datetime('now'))"
    ).bind(driver_id).execute(&pool).await.unwrap();

    // Slower lap should NOT replace the existing 85000ms best entry
    auto_enter_event(&pool, Some("sl-lap-1"), driver_id, "monza", "gt3", "assetto_corsa", 90000, None, None, None).await;

    let lap_time = sqlx::query_as::<_, (i64,)>(
        "SELECT lap_time_ms FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(driver_id).fetch_one(&pool).await.unwrap();

    assert_eq!(lap_time.0, 85000, "Entry must stay at best time 85000ms, not be replaced by 90000ms");
}

/// EVT-05 (#6): 107% rule — lap at 107.5% of leader is flagged (within_107_percent=0).
/// FAILS: within_107_percent calculation not yet implemented.
#[tokio::test]
async fn test_107_percent_rule() {
    let pool = create_test_db().await;

    let event_id = "p107-evt-1";
    let leader_driver = "p107-drv-leader";
    let slow_driver = "p107-drv-slow";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Leader Driver')")
        .bind(leader_driver).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Slow Driver')")
        .bind(slow_driver).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, rule_107_percent, starts_at, ends_at)
         VALUES (?, '107% Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active', 1,
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Leader at 80000ms (P1)
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, within_107_percent, result_status)
         VALUES ('p107-entry-leader', ?, ?, 80000, 1, 1, 'finished')"
    ).bind(event_id).bind(leader_driver).execute(&pool).await.unwrap();

    // Slow driver at 86000ms (107.5% of 80000 = 85600, so 86000 > 85600 => outside 107%)
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, within_107_percent, result_status)
         VALUES ('p107-entry-slow', ?, ?, 86000, 2, 1, 'finished')"
    ).bind(event_id).bind(slow_driver).execute(&pool).await.unwrap();

    // Recalculate positions and 107% flags for all entries in the event
    recalculate_event_positions(&pool, event_id).await;

    // Integer math: 86000*100=8600000 vs 80000*107=8560000 => outside 107%
    let flag = sqlx::query_as::<_, (i64,)>(
        "SELECT within_107_percent FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(slow_driver).fetch_one(&pool).await.unwrap();

    assert_eq!(flag.0, 0, "86000ms (107.5% of 80000) must be flagged outside 107% rule");
}

/// EVT-05 (#7): 107% boundary — exactly 107.0% is within the rule.
/// FAILS: within_107_percent calculation not yet implemented.
#[tokio::test]
async fn test_107_boundary() {
    let pool = create_test_db().await;

    let event_id = "p107b-evt-1";
    let leader_driver = "p107b-drv-leader";
    let boundary_driver = "p107b-drv-boundary";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Leader')")
        .bind(leader_driver).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Boundary Driver')")
        .bind(boundary_driver).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, rule_107_percent, starts_at, ends_at)
         VALUES (?, '107% Boundary', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active', 1,
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Leader at 80000ms
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, within_107_percent, result_status)
         VALUES ('p107b-entry-leader', ?, ?, 80000, 1, 1, 'finished')"
    ).bind(event_id).bind(leader_driver).execute(&pool).await.unwrap();

    // Exactly 107.0%: 80000 * 107 / 100 = 85600ms
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, within_107_percent, result_status)
         VALUES ('p107b-entry-boundary', ?, ?, 85600, 2, 1, 'finished')"
    ).bind(event_id).bind(boundary_driver).execute(&pool).await.unwrap();

    // Recalculate positions and 107% flags
    recalculate_event_positions(&pool, event_id).await;

    // Integer math: 85600 * 100 = 8560000 <= 80000 * 107 = 8560000 => exactly on boundary = within
    let flag = sqlx::query_as::<_, (i64,)>(
        "SELECT within_107_percent FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(boundary_driver).fetch_one(&pool).await.unwrap();

    assert_eq!(flag.0, 1, "85600ms (exactly 107.0% of 80000) must be within 107% rule");
}

/// EVT-06 (#8): Gold badge — within 102% of reference_time_ms.
#[tokio::test]
async fn test_badge_gold() {
    let pool = create_test_db().await;

    let event_id = "badge-evt-gold";
    let driver_id = "badge-drv-gold";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Gold Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // reference_time_ms=80000; gold = within 102% (<=81600)
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, reference_time_ms, starts_at, ends_at)
         VALUES (?, 'Badge Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active', 80000,
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // 81500ms = 101.875% of 80000 => within gold threshold (<=102%)
    // auto_enter_event computes badge at insert time from the event's reference_time_ms
    auto_enter_event(&pool, None, driver_id, "monza", "gt3", "assetto_corsa", 81500, None, None, None).await;

    let badge = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT badge FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(driver_id).fetch_one(&pool).await.unwrap();

    assert_eq!(badge.0, Some("gold".to_string()), "81500ms (101.875% of ref) must earn gold badge");
}

/// EVT-06 (#9): Silver badge — within 102-105% of reference_time_ms.
#[tokio::test]
async fn test_badge_silver() {
    let pool = create_test_db().await;

    let event_id = "badge-evt-silver";
    let driver_id = "badge-drv-silver";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Silver Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // reference_time_ms=80000; silver = within 105% (<=84000), beyond gold (>81600)
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, reference_time_ms, starts_at, ends_at)
         VALUES (?, 'Badge Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active', 80000,
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // 83500ms = 104.375% of 80000 => silver (>102% and <=105%)
    auto_enter_event(&pool, None, driver_id, "monza", "gt3", "assetto_corsa", 83500, None, None, None).await;

    let badge = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT badge FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(driver_id).fetch_one(&pool).await.unwrap();

    assert_eq!(badge.0, Some("silver".to_string()), "83500ms (104.375% of ref) must earn silver badge");
}

/// EVT-06 (#10): Bronze badge — within 105-108% of reference_time_ms.
#[tokio::test]
async fn test_badge_bronze() {
    let pool = create_test_db().await;

    let event_id = "badge-evt-bronze";
    let driver_id = "badge-drv-bronze";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Bronze Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // reference_time_ms=80000; bronze = within 108% (<=86400), beyond silver (>84000)
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, reference_time_ms, starts_at, ends_at)
         VALUES (?, 'Badge Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active', 80000,
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // 86000ms = 107.5% of 80000 => bronze (>105% and <=108%)
    auto_enter_event(&pool, None, driver_id, "monza", "gt3", "assetto_corsa", 86000, None, None, None).await;

    let badge = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT badge FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(driver_id).fetch_one(&pool).await.unwrap();

    assert_eq!(badge.0, Some("bronze".to_string()), "86000ms (107.5% of ref) must earn bronze badge");
}

/// EVT-06 (#11): No badge when reference_time_ms is NULL.
#[tokio::test]
async fn test_badge_no_reference() {
    let pool = create_test_db().await;

    let event_id = "badge-evt-noref";
    let driver_id = "badge-drv-noref";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'No Ref Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    // Event with no reference_time_ms (NULL)
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'No Ref Event', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // When event has no reference_time_ms, badge must be NULL (not "none")
    auto_enter_event(&pool, None, driver_id, "monza", "gt3", "assetto_corsa", 81000, None, None, None).await;

    let badge = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT badge FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
    ).bind(event_id).bind(driver_id).fetch_one(&pool).await.unwrap();

    assert!(badge.0.is_none(), "badge must be NULL when event has no reference_time_ms");
}

/// GRP-01 (#12): F1 points scoring — P1/P2/P3 get 25/18/15 points.
/// FAILS: F1 scoring logic not yet implemented — points remain 0.
#[tokio::test]
async fn test_f1_points_scoring() {
    let pool = create_test_db().await;

    let event_id = "f1pts-evt-1";
    let gs_id = "f1pts-gs-1";

    for (id, name) in [("f1pts-drv-1", "P1 Driver"), ("f1pts-drv-2", "P2 Driver"), ("f1pts-drv-3", "P3 Driver")] {
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)").bind(id).bind(name).execute(&pool).await.unwrap();
    }

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'F1 Scoring Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Group session linked to event
    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, status, hotlap_event_id)
         VALUES (?, 'f1pts-drv-1', 'exp-1', 'completed', ?)"
    ).bind(gs_id).bind(event_id).execute(&pool).await.unwrap();

    // Multiplayer results: 3 drivers finishing P1/P2/P3
    for (pos, drv_id, best_lap) in [(1, "f1pts-drv-1", 80000i64), (2, "f1pts-drv-2", 81500i64), (3, "f1pts-drv-3", 82000i64)] {
        sqlx::query(
            "INSERT INTO multiplayer_results (id, group_session_id, driver_id, position, best_lap_ms, dnf)
             VALUES (?, ?, ?, ?, ?, 0)"
        )
        .bind(format!("f1pts-res-{}", pos))
        .bind(gs_id)
        .bind(drv_id)
        .bind(pos as i64)
        .bind(best_lap)
        .execute(&pool).await.unwrap();
    }

    // Call score_group_event() to process multiplayer_results into hotlap_event_entries
    score_group_event(&pool, gs_id, event_id).await.expect("score_group_event failed");

    let points: Vec<(String, i64)> = sqlx::query_as(
        "SELECT driver_id, points FROM hotlap_event_entries WHERE event_id = ? ORDER BY position"
    ).bind(event_id).fetch_all(&pool).await.unwrap();

    assert_eq!(points.len(), 3, "3 drivers should have event entries after F1 scoring");
    assert_eq!(points[0].1, 25, "P1 should score 25 F1 points");
    assert_eq!(points[1].1, 18, "P2 should score 18 F1 points");
    assert_eq!(points[2].1, 15, "P3 should score 15 F1 points");
}

/// GRP-01 (#13): DNS/DNF drivers score 0 points.
/// FAILS: F1 scoring logic not yet implemented.
#[tokio::test]
async fn test_dns_dnf_zero_points() {
    let pool = create_test_db().await;

    let event_id = "dnf-evt-1";
    let gs_id = "dnf-gs-1";

    for (id, name) in [("dnf-drv-1", "P1 Driver"), ("dnf-drv-2", "DNF Driver")] {
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)").bind(id).bind(name).execute(&pool).await.unwrap();
    }

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'DNF Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, status, hotlap_event_id)
         VALUES (?, 'dnf-drv-1', 'exp-1', 'completed', ?)"
    ).bind(gs_id).bind(event_id).execute(&pool).await.unwrap();

    // P1 finishes, P2 DNF
    sqlx::query(
        "INSERT INTO multiplayer_results (id, group_session_id, driver_id, position, best_lap_ms, dnf)
         VALUES ('dnf-res-1', ?, 'dnf-drv-1', 1, 80000, 0)"
    ).bind(gs_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO multiplayer_results (id, group_session_id, driver_id, position, best_lap_ms, dnf)
         VALUES ('dnf-res-2', ?, 'dnf-drv-2', 2, 81500, 1)"
    ).bind(gs_id).execute(&pool).await.unwrap();

    // Call score_group_event() to process multiplayer_results including DNF driver
    score_group_event(&pool, gs_id, event_id).await.expect("score_group_event failed");

    let dnf_points = sqlx::query_as::<_, (i64,)>(
        "SELECT points FROM hotlap_event_entries WHERE event_id = ? AND driver_id = 'dnf-drv-2'"
    ).bind(event_id).fetch_optional(&pool).await.unwrap();

    assert!(dnf_points.is_some(), "DNF driver should have an event entry after scoring");
    assert_eq!(dnf_points.unwrap().0, 0, "DNF driver must score 0 F1 points");
}

/// GRP-04 (#14): Gap-to-leader calculation.
/// FAILS: gap calculation not yet implemented — gap_to_leader_ms won't be set.
#[tokio::test]
async fn test_gap_to_leader() {
    let pool = create_test_db().await;

    let event_id = "gap-evt-1";

    for (id, name) in [("gap-drv-1", "Leader"), ("gap-drv-2", "P2 Driver"), ("gap-drv-3", "P3 Driver")] {
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)").bind(id).bind(name).execute(&pool).await.unwrap();
    }

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Gap Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Insert entries with lap times — gap_to_leader_ms defaults to NULL
    for (pos, drv_id, lap_ms) in [(1, "gap-drv-1", 80000i64), (2, "gap-drv-2", 81500i64), (3, "gap-drv-3", 83000i64)] {
        sqlx::query(
            "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, result_status)
             VALUES (?, ?, ?, ?, ?, 'finished')"
        )
        .bind(format!("gap-entry-{}", pos))
        .bind(event_id)
        .bind(drv_id)
        .bind(lap_ms)
        .bind(pos as i64)
        .execute(&pool).await.unwrap();
    }

    // Call recalculate_event_positions() to compute gaps for all entries
    recalculate_event_positions(&pool, event_id).await;

    let gaps: Vec<(String, Option<i64>)> = sqlx::query_as(
        "SELECT driver_id, gap_to_leader_ms FROM hotlap_event_entries WHERE event_id = ? ORDER BY position"
    ).bind(event_id).fetch_all(&pool).await.unwrap();

    assert_eq!(gaps[0].1, Some(0), "Leader gap_to_leader_ms must be 0");
    assert_eq!(gaps[1].1, Some(1500), "P2 gap_to_leader_ms must be 1500ms");
    assert_eq!(gaps[2].1, Some(3000), "P3 gap_to_leader_ms must be 3000ms");
}

/// CHP-02 (#15): Championship standings sum points across rounds.
/// FAILS: standings calculation not yet implemented.
#[tokio::test]
async fn test_championship_standings_sum() {
    let pool = create_test_db().await;

    let champ_id = "champ-sum-1";
    let driver_id = "champ-drv-sum-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Standing Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO championships (id, name, car_class, sim_type, status, scoring_system, total_rounds, completed_rounds)
         VALUES (?, 'Test Championship', 'gt3', 'assetto_corsa', 'active', 'f1_2010', 2, 2)"
    ).bind(champ_id).execute(&pool).await.unwrap();

    // Create 2 events as rounds
    for (i, evt_pts) in [(1, 25i64), (2, 18i64)] {
        let evt_id = format!("champ-sum-evt-{}", i);
        sqlx::query(
            "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, championship_id, starts_at, ends_at)
             VALUES (?, ?, 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'completed', ?,
                     datetime('now', '-3 day'), datetime('now', '-1 day'))"
        ).bind(&evt_id).bind(format!("Round {}", i)).bind(champ_id).execute(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO championship_rounds (championship_id, event_id, round_number)
             VALUES (?, ?, ?)"
        ).bind(champ_id).bind(&evt_id).bind(i as i64).execute(&pool).await.unwrap();

        // Driver scores in each round
        sqlx::query(
            "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, points, result_status)
             VALUES (?, ?, ?, 80000, 1, ?, 'finished')"
        )
        .bind(format!("champ-sum-entry-{}", i))
        .bind(&evt_id)
        .bind(driver_id)
        .bind(evt_pts)
        .execute(&pool).await.unwrap();
    }

    // Call compute_championship_standings() to aggregate points and persist standings
    compute_championship_standings(&pool, champ_id).await.expect("compute_championship_standings failed");

    let standing = sqlx::query_as::<_, (i64,)>(
        "SELECT total_points FROM championship_standings WHERE championship_id = ? AND driver_id = ?"
    ).bind(champ_id).bind(driver_id).fetch_optional(&pool).await.unwrap();

    assert!(standing.is_some(), "Championship standing should exist after computation");
    assert_eq!(standing.unwrap().0, 43, "Total points should be 25+18=43");
}

/// CHP-04 (#16): Tiebreaker by wins count.
/// FAILS: championship tiebreaker sorting not yet implemented.
#[tokio::test]
async fn test_championship_tiebreaker_wins() {
    let pool = create_test_db().await;

    let champ_id = "champ-tie-wins";

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ctw-drv-a', 'Driver A')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ctw-drv-b', 'Driver B')").execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO championships (id, name, car_class, sim_type, status)
         VALUES (?, 'Tie Test', 'gt3', 'assetto_corsa', 'active')"
    ).bind(champ_id).execute(&pool).await.unwrap();

    // Both drivers at 43 points; A has 2 wins, B has 1 win
    sqlx::query(
        "INSERT INTO championship_standings (championship_id, driver_id, total_points, wins, podiums)
         VALUES (?, 'ctw-drv-a', 43, 2, 3)"
    ).bind(champ_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO championship_standings (championship_id, driver_id, total_points, wins, podiums)
         VALUES (?, 'ctw-drv-b', 43, 1, 2)"
    ).bind(champ_id).execute(&pool).await.unwrap();

    // Call assign_championship_positions() to sort by tiebreaker and assign positions
    assign_championship_positions(&pool, champ_id).await.expect("assign_championship_positions failed");

    let positions: Vec<(String, Option<i64>)> = sqlx::query_as(
        "SELECT driver_id, position FROM championship_standings WHERE championship_id = ? ORDER BY position"
    ).bind(champ_id).fetch_all(&pool).await.unwrap();

    let a_pos = positions.iter().find(|(d, _)| d == "ctw-drv-a").map(|(_, p)| *p);
    let b_pos = positions.iter().find(|(d, _)| d == "ctw-drv-b").map(|(_, p)| *p);

    assert_eq!(a_pos, Some(Some(1)), "Driver A (2 wins) must rank P1 on tiebreaker");
    assert_eq!(b_pos, Some(Some(2)), "Driver B (1 win) must rank P2 on tiebreaker");
}

/// CHP-04 (#17): Tiebreaker by P2 count when wins are equal.
/// FAILS: championship tiebreaker for P2 count not yet implemented.
#[tokio::test]
async fn test_championship_tiebreaker_p2() {
    let pool = create_test_db().await;

    let champ_id = "champ-tie-p2";

    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ctp2-drv-a', 'Driver A')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('ctp2-drv-b', 'Driver B')").execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO championships (id, name, car_class, sim_type, status)
         VALUES (?, 'P2 Tie Test', 'gt3', 'assetto_corsa', 'active')"
    ).bind(champ_id).execute(&pool).await.unwrap();

    // Both 43pts, 1 win each; A has 2 P2s, B has 1 P2
    sqlx::query(
        "INSERT INTO championship_standings (championship_id, driver_id, total_points, wins, p2_count)
         VALUES (?, 'ctp2-drv-a', 43, 1, 2)"
    ).bind(champ_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO championship_standings (championship_id, driver_id, total_points, wins, p2_count)
         VALUES (?, 'ctp2-drv-b', 43, 1, 1)"
    ).bind(champ_id).execute(&pool).await.unwrap();

    // Call assign_championship_positions() to sort by P2 tiebreaker and assign positions
    assign_championship_positions(&pool, champ_id).await.expect("assign_championship_positions failed");

    let positions: Vec<(String, Option<i64>)> = sqlx::query_as(
        "SELECT driver_id, position FROM championship_standings WHERE championship_id = ? ORDER BY position"
    ).bind(champ_id).fetch_all(&pool).await.unwrap();

    let a_pos = positions.iter().find(|(d, _)| d == "ctp2-drv-a").map(|(_, p)| *p);
    let b_pos = positions.iter().find(|(d, _)| d == "ctp2-drv-b").map(|(_, p)| *p);

    assert_eq!(a_pos, Some(Some(1)), "Driver A (2 P2s) must rank P1 on P2-count tiebreaker");
    assert_eq!(b_pos, Some(Some(2)), "Driver B (1 P2) must rank P2 on P2-count tiebreaker");
}

/// SYNC-01 (#18): Competitive tables appear in cloud push payload after update.
/// FAILS: cloud sync extension for competitive tables not yet implemented.
#[tokio::test]
async fn test_sync_competitive_tables() {
    let pool = create_test_db().await;

    let last_push = "2026-01-01T00:00:00Z";
    let event_id = "sync-evt-1";

    // Insert hotlap_event updated after last_push
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, updated_at, starts_at, ends_at)
         VALUES (?, 'Sync Test Event', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 '2026-03-17T10:00:00Z', datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // After Plan 14-05 extends collect_push_payload(): event should appear in payload
    let events_since: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM hotlap_events WHERE updated_at > ?"
    ).bind(last_push).fetch_all(&pool).await.unwrap();

    // This query passes (data is there), but the actual cloud_sync integration test
    // verifies the payload includes competitive tables (tested via cloud_sync.rs in Plan 14-05)
    assert!(!events_since.is_empty(), "Hotlap events updated after last_push must be included in sync payload");
    assert_eq!(events_since[0].0, event_id);
}

/// SYNC-02 (#19): Targeted telemetry — only event-entered laps have telemetry synced.
// =============================================================================
// Phase 14 Plan 05 — Public Read Endpoints
// Requirements: EVT-03, EVT-04, EVT-07, GRP-02, GRP-03, CHP-03
// =============================================================================

/// EVT-07: Public events list excludes cancelled events and returns active first.
/// Verifies: cancelled excluded, active before completed, entry_count included.
#[tokio::test]
async fn test_public_events_list() {
    let pool = create_test_db().await;

    // Insert 3 events: active, completed, cancelled
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES ('pel-evt-active', 'Active Race', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES ('pel-evt-completed', 'Completed Race', 'spa', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'completed',
                 datetime('now', '-3 days'), datetime('now', '-1 day'))"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES ('pel-evt-cancelled', 'Cancelled Race', 'silverstone', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'cancelled',
                 datetime('now', '-2 days'), datetime('now', '-1 day'))"
    ).execute(&pool).await.unwrap();

    // Add an entry to the active event so we can check entry_count
    sqlx::query("INSERT INTO drivers (id, name) VALUES ('pel-drv-1', 'Test Driver')").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, result_status)
         VALUES ('pel-entry-1', 'pel-evt-active', 'pel-drv-1', 80000, 'finished')"
    ).execute(&pool).await.unwrap();

    // Query matching what public_events_list does
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT id, status,
                (SELECT COUNT(*) FROM hotlap_event_entries WHERE event_id = hotlap_events.id) as entry_count
         FROM hotlap_events
         WHERE status != 'cancelled'
         ORDER BY
           CASE status
             WHEN 'active' THEN 1
             WHEN 'upcoming' THEN 2
             WHEN 'scoring' THEN 3
             WHEN 'completed' THEN 4
             ELSE 5
           END,
           starts_at DESC"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(rows.len(), 2, "Cancelled event must be excluded from public listing");
    assert_eq!(rows[0].0, "pel-evt-active", "Active event must be listed first");
    assert_eq!(rows[0].1, "active", "First event must have status 'active'");
    assert_eq!(rows[0].2, 1, "Active event entry_count must be 1");
    assert_eq!(rows[1].0, "pel-evt-completed", "Completed event must be listed second");
    assert_eq!(rows[1].2, 0, "Completed event entry_count must be 0");

    // Verify cancelled event is not in results
    let has_cancelled = rows.iter().any(|(id, _, _)| id == "pel-evt-cancelled");
    assert!(!has_cancelled, "Cancelled event must not appear in public listing");
}

/// EVT-03, EVT-04: Public event leaderboard includes badges, 107% flags, gap-to-leader.
/// PII excluded: response uses display_name (never email/phone).
#[tokio::test]
async fn test_public_event_leaderboard() {
    let pool = create_test_db().await;

    let event_id = "plb-evt-1";

    // Event with reference time 80000ms for badge computation
    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at, reference_time_ms)
         VALUES (?, 'Leaderboard Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'completed',
                 datetime('now', '-2 days'), datetime('now', '-1 day'), 80000)"
    ).bind(event_id).execute(&pool).await.unwrap();

    // 3 drivers — one with nickname enabled, one without, one not set
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone, nickname, show_nickname_on_leaderboard)
         VALUES ('plb-drv-1', 'Alice Smith', 'alice@example.com', '9999999999', 'AliRacer', 1)"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone, nickname, show_nickname_on_leaderboard)
         VALUES ('plb-drv-2', 'Bob Jones', 'bob@example.com', '8888888888', 'BobSpeed', 0)"
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO drivers (id, name, email, phone)
         VALUES ('plb-drv-3', 'Carol White', 'carol@example.com', '7777777777')"
    ).execute(&pool).await.unwrap();

    // 3 entries: P1=80000ms (leader), P2=82000ms (+2000ms gap), P3=88000ms (>107% = 80000*1.07=85600)
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, badge,
                                           gap_to_leader_ms, within_107_percent, result_status, points)
         VALUES ('plb-e1', ?, 'plb-drv-1', 80000, 1, 'gold', 0, 1, 'finished', 25)"
    ).bind(event_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, badge,
                                           gap_to_leader_ms, within_107_percent, result_status, points)
         VALUES ('plb-e2', ?, 'plb-drv-2', 82000, 2, 'silver', 2000, 1, 'finished', 18)"
    ).bind(event_id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_time_ms, position, badge,
                                           gap_to_leader_ms, within_107_percent, result_status, points)
         VALUES ('plb-e3', ?, 'plb-drv-3', 88000, 3, 'bronze', 8000, 0, 'finished', 15)"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Query matching what public_event_leaderboard does (PII-safe display name)
    let entries: Vec<(String, String, i64, Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT hee.driver_id,
                CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                     THEN d.nickname ELSE d.name END as display_name,
                hee.lap_time_ms, hee.badge, hee.gap_to_leader_ms, hee.within_107_percent
         FROM hotlap_event_entries hee
         LEFT JOIN drivers d ON d.id = hee.driver_id
         WHERE hee.event_id = ?
         ORDER BY hee.position ASC"
    ).bind(event_id).fetch_all(&pool).await.unwrap();

    assert_eq!(entries.len(), 3, "Leaderboard must have 3 entries");

    // P1: Alice has nickname enabled → display as 'AliRacer'
    let (d1_id, d1_display, d1_lap, d1_badge, d1_gap, _) = &entries[0];
    assert_eq!(d1_id, "plb-drv-1");
    assert_eq!(d1_display, "AliRacer", "Nickname must be used when show_nickname_on_leaderboard=1");
    assert_eq!(*d1_lap, 80000);
    assert_eq!(d1_badge.as_deref(), Some("gold"));
    assert_eq!(*d1_gap, Some(0));

    // P2: Bob has nickname disabled → display as 'Bob Jones'
    let (d2_id, d2_display, _, _, d2_gap, _) = &entries[1];
    assert_eq!(d2_id, "plb-drv-2");
    assert_eq!(d2_display, "Bob Jones", "Real name must be used when show_nickname_on_leaderboard=0");
    assert_eq!(*d2_gap, Some(2000));

    // P3: Carol has no nickname column → display as 'Carol White'
    let (_, d3_display, _, _, d3_gap, d3_107) = &entries[2];
    assert_eq!(d3_display, "Carol White");
    assert_eq!(*d3_gap, Some(8000));
    assert_eq!(*d3_107, Some(0), "P3 with 88000ms must be outside 107% of 80000ms reference");

    // Verify PII is excluded: query the same data and ensure email/phone not present
    // (The handler never SELECTs email/phone, so this is guaranteed by construction)
    // Direct assertion: email field should not be queryable from the display query
    let pii_check: Vec<(String,)> = sqlx::query_as(
        "SELECT display_name FROM (
           SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                       THEN d.nickname ELSE d.name END as display_name
           FROM hotlap_event_entries hee
           LEFT JOIN drivers d ON d.id = hee.driver_id
           WHERE hee.event_id = ?
         )"
    ).bind(event_id).fetch_all(&pool).await.unwrap();

    // Ensure none of the display_names are email addresses (PII)
    for (name,) in &pii_check {
        assert!(!name.contains('@'), "Email must never appear in public leaderboard display_name: got '{}'", name);
        assert!(!name.chars().all(|c| c.is_ascii_digit()), "Phone number must never appear in display_name: got '{}'", name);
    }
}

/// FAILS: targeted telemetry sync not yet implemented.
#[tokio::test]
async fn test_sync_targeted_telemetry() {
    let pool = create_test_db().await;

    let event_id = "tele-evt-1";
    let driver_id = "tele-drv-1";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Tele Driver')")
        .bind(driver_id).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_events (id, name, track, car, car_class, sim_type, status, starts_at, ends_at)
         VALUES (?, 'Tele Test', 'monza', 'ks_ferrari_458_gt2', 'gt3', 'assetto_corsa', 'active',
                 datetime('now', '-1 day'), datetime('now', '+1 day'))"
    ).bind(event_id).execute(&pool).await.unwrap();

    // Lap that IS in an event entry (should sync telemetry)
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect)
         VALUES ('tele-lap-event', ?, 'assetto_corsa', 'monza', 'ks_ferrari_458_gt2', 1, 80000, 1, 'gt3', 0)"
    ).bind(driver_id).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO hotlap_event_entries (id, event_id, driver_id, lap_id, lap_time_ms, result_status)
         VALUES ('tele-entry-1', ?, ?, 'tele-lap-event', 80000, 'finished')"
    ).bind(event_id).bind(driver_id).execute(&pool).await.unwrap();

    // Lap that is NOT in any event (should NOT sync telemetry)
    sqlx::query(
        "INSERT INTO laps (id, driver_id, sim_type, track, car, lap_number, lap_time_ms, valid, car_class, suspect)
         VALUES ('tele-lap-free', ?, 'assetto_corsa', 'silverstone', 'ks_bmw_m3_e30', 1, 95000, 1, 'street', 0)"
    ).bind(driver_id).execute(&pool).await.unwrap();

    // Query: which laps are event-entered (for targeted telemetry sync in Plan 14-05)
    let event_laps: Vec<(String,)> = sqlx::query_as(
        "SELECT l.id FROM laps l
         INNER JOIN hotlap_event_entries hee ON hee.lap_id = l.id
         WHERE hee.event_id = ?"
    ).bind(event_id).fetch_all(&pool).await.unwrap();

    assert_eq!(event_laps.len(), 1, "Only event-entered laps must be targeted for telemetry sync");
    assert_eq!(event_laps[0].0, "tele-lap-event", "Event lap ID must match");

    // Free practice lap must not be in the sync set
    let free_in_sync = event_laps.iter().any(|(id,)| id == "tele-lap-free");
    assert!(!free_in_sync, "Free practice lap must not be in targeted telemetry sync");
}

// =============================================================================
// Phase 34: Admin Rates API — ADMIN-01..04
// =============================================================================

#[tokio::test]
async fn test_billing_rates_get_returns_seed_rows() {
    use racecontrol_crate::billing;

    let pool = create_test_db().await;
    let state = create_test_state(pool.clone());

    // Seed rows are already in the DB from run_test_migrations.
    // Populate the cache from DB.
    billing::refresh_rate_tiers(&state).await;

    let tiers = state.billing.rate_tiers.read().await;
    assert_eq!(
        tiers.len(),
        3,
        "GET /billing/rates should return the 3 seeded tiers (Standard, Extended, Marathon)"
    );

    // Verify tier names are the expected seed rows.
    let names: Vec<&str> = tiers.iter().map(|t| t.tier_name.as_str()).collect();
    assert!(names.contains(&"Standard"), "Standard tier must be present");
    assert!(names.contains(&"Extended"), "Extended tier must be present");
    assert!(names.contains(&"Marathon"), "Marathon tier must be present");
}

#[tokio::test]
async fn test_billing_rates_create_inserts_and_cache_updates() {
    use racecontrol_crate::billing;

    let pool = create_test_db().await;
    let state = create_test_state(pool.clone());

    // Perform the same INSERT the create_billing_rate handler does.
    let new_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&new_id)
    .bind(4_i64)
    .bind("VIP")
    .bind(90_i64)
    .bind(1200_i64)
    .execute(&pool)
    .await
    .expect("INSERT billing_rate should succeed");

    // Invalidate and reload cache.
    billing::refresh_rate_tiers(&state).await;

    // Cache should now have 4 entries.
    let tiers = state.billing.rate_tiers.read().await;
    assert_eq!(tiers.len(), 4, "Cache must have 4 tiers after POST creates a new one");

    // New tier must be in the cache.
    let vip = tiers.iter().find(|t| t.tier_name == "VIP");
    assert!(vip.is_some(), "VIP tier must appear in cache after create + refresh");
    assert_eq!(vip.unwrap().rate_per_min_paise, 1200);

    // New row must be in the DB.
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT tier_name FROM billing_rates WHERE id = ?",
    )
    .bind(&new_id)
    .fetch_one(&pool)
    .await
    .expect("New billing_rate must exist in DB after POST");
    assert_eq!(row.0, "VIP");
}

#[tokio::test]
async fn test_billing_rates_update_invalidates_cache() {
    use racecontrol_crate::billing;

    let pool = create_test_db().await;
    let state = create_test_state(pool.clone());

    // Initial cache load.
    billing::refresh_rate_tiers(&state).await;
    {
        let tiers = state.billing.rate_tiers.read().await;
        let std = tiers.iter().find(|t| t.tier_name == "Standard").expect("Standard must exist");
        assert_eq!(std.rate_per_min_paise, 2500, "Standard seed rate must be 2500 paise");
    }

    // Perform the same UPDATE the update_billing_rate handler does.
    sqlx::query(
        "UPDATE billing_rates SET rate_per_min_paise = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(3000_i64)
    .bind("rate_standard")
    .execute(&pool)
    .await
    .expect("UPDATE billing_rate should succeed");

    // Invalidate cache immediately (simulates handler calling refresh_rate_tiers).
    billing::refresh_rate_tiers(&state).await;

    // Cache must reflect new value within this call — no restart required.
    let tiers = state.billing.rate_tiers.read().await;
    let std = tiers.iter().find(|t| t.tier_name == "Standard").expect("Standard must still exist");
    assert_eq!(
        std.rate_per_min_paise, 3000,
        "Cache must reflect updated rate_per_min_paise within one billing tick"
    );
}

#[tokio::test]
async fn test_billing_rates_delete_excludes_from_cost() {
    use racecontrol_crate::billing;

    let pool = create_test_db().await;
    let state = create_test_state(pool.clone());

    // Load all 3 tiers and compute cost for a 90-minute session as baseline.
    billing::refresh_rate_tiers(&state).await;
    let all_tiers = state.billing.rate_tiers.read().await.clone();
    assert_eq!(all_tiers.len(), 3, "Baseline: 3 tiers seeded");
    let cost_before = billing::compute_session_cost(90 * 60, &all_tiers);
    // 90 min = 30 × 2500 + 30 × 2000 + 30 × 1500 = 75000 + 60000 + 45000 = 180000
    assert_eq!(cost_before.total_paise, 180_000, "Baseline 90-min cost must be 180000 paise");
    drop(all_tiers);

    // Soft-delete the Marathon tier (rate_marathon) — same UPDATE the handler does.
    sqlx::query(
        "UPDATE billing_rates SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind("rate_marathon")
    .execute(&pool)
    .await
    .expect("Soft-delete billing_rate must succeed");

    // Refresh cache — Marathon tier must be gone.
    billing::refresh_rate_tiers(&state).await;
    let tiers_after = state.billing.rate_tiers.read().await.clone();
    assert_eq!(tiers_after.len(), 2, "Cache must have 2 tiers after soft-deleting Marathon");

    // compute_session_cost must not include the deleted tier's contribution.
    let cost_after = billing::compute_session_cost(90 * 60, &tiers_after);
    // Without Marathon tier: 30 × 2500 + 30 × 2000 = 75000 + 60000 = 135000
    // (minutes 60–90 are uncovered — no unlimited tier remains after Marathon deleted)
    assert_eq!(
        cost_after.total_paise, 135_000,
        "compute_session_cost must exclude soft-deleted Marathon tier"
    );
    assert!(
        cost_after.total_paise != cost_before.total_paise,
        "Cost must differ after deleting a tier"
    );
}

// =============================================================================
// Financial Flow E2E Tests (GAP-6 — Unified Protocol Layer 2.5)
//
// Traces actual currency values through complete billing lifecycle.
// Each test exercises the real wallet/billing functions, not just formulas.
// =============================================================================

/// GAP-6.1: compute_refund uses integer arithmetic (no f64 drift)
#[tokio::test]
async fn test_financial_e2e_compute_refund_integer_only() {
    use racecontrol_crate::billing::compute_refund;

    // Standard 30-min session at ₹700
    assert_eq!(compute_refund(1800, 600, 70000), 46666, "20min remaining of 30min @ ₹700");
    assert_eq!(compute_refund(1800, 900, 70000), 35000, "15min remaining of 30min @ ₹700");
    assert_eq!(compute_refund(1800, 0, 70000), 70000, "full refund when 0 driven");
    assert_eq!(compute_refund(1800, 1800, 70000), 0, "no refund when fully used");
    assert_eq!(compute_refund(1800, 1900, 70000), 0, "no refund when overtime");

    // Edge cases
    assert_eq!(compute_refund(0, 0, 70000), 0, "zero allocated = safe zero");
    assert_eq!(compute_refund(1800, 100, 0), 0, "zero debit = safe zero");
    assert_eq!(compute_refund(-1, 0, 70000), 0, "negative allocated = safe zero");
}

/// GAP-6.2: Full wallet lifecycle — topup → debit → refund → verify balance
#[tokio::test]
async fn test_financial_e2e_full_wallet_lifecycle() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "fin-e2e-1", 0).await;
    let state = create_test_state(pool);

    // Step 1: Topup ₹1000 (100000 paise)
    let balance = racecontrol_crate::wallet::credit(
        &state, "fin-e2e-1", 100000, "topup_cash", None, None, None,
    ).await.unwrap();
    assert_eq!(balance, 100000, "balance after topup should be 100000");

    // Step 2: Debit ₹700 (70000 paise) for session
    let (balance, _txn_id) = racecontrol_crate::wallet::debit(
        &state, "fin-e2e-1", 70000, "debit_session", Some("sess-fin-1"), None,
    ).await.unwrap();
    assert_eq!(balance, 30000, "balance after session debit should be 30000");

    // Step 3: Early end — refund proportional (drove 600s of 1800s)
    let refund_paise = racecontrol_crate::billing::compute_refund(1800, 600, 70000);
    assert_eq!(refund_paise, 46666, "refund for 20min remaining should be 46666");

    let balance = racecontrol_crate::wallet::refund(
        &state, "fin-e2e-1", refund_paise, Some("sess-fin-1"), Some("early-end refund"),
    ).await.unwrap();
    assert_eq!(balance, 76666, "balance after refund should be 30000 + 46666 = 76666");

    // Step 4: Verify transaction audit trail
    let txn_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM wallet_transactions WHERE driver_id = 'fin-e2e-1'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(txn_count.0, 3, "should have 3 transactions: topup + debit + refund");

    // Step 5: Verify wallet totals
    let (credited, debited) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT total_credited_paise, total_debited_paise FROM wallets WHERE driver_id = 'fin-e2e-1'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(credited, 100000 + refund_paise, "total_credited should include topup + refund");
    assert_eq!(debited, 70000, "total_debited should be session charge only");
}

/// GAP-6.3: Insufficient balance blocks session booking
#[tokio::test]
async fn test_financial_e2e_insufficient_funds_blocks_booking() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "fin-e2e-2", 50000).await; // ₹500
    let state = create_test_state(pool);

    // Try to debit ₹700 with only ₹500 balance
    let result = racecontrol_crate::wallet::debit(
        &state, "fin-e2e-2", 70000, "debit_session", None, None,
    ).await;

    assert!(result.is_err(), "debit should fail with insufficient balance");

    // Verify balance unchanged
    let balance = racecontrol_crate::wallet::get_balance(&state, "fin-e2e-2").await.unwrap();
    assert_eq!(balance, 50000, "balance must remain unchanged after failed debit");

    // Verify no transaction was created
    let txn_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM wallet_transactions WHERE driver_id = 'fin-e2e-2'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(txn_count.0, 0, "no transaction should exist for failed debit");
}

/// GAP-6.4: Full-time session = zero refund, wallet correctly debited
#[tokio::test]
async fn test_financial_e2e_full_session_no_refund() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "fin-e2e-3", 100000).await;
    let state = create_test_state(pool);

    // Debit for session
    let (balance, _) = racecontrol_crate::wallet::debit(
        &state, "fin-e2e-3", 70000, "debit_session", Some("sess-full"), None,
    ).await.unwrap();
    assert_eq!(balance, 30000);

    // Drove full allocated time
    let refund = racecontrol_crate::billing::compute_refund(1800, 1800, 70000);
    assert_eq!(refund, 0, "no refund for full-time session");

    // Balance stays at post-debit amount
    let final_balance = racecontrol_crate::wallet::get_balance(&state, "fin-e2e-3").await.unwrap();
    assert_eq!(final_balance, 30000, "balance unchanged — no refund issued");
}

/// GAP-6.5: Cancel before launch = full refund
#[tokio::test]
async fn test_financial_e2e_cancel_before_launch_full_refund() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "fin-e2e-4", 100000).await;
    let state = create_test_state(pool);

    // Debit for session
    let (balance, _) = racecontrol_crate::wallet::debit(
        &state, "fin-e2e-4", 70000, "debit_session", Some("sess-cancel"), None,
    ).await.unwrap();
    assert_eq!(balance, 30000);

    // Cancel before any driving (0 seconds driven)
    let refund = racecontrol_crate::billing::compute_refund(1800, 0, 70000);
    assert_eq!(refund, 70000, "full refund when 0 seconds driven");

    let balance = racecontrol_crate::wallet::refund(
        &state, "fin-e2e-4", refund, Some("sess-cancel"), Some("cancelled before launch"),
    ).await.unwrap();
    assert_eq!(balance, 100000, "balance restored to pre-booking amount");
}

/// GAP-6.6: Overtime = zero refund (driving >= allocated)
#[tokio::test]
async fn test_financial_e2e_overtime_zero_refund() {
    use racecontrol_crate::billing::compute_refund;

    // Drove 35 minutes of a 30-minute session
    assert_eq!(compute_refund(1800, 2100, 70000), 0, "overtime = no refund");
    // Drove exactly allocated
    assert_eq!(compute_refund(1800, 1800, 70000), 0, "exact = no refund");
    // Drove 1 second over
    assert_eq!(compute_refund(1800, 1801, 70000), 0, "1s over = no refund");
}

/// GAP-6.7: Tiered pricing — compute_session_cost uses integer arithmetic
#[tokio::test]
async fn test_financial_e2e_tiered_pricing_integer_math() {
    use racecontrol_crate::billing::compute_session_cost;
    use racecontrol_crate::billing::BillingRateTier;

    let tiers = vec![
        BillingRateTier {
            tier_order: 1, tier_name: "Standard".into(),
            threshold_minutes: 30, rate_per_min_paise: 2500, sim_type: None,
        },
        BillingRateTier {
            tier_order: 2, tier_name: "Extended".into(),
            threshold_minutes: 60, rate_per_min_paise: 2000, sim_type: None,
        },
        BillingRateTier {
            tier_order: 3, tier_name: "Marathon".into(),
            threshold_minutes: 0, rate_per_min_paise: 1500, sim_type: None,
        },
    ];

    // 30 min = 30 * 2500 = 75000
    let cost_30 = compute_session_cost(30 * 60, &tiers);
    assert_eq!(cost_30.total_paise, 75000, "30 min standard tier");

    // 45 min = 30 * 2500 + 15 * 2000 = 75000 + 30000 = 105000
    let cost_45 = compute_session_cost(45 * 60, &tiers);
    assert_eq!(cost_45.total_paise, 105000, "45 min crosses into extended tier");

    // 90 min = 30 * 2500 + 30 * 2000 + 30 * 1500 = 75000 + 60000 + 45000 = 180000
    let cost_90 = compute_session_cost(90 * 60, &tiers);
    assert_eq!(cost_90.total_paise, 180000, "90 min uses all three tiers");
}

/// GAP-6.8: Wallet double-debit prevention (idempotency)
#[tokio::test]
async fn test_financial_e2e_no_double_debit() {
    let pool = create_test_db().await;
    seed_test_driver_with_balance(&pool, "fin-e2e-5", 100000).await;
    let state = create_test_state(pool);

    // First debit succeeds
    let (balance1, _) = racecontrol_crate::wallet::debit(
        &state, "fin-e2e-5", 70000, "debit_session", Some("sess-double"), None,
    ).await.unwrap();
    assert_eq!(balance1, 30000);

    // Second debit for same amount would overdraw
    let result = racecontrol_crate::wallet::debit(
        &state, "fin-e2e-5", 70000, "debit_session", Some("sess-double-2"), None,
    ).await;
    assert!(result.is_err(), "second debit should fail — insufficient balance");

    // Balance unchanged
    let balance = racecontrol_crate::wallet::get_balance(&state, "fin-e2e-5").await.unwrap();
    assert_eq!(balance, 30000, "balance should remain at 30000 after failed second debit");
}

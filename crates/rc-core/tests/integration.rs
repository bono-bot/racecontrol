//! Integration tests for RaceControl core: wallet, billing, ports, leaderboard.
//!
//! Uses in-memory SQLite so tests are fast and isolated.

use std::sync::Arc;

use rc_common::types::{BillingSessionStatus, DrivingState};
use sqlx::SqlitePool;

// ─── Test Helpers ────────────────────────────────────────────────────────────

/// Create an in-memory SQLite database with all migrations applied.
async fn create_test_db() -> SqlitePool {
    // rc-core's db::init_pool needs a file path, so we build the pool manually
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
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS personal_bests (
            driver_id TEXT REFERENCES drivers(id),
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            best_lap_ms INTEGER NOT NULL,
            lap_id TEXT REFERENCES laps(id),
            achieved_at TEXT,
            PRIMARY KEY (driver_id, track, car)
        )"
    ).execute(pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS track_records (
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            driver_id TEXT REFERENCES drivers(id),
            best_lap_ms INTEGER NOT NULL,
            lap_id TEXT REFERENCES laps(id),
            achieved_at TEXT,
            PRIMARY KEY (track, car)
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
}

/// Create a minimal AppState backed by the given pool.
fn create_test_state(pool: SqlitePool) -> Arc<rc_core::state::AppState> {
    let config = rc_core::config::Config::default_test();
    Arc::new(rc_core::state::AppState::new(config, pool))
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
    let balance = rc_core::wallet::credit(
        &state, "wallet-test-1", 100000, "topup_cash", None, None, None,
    ).await.unwrap();
    assert_eq!(balance, 100000, "balance after credit should be 100000");

    // Debit 70000 paise
    let (balance, _txn_id) = rc_core::wallet::debit(
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
    rc_core::wallet::credit(
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
    let result = rc_core::wallet::debit(
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
    let balance = rc_core::wallet::get_balance(&state, "wallet-test-3").await.unwrap();
    assert_eq!(balance, 10000, "balance should remain unchanged at 10000");
}

// =============================================================================
// Task 3: Billing integration tests
// =============================================================================

#[tokio::test]
async fn test_billing_timer_counting() {
    use rc_core::billing::BillingTimer;

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
    use rc_core::billing::BillingTimer;

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
    use rc_core::billing::BillingTimer;

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
    use rc_core::billing::BillingTimer;

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
    let new_balance = rc_core::wallet::refund(
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
    use rc_core::port_allocator::PortAllocator;

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
    use rc_core::port_allocator::PortAllocator;

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
    rc_core::wallet::credit(
        &state, "sync-drv", 10000, "topup_cash", None, Some("txn1"), None,
    ).await.unwrap();
    rc_core::wallet::credit(
        &state, "sync-drv", 20000, "topup_upi", None, Some("txn2"), None,
    ).await.unwrap();
    rc_core::wallet::credit(
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
        created_at: chrono::Utc::now(),
    };

    rc_core::lap_tracker::persist_lap(&state, &lap).await;

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
        created_at: chrono::Utc::now(),
    };

    rc_core::lap_tracker::persist_lap(&state, &lap).await;

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
        created_at: chrono::Utc::now(),
    };

    rc_core::lap_tracker::persist_lap(&state, &lap).await;

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
        created_at: chrono::Utc::now(),
    };

    rc_core::lap_tracker::persist_lap(&state, &lap).await;

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
        created_at: chrono::Utc::now(),
    };

    rc_core::lap_tracker::persist_lap(&state, &lap).await;

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

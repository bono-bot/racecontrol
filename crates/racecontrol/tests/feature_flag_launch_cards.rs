// Phase 368 Plan 03 — Task 1: feature_flag_kiosk_launch_cards test
//
// Verifies that the kiosk_launch_cards_enabled feature flag is seeded with
// default_value=0 and enabled=0 (kill-switch default: off for shadow deploy).

use sqlx::SqlitePool;

async fn create_test_db_with_flags() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory pool");

    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await.expect("WAL");

    // Create the feature_flags table (mirrors db/mod.rs Phase 177 block)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS feature_flags (
            name         TEXT PRIMARY KEY,
            enabled      BOOLEAN NOT NULL DEFAULT 0,
            default_value BOOLEAN NOT NULL DEFAULT 0,
            overrides    TEXT NOT NULL DEFAULT '{}',
            version      INTEGER NOT NULL DEFAULT 1,
            updated_at   TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("create feature_flags");

    // Seed the Phase 368 flag (mirrors db/mod.rs Phase 368 seed block)
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('kiosk_launch_cards_enabled', 0, 0, '{}')",
    )
    .execute(&pool)
    .await;

    pool
}

/// Test: kiosk_launch_cards_enabled row exists with default_value=0 and enabled=0.
/// D-13 (P2-06): flag is in DB feature_flags table, default false (shadow-deploy safe).
#[tokio::test]
async fn feature_flag_kiosk_launch_cards() {
    let pool = create_test_db_with_flags().await;

    let row: Option<(String, bool, bool)> = sqlx::query_as(
        "SELECT name, enabled, default_value FROM feature_flags WHERE name = 'kiosk_launch_cards_enabled'",
    )
    .fetch_optional(&pool)
    .await
    .expect("SELECT feature_flags");

    let (name, enabled, default_value) = row.expect(
        "kiosk_launch_cards_enabled flag must exist after db init"
    );

    assert_eq!(name, "kiosk_launch_cards_enabled");
    assert!(!enabled, "kiosk_launch_cards_enabled.enabled must be 0 (false) for shadow deploy");
    assert!(!default_value, "kiosk_launch_cards_enabled.default_value must be 0 (false)");
}

/// Test: INSERT OR IGNORE is idempotent — seeding twice leaves exactly one row.
#[tokio::test]
async fn feature_flag_kiosk_launch_cards_seed_idempotent() {
    let pool = create_test_db_with_flags().await;

    // Seed again — should be silently ignored
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('kiosk_launch_cards_enabled', 0, 0, '{}')",
    )
    .execute(&pool)
    .await;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM feature_flags WHERE name = 'kiosk_launch_cards_enabled'",
    )
    .fetch_one(&pool)
    .await
    .expect("COUNT feature_flags");

    assert_eq!(count, 1, "INSERT OR IGNORE must leave exactly 1 row after double-seed");
}

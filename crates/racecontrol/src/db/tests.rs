// ─── Phase 303: venue_id migration tests ──────────────────────────────────────

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

// ─── Phase 363: Data Recording Verification migration tests ───────────────────

mod phase363_migration_tests {
    use super::*;

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

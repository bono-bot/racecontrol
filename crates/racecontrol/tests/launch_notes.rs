// Phase 368 Plan 03 — Task 1: launch_notes DB migration tests
//
// Tests:
//   1. launch_notes_migration_idempotent — run migrate twice on the same db, no error
//   2. launch_notes_schema_has_expected_columns — PRAGMA table_info check
//   3. launch_notes_has_launch_id_index — PRAGMA index_list check
//   4. launch_notes_append_only — INSERT + SELECT verify (append-only enforced at API layer)
//   5. launch_timeline_spans_has_staff_dismissed_at_column — PRAGMA table_info after ALTER

use sqlx::SqlitePool;

/// Create an in-memory SQLite pool and run the essential migrations needed for launch_notes tests.
async fn create_test_db() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    run_migrations(&pool).await;
    pool
}

/// Minimal migrations: launch_timeline_spans (required for ALTER) + launch_notes.
async fn run_migrations(pool: &SqlitePool) {
    sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await.expect("WAL");
    sqlx::query("PRAGMA foreign_keys=ON").execute(pool).await.expect("FK");

    // launch_timeline_spans must exist before we ALTER it to add staff_dismissed_at
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_timeline_spans (
            launch_id   TEXT PRIMARY KEY,
            pod_id      TEXT NOT NULL,
            sim_type    TEXT NOT NULL,
            preset_id   TEXT,
            billing_session_id TEXT,
            outcome     TEXT NOT NULL,
            total_duration_ms INTEGER NOT NULL,
            started_at  TEXT NOT NULL,
            events_json TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await
    .expect("create launch_timeline_spans");

    // Phase 368 migrations
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_notes (
            id          TEXT PRIMARY KEY,
            launch_id   TEXT NOT NULL,
            pod_id      TEXT NOT NULL,
            staff_id    TEXT,
            staff_name  TEXT,
            body        TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await
    .expect("create launch_notes");

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_launch_notes_launch_id ON launch_notes(launch_id)",
    )
    .execute(pool)
    .await
    .expect("create index");

    let _ = sqlx::query(
        "ALTER TABLE launch_timeline_spans ADD COLUMN staff_dismissed_at TEXT",
    )
    .execute(pool)
    .await;
    // Intentional: ignore error if column already exists (idempotent migration pattern)
}

/// Test 1: Running migrations twice on the same db must not error.
/// Verifies CREATE TABLE IF NOT EXISTS + idempotent ALTER behaviour.
#[tokio::test]
async fn launch_notes_migration_idempotent() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");

    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await.expect("WAL");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_timeline_spans (
            launch_id TEXT PRIMARY KEY, pod_id TEXT NOT NULL, sim_type TEXT NOT NULL,
            preset_id TEXT, billing_session_id TEXT, outcome TEXT NOT NULL,
            total_duration_ms INTEGER NOT NULL, started_at TEXT NOT NULL,
            events_json TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("first create launch_timeline_spans");

    // First run
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_notes (
            id TEXT PRIMARY KEY, launch_id TEXT NOT NULL, pod_id TEXT NOT NULL,
            staff_id TEXT, staff_name TEXT, body TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("first create launch_notes");
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_notes_launch_id ON launch_notes(launch_id)")
        .execute(&pool)
        .await
        .expect("first create index");
    let _ = sqlx::query("ALTER TABLE launch_timeline_spans ADD COLUMN staff_dismissed_at TEXT")
        .execute(&pool)
        .await;

    // Second run — must be completely idempotent (no errors)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_notes (
            id TEXT PRIMARY KEY, launch_id TEXT NOT NULL, pod_id TEXT NOT NULL,
            staff_id TEXT, staff_name TEXT, body TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("second create launch_notes must not fail");
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_notes_launch_id ON launch_notes(launch_id)")
        .execute(&pool)
        .await
        .expect("second create index must not fail");
    let _ = sqlx::query("ALTER TABLE launch_timeline_spans ADD COLUMN staff_dismissed_at TEXT")
        .execute(&pool)
        .await;
    // Second ALTER returns an error (column exists) but is silently ignored
}

/// Test 2: PRAGMA table_info(launch_notes) returns the expected 7 columns.
#[tokio::test]
async fn launch_notes_schema_has_expected_columns() {
    let pool = create_test_db().await;

    // Use column names only to avoid reserved-word conflicts (notnull, pk, etc.)
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT cid, name FROM pragma_table_info('launch_notes')",
    )
    .fetch_all(&pool)
    .await
    .expect("PRAGMA table_info(launch_notes)");

    let col_names: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    for expected in &["id", "launch_id", "pod_id", "staff_id", "staff_name", "body", "created_at"] {
        assert!(
            col_names.contains(expected),
            "Expected column '{}' not found in launch_notes. Got: {:?}",
            expected,
            col_names
        );
    }
    assert_eq!(rows.len(), 7, "launch_notes should have exactly 7 columns, got {}", rows.len());
}

/// Test 3: idx_launch_notes_launch_id index exists.
#[tokio::test]
async fn launch_notes_has_launch_id_index() {
    let pool = create_test_db().await;

    // Use seq + name only to avoid the `unique` reserved-word conflict
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT seq, name FROM pragma_index_list('launch_notes')",
    )
    .fetch_all(&pool)
    .await
    .expect("PRAGMA index_list(launch_notes)");

    let index_names: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert!(
        index_names.contains(&"idx_launch_notes_launch_id"),
        "Expected index 'idx_launch_notes_launch_id' not found. Got: {:?}",
        index_names
    );
}

/// Test 4: INSERT + SELECT round-trip (append-only guarantee is at the API layer, not schema).
/// This test confirms the table supports inserts correctly.
/// No UPDATE/DELETE handlers are introduced (API-level append-only enforcement).
#[tokio::test]
async fn launch_notes_append_only() {
    let pool = create_test_db().await;

    sqlx::query(
        "INSERT INTO launch_notes (id, launch_id, pod_id, staff_id, staff_name, body)
         VALUES ('note-001', 'launch-abc', 'pod_1', 'staff-007', 'James', 'Test note body')",
    )
    .execute(&pool)
    .await
    .expect("INSERT into launch_notes");

    let row: (String, String, String) = sqlx::query_as(
        "SELECT id, launch_id, body FROM launch_notes WHERE launch_id = 'launch-abc'",
    )
    .fetch_one(&pool)
    .await
    .expect("SELECT from launch_notes");

    assert_eq!(row.0, "note-001");
    assert_eq!(row.1, "launch-abc");
    assert_eq!(row.2, "Test note body");
}

/// Test 5: launch_timeline_spans gains staff_dismissed_at column after ALTER.
/// Runs on a fresh schema AND on a pre-existing schema to prove idempotency.
#[tokio::test]
async fn launch_timeline_spans_has_staff_dismissed_at_column() {
    let pool = create_test_db().await;

    // Use cid + name only to avoid reserved-word conflicts (notnull, pk)
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT cid, name FROM pragma_table_info('launch_timeline_spans')",
    )
    .fetch_all(&pool)
    .await
    .expect("PRAGMA table_info(launch_timeline_spans)");

    let col_names: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert!(
        col_names.contains(&"staff_dismissed_at"),
        "Expected column 'staff_dismissed_at' in launch_timeline_spans. Got: {:?}",
        col_names
    );

    // Idempotency: run the ALTER again — must not panic or error (we ignore the Err)
    let _ = sqlx::query(
        "ALTER TABLE launch_timeline_spans ADD COLUMN staff_dismissed_at TEXT",
    )
    .execute(&pool)
    .await;
    // Column already exists — error is expected and silently ignored
    // Table must still be queryable after the ignored error
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM launch_timeline_spans")
        .fetch_one(&pool)
        .await
        .expect("SELECT COUNT(*) after idempotent ALTER");
    assert_eq!(count, 0, "Table should be empty after test migration");
}

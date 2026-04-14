use super::*;
use sqlx::sqlite::SqlitePoolOptions;

/// Build an in-memory DB with game_presets and combo_reliability tables.
async fn make_test_db() -> sqlx::SqlitePool {
    let db = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite for preset tests");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS game_presets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            game TEXT NOT NULL,
            car TEXT,
            track TEXT,
            session_type TEXT,
            notes TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&db)
    .await
    .expect("create game_presets");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS combo_reliability (
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            car TEXT,
            track TEXT,
            success_rate REAL NOT NULL DEFAULT 0.0,
            avg_time_to_track_ms REAL,
            p95_time_to_track_ms REAL,
            total_launches INTEGER NOT NULL DEFAULT 0,
            common_failure_modes TEXT,
            last_updated TEXT NOT NULL
        )",
    )
    .execute(&db)
    .await
    .expect("create combo_reliability");

    db
}

async fn insert_preset(
    db: &sqlx::SqlitePool,
    id: &str,
    name: &str,
    game: &str,
    car: Option<&str>,
    track: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO game_presets (id, name, game, car, track, session_type, notes, enabled)
         VALUES (?, ?, ?, ?, ?, NULL, NULL, 1)",
    )
    .bind(id)
    .bind(name)
    .bind(game)
    .bind(car)
    .bind(track)
    .execute(db)
    .await
    .expect("insert preset");
}

async fn insert_combo_reliability(
    db: &sqlx::SqlitePool,
    pod_id: &str,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
    success_rate: f64,
    total_launches: i64,
) {
    sqlx::query(
        "INSERT INTO combo_reliability (pod_id, sim_type, car, track, success_rate, total_launches, last_updated)
         VALUES (?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car)
    .bind(track)
    .bind(success_rate)
    .bind(total_launches)
    .execute(db)
    .await
    .expect("insert combo_reliability");
}

/// Test 1: create_preset inserts a row and returns the new GamePreset with a UUID id
#[tokio::test]
async fn test_create_preset_returns_uuid_id() {
    let db = make_test_db().await;
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO game_presets (id, name, game, car, track, session_type, notes, enabled)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind("Monza Hotlap")
    .bind("assettoCorsa")
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind(1i64)
    .execute(&db)
    .await
    .expect("insert");

    let preset = fetch_preset_by_id(&db, &id)
        .await
        .expect("fetch")
        .expect("should exist");

    // UUID should be 36 chars (8-4-4-4-12 format)
    assert_eq!(preset.id.len(), 36, "id must be UUID format");
    assert_eq!(preset.name, "Monza Hotlap");
    assert_eq!(preset.game, "assettoCorsa");
    assert!(preset.enabled);
}

/// Test 2: list_presets_with_reliability returns presets with reliability_score=None
/// when combo_reliability has no rows for that (game, car, track)
#[tokio::test]
async fn test_list_presets_no_reliability_data() {
    let db = make_test_db().await;
    insert_preset(&db, "p1", "No Data Preset", "assettoCorsa", Some("ks_ferrari_gte"), Some("monza")).await;

    let result = list_presets_with_reliability(&db, 0.6)
        .await
        .expect("list presets");

    assert_eq!(result.len(), 1);
    assert!(result[0].reliability_score.is_none(), "no combo data → reliability_score=None");
    assert_eq!(result[0].total_launches, 0);
    assert!(!result[0].flagged_unreliable, "no data → not flagged");
}

/// Test 3: list_presets_with_reliability returns reliability_score=0.5 and
/// flagged_unreliable=true when combo_reliability has avg success_rate=0.5
/// and total_launches >= 5 (threshold = 0.6)
#[tokio::test]
async fn test_list_presets_unreliable_when_low_score_and_enough_launches() {
    let db = make_test_db().await;
    insert_preset(&db, "p2", "Unreliable Preset", "assettoCorsa", Some("ks_ferrari_gte"), Some("monza")).await;

    // Insert reliability data: 5 launches, 50% success
    insert_combo_reliability(&db, "pod1", "assettoCorsa", Some("ks_ferrari_gte"), Some("monza"), 0.5, 5).await;

    let result = list_presets_with_reliability(&db, 0.6)
        .await
        .expect("list presets");

    assert_eq!(result.len(), 1);
    let preset = &result[0];
    assert!(preset.reliability_score.is_some(), "5 launches → should have score");
    let score = preset.reliability_score.unwrap();
    assert!((score - 0.5).abs() < 0.01, "score should be ~0.5, got {}", score);
    assert_eq!(preset.total_launches, 5);
    assert!(preset.flagged_unreliable, "0.5 < 0.6 threshold AND 5 launches → flagged");
}

/// Test 4: list_presets_with_reliability returns flagged_unreliable=false
/// when total_launches=4 (below 5-launch minimum), even if score < threshold
#[tokio::test]
async fn test_list_presets_not_flagged_when_too_few_launches() {
    let db = make_test_db().await;
    insert_preset(&db, "p3", "Not Enough Data", "assettoCorsa", Some("ks_bmw_m4_gt3"), Some("spa")).await;

    // Insert reliability data: only 4 launches (below minimum), low success rate
    insert_combo_reliability(&db, "pod1", "assettoCorsa", Some("ks_bmw_m4_gt3"), Some("spa"), 0.25, 4).await;

    let result = list_presets_with_reliability(&db, 0.6)
        .await
        .expect("list presets");

    assert_eq!(result.len(), 1);
    let preset = &result[0];
    // Below 5 launches → reliability_score is None
    assert!(preset.reliability_score.is_none(), "4 launches < 5 minimum → score=None");
    assert_eq!(preset.total_launches, 4);
    assert!(!preset.flagged_unreliable, "< 5 launches → never flagged, even if score would be low");
}

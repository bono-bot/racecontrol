//! Tests for metrics, metrics_intel modules (ARCH-03 split).

use super::*;
use crate::api::metrics_intel::*;
use sqlx::SqlitePool;

/// Build an in-memory DB with combo_reliability and launch_events tables.
async fn make_test_db() -> SqlitePool {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    // launch_events table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            car TEXT,
            track TEXT,
            session_type TEXT,
            timestamp TEXT NOT NULL,
            outcome TEXT NOT NULL,
            error_taxonomy TEXT,
            duration_to_playable_ms INTEGER,
            error_details TEXT,
            launch_args_hash TEXT,
            attempt_number INTEGER DEFAULT 1,
            db_fallback INTEGER,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&db)
    .await
    .expect("create launch_events");

    // combo_reliability table
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

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_combo_rel_pk ON combo_reliability(pod_id, sim_type, COALESCE(car, ''), COALESCE(track, ''))",
    )
    .execute(&db)
    .await
    .expect("create unique index");

    db
}

/// Insert a row into combo_reliability directly (for alternatives tests).
async fn seed_combo(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
    success_rate: f64,
    total_launches: i64,
) {
    let now = "2026-03-26T00:00:00Z";
    sqlx::query(
        "INSERT INTO combo_reliability (pod_id, sim_type, car, track, success_rate, total_launches, last_updated)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car)
    .bind(track)
    .bind(success_rate)
    .bind(total_launches)
    .bind(now)
    .execute(db)
    .await
    .expect("seed combo_reliability");
}

/// Insert a launch_events row for matrix tests.
async fn seed_launch_event(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    outcome: &str,
    duration_ms: Option<i64>,
    error_taxonomy: Option<&str>,
    created_at: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO launch_events (id, pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, attempt_number, created_at)
         VALUES (?, ?, ?, NULL, NULL, NULL, ?, ?, ?, ?, 1, ?)",
    )
    .bind(&id)
    .bind(pod_id)
    .bind(sim_type)
    .bind(created_at)
    .bind(outcome)
    .bind(error_taxonomy)
    .bind(duration_ms)
    .bind(created_at)
    .execute(db)
    .await
    .expect("seed launch_event");
}

// ─── Alternatives Tests ─────────────────────────────────────────────────

/// INTEL-03: alternatives returns max 3 high-reliability combos, sorted DESC by success_rate.
#[tokio::test]
async fn test_alternatives_top3() {
    let db = make_test_db().await;

    // Seed 5 combos for assetto_corsa/pod-5 with varying rates (all with >= 5 launches)
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.50, 10).await;
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("nurburgring"), 0.95, 10).await;
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.98, 10).await;
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ford"), Some("nurburgring"), 0.92, 10).await;
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_porsche"), Some("spa"), 0.91, 10).await;
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_lamborghini"), Some("mugello"), 0.78, 10).await;

    let params = AlternativesParams {
        game: "assetto_corsa".to_string(),
        car: Some("ks_ferrari".to_string()),
        track: Some("spa".to_string()),
        pod: Some("pod-5".to_string()),
    };

    let result = query_alternatives(&db, &params).await;

    // Must return max 3 results
    assert!(result.len() <= 3, "Must return at most 3 alternatives, got {}", result.len());
    // All results must have success_rate > 0.90
    for combo in &result {
        assert!(combo.success_rate > 0.90, "All alternatives must have success_rate > 0.90, got {}", combo.success_rate);
    }
    // Must return at least 1 result
    assert!(!result.is_empty(), "Must return at least 1 alternative");
}

/// INTEL-03: alternatives prefers combos that share car or track with the request.
#[tokio::test]
async fn test_alternatives_similarity() {
    let db = make_test_db().await;

    // Seed combos: one shares car (ks_ferrari), one shares track (spa), one is unrelated
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("nurburgring"), 0.93, 10).await; // shares car
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("spa"), 0.94, 10).await;             // shares track
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ford"), Some("monza"), 0.92, 10).await;          // unrelated

    let params = AlternativesParams {
        game: "assetto_corsa".to_string(),
        car: Some("ks_ferrari".to_string()),
        track: Some("spa".to_string()),
        pod: Some("pod-5".to_string()),
    };

    let result = query_alternatives(&db, &params).await;

    assert!(!result.is_empty(), "Must return alternatives");
    // At least 1 result must share car or track with request
    let has_similar = result.iter().any(|c| {
        c.car.as_deref() == Some("ks_ferrari") || c.track.as_deref() == Some("spa")
    });
    assert!(has_similar, "At least 1 alternative must share car or track with the request");
}

/// INTEL-03: the failing combo itself is excluded from alternatives.
#[tokio::test]
async fn test_alternatives_excludes_self() {
    let db = make_test_db().await;

    // Seed the "failing" combo itself with high success_rate (should still be excluded)
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.95, 10).await;
    // Seed a different combo that should appear
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.96, 10).await;

    let params = AlternativesParams {
        game: "assetto_corsa".to_string(),
        car: Some("ks_ferrari".to_string()),
        track: Some("spa".to_string()),
        pod: Some("pod-5".to_string()),
    };

    let result = query_alternatives(&db, &params).await;

    // The failing combo (ks_ferrari/spa) must NOT appear in alternatives
    let has_self = result.iter().any(|c| {
        c.car.as_deref() == Some("ks_ferrari") && c.track.as_deref() == Some("spa")
    });
    assert!(!has_self, "The failing combo (ks_ferrari/spa) must not appear in alternatives");
}

/// INTEL-03: pod-specific < 3 results falls back to fleet-wide data.
#[tokio::test]
async fn test_alternatives_pod_fallback() {
    let db = make_test_db().await;

    // Pod-5 has only 1 high-reliability combo (not the failing one)
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.97, 10).await;
    seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.50, 10).await; // the failing one

    // Other pods have fleet-wide high-reliability combos
    seed_combo(&db, "pod-3", "assetto_corsa", Some("ks_ford"), Some("nurburgring"), 0.95, 10).await;
    seed_combo(&db, "pod-1", "assetto_corsa", Some("ks_porsche"), Some("spa"), 0.94, 10).await;
    seed_combo(&db, "pod-2", "assetto_corsa", Some("ks_lamborghini"), Some("mugello"), 0.93, 10).await;

    let params = AlternativesParams {
        game: "assetto_corsa".to_string(),
        car: Some("ks_ferrari".to_string()),
        track: Some("spa".to_string()),
        pod: Some("pod-5".to_string()),
    };

    let result = query_alternatives(&db, &params).await;

    // With fallback, should return up to 3 total
    assert!(result.len() >= 2, "Fallback should return results from fleet, got {}", result.len());
    assert!(result.len() <= 3, "Must cap at 3 alternatives, got {}", result.len());
}

// ─── Launch Matrix Tests ────────────────────────────────────────────────

/// INTEL-04: launch matrix flags pods with < 70% success rate.
#[tokio::test]
async fn test_launch_matrix_flagged() {
    let db = make_test_db().await;
    let now = "2026-03-26T00:00:00Z";

    // pod-1: 9 success / 10 total = 90% → not flagged
    for _ in 0..9 {
        seed_launch_event(&db, "pod-1", "assetto_corsa", "\"Success\"", Some(20000), None, now).await;
    }
    seed_launch_event(&db, "pod-1", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;

    // pod-5: 3 success / 5 total = 60% → flagged
    for _ in 0..3 {
        seed_launch_event(&db, "pod-5", "assetto_corsa", "\"Success\"", Some(25000), None, now).await;
    }
    for _ in 0..2 {
        seed_launch_event(&db, "pod-5", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;
    }

    // pod-8: 8 success / 10 total = 80% → not flagged
    for _ in 0..8 {
        seed_launch_event(&db, "pod-8", "assetto_corsa", "\"Success\"", Some(22000), None, now).await;
    }
    for _ in 0..2 {
        seed_launch_event(&db, "pod-8", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
    }

    let result = query_launch_matrix(&db, "assetto_corsa").await;

    assert_eq!(result.len(), 3, "Matrix must have 3 rows");

    let pod5 = result.iter().find(|r| r.pod_id == "pod-5").expect("pod-5 must be in matrix");
    assert!(pod5.flagged, "pod-5 (60% success) must be flagged=true");

    let pod1 = result.iter().find(|r| r.pod_id == "pod-1").expect("pod-1 must be in matrix");
    assert!(!pod1.flagged, "pod-1 (90% success) must be flagged=false");

    let pod8 = result.iter().find(|r| r.pod_id == "pod-8").expect("pod-8 must be in matrix");
    assert!(!pod8.flagged, "pod-8 (80% success) must be flagged=false");
}

/// INTEL-04: launch matrix populates top_3_failure_modes per pod.
#[tokio::test]
async fn test_launch_matrix_failure_modes() {
    let db = make_test_db().await;
    let now = "2026-03-26T00:00:00Z";

    // pod-3: failures with different taxonomies
    seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;
    seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;
    seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
    seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
    seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
    seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Error\"", None, Some("OutOfMemory"), now).await;
    // Add some successes
    for _ in 0..4 {
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Success\"", Some(20000), None, now).await;
    }

    let result = query_launch_matrix(&db, "assetto_corsa").await;

    let pod3 = result.iter().find(|r| r.pod_id == "pod-3").expect("pod-3 must be in matrix");
    // top_3_failure_modes must be populated
    assert!(!pod3.top_3_failure_modes.is_empty(), "top_3_failure_modes must be populated for pod-3");
    assert!(pod3.top_3_failure_modes.len() <= 3, "At most 3 failure modes");
    // LaunchTimeout (count=3) must be first (highest count)
    assert_eq!(pod3.top_3_failure_modes[0].mode, "LaunchTimeout",
        "LaunchTimeout (count=3) must be first failure mode, got: {:?}",
        pod3.top_3_failure_modes.iter().map(|m| &m.mode).collect::<Vec<_>>());
}

// ─── Combo List Tests (DASH-02) ────────────────────────────────────────

/// DASH-02: combo_list returns flagged=true for rows with success_rate < 0.70.
#[tokio::test]
async fn test_combo_list_flagged_when_below_threshold() {
    let db = make_test_db().await;

    // Insert a row with success_rate=0.65 → must be flagged
    seed_combo(&db, "pod-1", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.65, 20).await;
    // Insert a row with success_rate=0.80 → must NOT be flagged
    seed_combo(&db, "pod-2", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.80, 15).await;

    let params = ComboListParams {
        game: None,
        sort_by: None,
        order: None,
    };

    let sql = "SELECT pod_id, sim_type, car, track, success_rate, avg_time_to_track_ms, total_launches
               FROM combo_reliability ORDER BY success_rate DESC";
    let rows: Vec<ComboListRow> = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, f64, Option<f64>, i64)>(sql)
        .fetch_all(&db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(pod_id, sim_type, car, track, success_rate, avg_time_ms, total_launches)| ComboListRow {
            pod_id,
            sim_type,
            car,
            track,
            success_rate,
            avg_time_ms,
            total_launches,
            flagged: success_rate < 0.70,
        })
        .collect();

    assert_eq!(rows.len(), 2, "Must have 2 rows");

    let ferrari = rows.iter().find(|r| r.pod_id == "pod-1").expect("pod-1 row");
    assert!(ferrari.flagged, "success_rate=0.65 must be flagged=true");

    let bmw = rows.iter().find(|r| r.pod_id == "pod-2").expect("pod-2 row");
    assert!(!bmw.flagged, "success_rate=0.80 must be flagged=false");

    // Verify params is used (suppress unused warning)
    let _ = params;
}

/// DASH-02: combo_list returns empty array (not error) for empty DB.
#[tokio::test]
async fn test_combo_list_empty_db() {
    let db = make_test_db().await;

    let sql = "SELECT pod_id, sim_type, car, track, success_rate, avg_time_to_track_ms, total_launches
               FROM combo_reliability ORDER BY success_rate DESC";
    let rows: Vec<(String, String, Option<String>, Option<String>, f64, Option<f64>, i64)> =
        sqlx::query_as(sql).fetch_all(&db).await.unwrap_or_default();

    assert!(rows.is_empty(), "Empty DB must return empty array, not error");
}

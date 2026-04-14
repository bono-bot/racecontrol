//! Combo reliability — rolling 30-day success rate tracking per (pod, sim, car, track) combo.
//! Extracted from metrics.rs as part of ARCH-03.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// A combo reliability record — rolling 30-day success rate for a (pod, sim, car, track) combo.
/// Minimum 5 launches required for query_combo_reliability to return a result (INTEL-02).
#[derive(Debug, Clone, Serialize)]
pub struct ComboReliability {
    pub pod_id: String,
    pub sim_type: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub success_rate: f64,
    pub avg_time_to_track_ms: Option<f64>,
    pub p95_time_to_track_ms: Option<f64>,
    pub total_launches: i64,
    pub common_failure_modes: Vec<FailureMode>,
    pub last_updated: String,
}

/// Local FailureMode — same shape as api::metrics::FailureMode, defined here to avoid
/// circular imports between metrics.rs and api::metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureMode {
    pub mode: String,
    pub count: i64,
}

/// Update the combo_reliability materialized table for a given (pod, sim, car, track) combo.
/// Computes rolling 30-day: success_rate, avg/p95 time_to_track, top 3 failure modes.
/// Called at the end of record_launch_event so every launch keeps scores current (INTEL-01).
pub async fn update_combo_reliability(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
) {
    // Count total launches in 30-day window for this combo
    let total_row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM launch_events
         WHERE pod_id = ? AND sim_type = ?
           AND (car = ? OR (? IS NULL AND car IS NULL))
           AND (track = ? OR (? IS NULL AND track IS NULL))
           AND created_at >= datetime('now', '-30 days')",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car).bind(car)
    .bind(track).bind(track)
    .fetch_one(db)
    .await
    .unwrap_or((0,));
    let total_launches = total_row.0;

    if total_launches == 0 {
        return;
    }

    // Count successes — outcome stored as JSON-serialized enum e.g. '"Success"'
    let success_row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM launch_events
         WHERE pod_id = ? AND sim_type = ?
           AND (car = ? OR (? IS NULL AND car IS NULL))
           AND (track = ? OR (? IS NULL AND track IS NULL))
           AND outcome = '\"Success\"'
           AND created_at >= datetime('now', '-30 days')",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car).bind(car)
    .bind(track).bind(track)
    .fetch_one(db)
    .await
    .unwrap_or((0,));
    let successes = success_row.0;
    let success_rate = if total_launches > 0 { successes as f64 / total_launches as f64 } else { 0.0 };

    // Compute avg time_to_track from successful launches
    let durations: Vec<(i64,)> = sqlx::query_as(
        "SELECT duration_to_playable_ms FROM launch_events
         WHERE pod_id = ? AND sim_type = ?
           AND (car = ? OR (? IS NULL AND car IS NULL))
           AND (track = ? OR (? IS NULL AND track IS NULL))
           AND outcome = '\"Success\"'
           AND duration_to_playable_ms IS NOT NULL
           AND created_at >= datetime('now', '-30 days')
         ORDER BY duration_to_playable_ms ASC",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car).bind(car)
    .bind(track).bind(track)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let avg_time = if !durations.is_empty() {
        let sum: f64 = durations.iter().map(|(d,)| *d as f64).sum();
        Some(sum / durations.len() as f64)
    } else {
        None
    };

    let p95_time = if !durations.is_empty() {
        // Already sorted ASC — p95 index
        let idx = ((durations.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        let idx = idx.min(durations.len() - 1);
        Some(durations[idx].0 as f64)
    } else {
        None
    };

    // Top 3 failure modes from error_taxonomy where outcome != Success
    let failure_modes: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(error_taxonomy, 'Unknown'), COUNT(*) as cnt
         FROM launch_events
         WHERE pod_id = ? AND sim_type = ?
           AND (car = ? OR (? IS NULL AND car IS NULL))
           AND (track = ? OR (? IS NULL AND track IS NULL))
           AND outcome != '\"Success\"'
           AND created_at >= datetime('now', '-30 days')
         GROUP BY error_taxonomy
         ORDER BY cnt DESC
         LIMIT 3",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car).bind(car)
    .bind(track).bind(track)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let failure_modes_vec: Vec<FailureMode> = failure_modes
        .into_iter()
        .map(|(mode, count)| FailureMode { mode, count })
        .collect();
    let failure_modes_json = serde_json::to_string(&failure_modes_vec).unwrap_or_default();

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    // Use a transaction to make DELETE+INSERT atomic — prevents a reader seeing
    // zero rows between the delete and insert.
    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("combo_reliability transaction begin failed for pod {}/{}: {}", pod_id, sim_type, e);
            return;
        }
    };

    // Delete existing row (if any) then insert fresh — handles NULL car/track correctly
    // since SQLite's UNIQUE INDEX on COALESCE(car,'') treats NULL as '' for conflict detection
    // but INSERT OR REPLACE needs a real PRIMARY KEY to replace on conflict.
    let delete_result = sqlx::query(
        "DELETE FROM combo_reliability
         WHERE pod_id = ? AND sim_type = ?
           AND (car = ? OR (? IS NULL AND car IS NULL))
           AND (track = ? OR (? IS NULL AND track IS NULL))",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car).bind(car)
    .bind(track).bind(track)
    .execute(&mut *tx)
    .await;

    if let Err(e) = delete_result {
        tracing::error!(
            "combo_reliability delete failed for pod {}/{}: {}",
            pod_id, sim_type, e
        );
        return;
    }

    let insert_result = sqlx::query(
        "INSERT INTO combo_reliability
            (pod_id, sim_type, car, track, success_rate, avg_time_to_track_ms, p95_time_to_track_ms, total_launches, common_failure_modes, last_updated)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(car)
    .bind(track)
    .bind(success_rate)
    .bind(avg_time)
    .bind(p95_time)
    .bind(total_launches)
    .bind(&failure_modes_json)
    .bind(&now)
    .execute(&mut *tx)
    .await;

    if let Err(e) = insert_result {
        tracing::error!(
            "combo_reliability insert failed for pod {}/{}: {}",
            pod_id, sim_type, e
        );
        return;
    }

    if let Err(e) = tx.commit().await {
        tracing::error!(
            "combo_reliability commit failed for pod {}/{}: {}",
            pod_id, sim_type, e
        );
    }
}

/// Query the combo_reliability table for a given (pod, sim, car, track) combo.
/// Returns None if total_launches < 5 (minimum sample threshold per INTEL-02).
/// Returns None if no record exists.
pub async fn query_combo_reliability(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
) -> Option<ComboReliability> {
    let row: Option<(f64, Option<f64>, Option<f64>, i64, Option<String>, String)> =
        sqlx::query_as(
            "SELECT success_rate, avg_time_to_track_ms, p95_time_to_track_ms, total_launches, common_failure_modes, last_updated
             FROM combo_reliability
             WHERE pod_id = ? AND sim_type = ?
               AND (car = ? OR (? IS NULL AND car IS NULL))
               AND (track = ? OR (? IS NULL AND track IS NULL))",
        )
        .bind(pod_id)
        .bind(sim_type)
        .bind(car).bind(car)
        .bind(track).bind(track)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    let (success_rate, avg_time, p95_time, total_launches, failure_modes_json, last_updated) =
        row?;

    // Minimum threshold — below 5 launches, return None (INTEL-02)
    if total_launches < 5 {
        return None;
    }

    let common_failure_modes: Vec<FailureMode> = failure_modes_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Some(ComboReliability {
        pod_id: pod_id.to_string(),
        sim_type: sim_type.to_string(),
        car: car.map(|s| s.to_string()),
        track: track.map(|s| s.to_string()),
        success_rate,
        avg_time_to_track_ms: avg_time,
        p95_time_to_track_ms: p95_time,
        total_launches,
        common_failure_modes,
        last_updated,
    })
}

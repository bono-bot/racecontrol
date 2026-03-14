//! Lap persistence and leaderboard tracking.
//!
//! When a pod agent reports a completed lap, this module:
//! 1. Resolves the driver from the active billing session
//! 2. Inserts the lap into the `laps` table
//! 3. Updates `personal_bests` if this is the driver's fastest lap for this track+car
//! 4. Updates `track_records` if this is the fastest lap ever for this track+car
//! 5. Updates driver aggregate stats (total_laps, total_time_ms)

use std::sync::Arc;

use rc_common::types::LapData;

use crate::state::AppState;

/// Resolve which driver is currently on a pod (from active billing session).
pub async fn resolve_driver_for_pod(state: &Arc<AppState>, pod_id: &str) -> Option<(String, String)> {
    let timers = state.billing.active_timers.read().await;
    timers.get(pod_id).map(|t| (t.driver_id.clone(), t.session_id.clone()))
}

/// Persist a completed lap to the database and update leaderboards.
/// Returns true if a new track record was set.
pub async fn persist_lap(state: &Arc<AppState>, lap: &LapData) -> bool {
    // Skip invalid laps or laps with 0 time
    if lap.lap_time_ms == 0 || !lap.valid {
        return false;
    }

    // Look up car_class from active billing session's kiosk_experience
    let car_class: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT ke.car_class
         FROM billing_sessions bs
         JOIN kiosk_experiences ke ON ke.id = bs.experience_id
         WHERE bs.driver_id = ? AND bs.status = 'active'
         LIMIT 1",
    )
    .bind(&lap.driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|(c,)| c);

    // Compute suspect flag before INSERT
    // A lap is suspect if:
    //   - lap_time_ms < 20_000 (impossibly fast, under 20 seconds)
    //   - sector times are all present and > 0 but their sum differs from lap_time_ms by > 500ms
    let sanity_ok = lap.lap_time_ms >= 20_000;
    let sector_sum_ok = match (lap.sector1_ms, lap.sector2_ms, lap.sector3_ms) {
        (Some(s1), Some(s2), Some(s3)) if s1 > 0 && s2 > 0 && s3 > 0 => {
            let sector_sum = s1 + s2 + s3;
            let diff = (sector_sum as i64 - lap.lap_time_ms as i64).unsigned_abs();
            diff <= 500
        }
        _ => true, // sectors absent or zero — treat as ok
    };
    let suspect_flag: i32 = if !sanity_ok || !sector_sum_ok { 1 } else { 0 };

    // 1. Insert lap into DB (with car_class from billing session lookup)
    let result = sqlx::query(
        "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, car_class, suspect, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&lap.id)
    .bind(&lap.session_id)
    .bind(&lap.driver_id)
    .bind(&lap.pod_id)
    .bind(format!("{:?}", lap.sim_type).to_lowercase())
    .bind(&lap.track)
    .bind(&lap.car)
    .bind(lap.lap_number as i64)
    .bind(lap.lap_time_ms as i64)
    .bind(lap.sector1_ms.map(|v| v as i64))
    .bind(lap.sector2_ms.map(|v| v as i64))
    .bind(lap.sector3_ms.map(|v| v as i64))
    .bind(lap.valid)
    .bind(&car_class)
    .bind(suspect_flag)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        tracing::error!("Failed to insert lap: {}", e);
        return false;
    }

    // 2. Check and update personal best for this driver+track+car
    let existing_pb = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
    )
    .bind(&lap.driver_id)
    .bind(&lap.track)
    .bind(&lap.car)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let is_pb = match existing_pb {
        Some((current_best,)) => (lap.lap_time_ms as i64) < current_best,
        None => true, // First lap on this track+car
    };

    if is_pb {
        let _ = sqlx::query(
            "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, lap_id, achieved_at)
             VALUES (?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT(driver_id, track, car) DO UPDATE SET
                best_lap_ms = excluded.best_lap_ms,
                lap_id = excluded.lap_id,
                achieved_at = excluded.achieved_at",
        )
        .bind(&lap.driver_id)
        .bind(&lap.track)
        .bind(&lap.car)
        .bind(lap.lap_time_ms as i64)
        .bind(&lap.id)
        .execute(&state.db)
        .await;

        tracing::info!(
            "New personal best for driver {} on {}/{}: {}ms",
            lap.driver_id, lap.track, lap.car, lap.lap_time_ms
        );
    }

    // 3. Check and update track record for this track+car
    let existing_record = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM track_records WHERE track = ? AND car = ?",
    )
    .bind(&lap.track)
    .bind(&lap.car)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let is_record = match existing_record {
        Some((current_record,)) => (lap.lap_time_ms as i64) < current_record,
        None => true, // First lap on this track+car
    };

    if is_record {
        let _ = sqlx::query(
            "INSERT INTO track_records (track, car, driver_id, best_lap_ms, lap_id, achieved_at)
             VALUES (?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT(track, car) DO UPDATE SET
                driver_id = excluded.driver_id,
                best_lap_ms = excluded.best_lap_ms,
                lap_id = excluded.lap_id,
                achieved_at = excluded.achieved_at",
        )
        .bind(&lap.track)
        .bind(&lap.car)
        .bind(&lap.driver_id)
        .bind(lap.lap_time_ms as i64)
        .bind(&lap.id)
        .execute(&state.db)
        .await;

        tracing::info!(
            "NEW TRACK RECORD on {}/{}: {}ms by driver {}",
            lap.track, lap.car, lap.lap_time_ms, lap.driver_id
        );
    }

    // 4. Update driver aggregate stats
    let _ = sqlx::query(
        "UPDATE drivers SET
            total_laps = COALESCE(total_laps, 0) + 1,
            total_time_ms = COALESCE(total_time_ms, 0) + ?,
            updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(lap.lap_time_ms as i64)
    .bind(&lap.driver_id)
    .execute(&state.db)
    .await;

    is_record
}

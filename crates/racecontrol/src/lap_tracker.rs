//! Lap persistence and leaderboard tracking.
//!
//! Persists laps, updates personal_bests/track_records, sends record-beaten emails.

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::LapData;
use sqlx::SqlitePool;

use crate::catalog;
use crate::psychology;
use crate::state::AppState;

#[path = "lap_tracker_events.rs"]
mod events;
#[path = "lap_tracker_notify.rs"]
mod notify;

pub use events::{
    auto_enter_event, recalculate_event_positions,
    f1_points_for_position, score_group_event,
    compute_championship_standings, assign_championship_positions,
};
pub use notify::get_previous_record_holder;
use notify::{compute_assist_evidence, send_gmail, format_lap_time};

#[cfg(test)]
#[path = "lap_tracker_tests.rs"]
mod tests;

/// UX-07: Mark laps as 'unverifiable' when the telemetry adapter crashes mid-session.
/// Called by the event_loop or ws_handler when it detects adapter disconnection.
/// Only laps that were previously 'valid' are updated — avoids overwriting 'invalid'/'suspect'.
pub async fn mark_laps_unverifiable(db: &SqlitePool, session_id: &str, from_lap: i32) {
    let result = sqlx::query(
        "UPDATE laps SET validity = 'unverifiable' WHERE session_id = ? AND lap_number >= ? AND validity = 'valid'",
    )
    .bind(session_id)
    .bind(from_lap)
    .execute(db)
    .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::warn!(
                "UX-07: Marked {} laps >= {} as unverifiable for session {} — telemetry adapter crashed",
                r.rows_affected(), from_lap, session_id
            );
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!(
                "UX-07: Failed to mark laps unverifiable for session {}: {}",
                session_id, e
            );
        }
    }
}

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

    // Compute sim_type_str once — used for DB storage, normalization, and all queries.
    // Format: format!("{:?}", SimType::X).to_lowercase() — e.g. "assettoCorsa", "f125", "iracing"
    let sim_type_str = format!("{:?}", lap.sim_type).to_lowercase();

    // Normalize the raw track name to the canonical Racing Point catalog ID.
    // AC tracks are already canonical (passthrough). Unknown game tracks pass through unchanged.
    let normalized_track = catalog::normalize_track_name(&sim_type_str, &lap.track);

    // Idempotent schema migration: add review_required and session_type columns if absent.
    // SQLite returns an error when a column already exists — silently ignore it.
    let _ = sqlx::query(
        "ALTER TABLE laps ADD COLUMN review_required INTEGER NOT NULL DEFAULT 0",
    )
    .execute(&state.db)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE laps ADD COLUMN session_type TEXT NOT NULL DEFAULT 'practice'",
    )
    .execute(&state.db)
    .await;

    // UX-04: Resolve billing_session_id from active timers.
    // Laps can only originate from billed sessions — no manual entry path.
    // The billing timer keyed by pod_id holds the authoritative session_id.
    let billing_session_id: Option<String> = {
        let timers = state.billing.active_timers.read().await;
        timers.get(&lap.pod_id).map(|t| t.session_id.clone())
    };

    // Phase 373-02: resolve group_session_id by finding the active ac_server
    // instance whose assigned_pods contains this lap's pod. Server-side only
    // — no protocol addition needed because each pod's billing session already
    // resolves to the correct single driver via resolve_driver_for_pod.
    // NULL for solo laps (no active AC MP instance contains the pod).
    let group_session_id: Option<String> = {
        use rc_common::types::AcServerStatus;
        let instances = state.ac_server.instances.read().await;
        instances.values()
            .filter(|inst| matches!(
                inst.status,
                AcServerStatus::Starting | AcServerStatus::Running
            ))
            .find(|inst| inst.assigned_pods.iter().any(|p| p == &lap.pod_id))
            .and_then(|inst| inst.group_session_id.clone())
    };

    // UX-04: Log when no billing session exists but still record the lap.
    // Previously this was a hard gate that rejected all laps without billing,
    // causing zero laps to be recorded for 43 days. Laps during free trials,
    // testing, or billing timing mismatches are now recorded with NULL
    // billing_session_id. The suspect flag (line ~209) already catches
    // unattributed laps (empty driver_id).
    if billing_session_id.is_none() {
        tracing::info!(
            pod = %lap.pod_id, driver = %lap.driver_id, lap_id = %lap.id,
            "UX-04: Lap has no active billing session — recording with NULL billing_session_id"
        );
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

    // UX-06: Compute assist evidence from billing session's kiosk experience.
    // Assist config JSON is stored on the experience; 'unknown' if unavailable.
    // Future: rc-agent telemetry will send per-lap assist state directly.
    let assist_config_json: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT ke.assist_config
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

    let (assist_config_hash, assist_tier) =
        compute_assist_evidence(assist_config_json.as_deref());

    // Compute suspect flag before INSERT
    // MMA-SEC: Lap time integrity validation — prevents fake leaderboard entries.
    // A lap is suspect if:
    //   - lap_time_ms < 20_000 (impossibly fast, under 20 seconds)
    //   - lap_time_ms > 600_000 (10 minutes — unreasonably slow, likely paused/glitched)
    //   - sector times present but their sum differs from lap_time_ms by > 500ms
    //   - driver_id is empty (no billing session matched)
    //   - pod_id doesn't match a known pod (1-8)
    let sanity_ok = lap.lap_time_ms >= 20_000 && lap.lap_time_ms <= 600_000;
    let driver_ok = !lap.driver_id.is_empty();
    let pod_ok = (1..=8).any(|n| lap.pod_id == format!("pod_{}", n) || lap.pod_id == format!("pod-{}", n));
    let sector_sum_ok = match (lap.sector1_ms, lap.sector2_ms, lap.sector3_ms) {
        (Some(s1), Some(s2), Some(s3)) if s1 > 0 && s2 > 0 && s3 > 0 => {
            let sector_sum = s1 + s2 + s3;
            let diff = (sector_sum as i64 - lap.lap_time_ms as i64).unsigned_abs();
            diff <= 500
        }
        _ => true, // sectors absent or zero -- treat as ok
    };
    let suspect_flag: i32 = if !sanity_ok || !sector_sum_ok || !driver_ok || !pod_ok { 1 } else { 0 };
    if suspect_flag == 1 {
        tracing::warn!(
            pod = %lap.pod_id, driver = %lap.driver_id, time_ms = lap.lap_time_ms,
            sanity_ok, sector_sum_ok, driver_ok, pod_ok,
            "Suspect lap flagged — will not appear on public leaderboard"
        );
    }

    // 1. Insert lap into DB (with car_class from billing session lookup)
    // Use a transaction to ensure lap INSERT + PB update + record update are atomic.
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("Failed to begin lap transaction: {}", e);
            return false;
        }
    };

    let result = sqlx::query(
        "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, car_class, suspect, session_type, assist_config_hash, assist_tier, billing_session_id, group_session_id, validity, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'valid', datetime('now'), ?)",
    )
    .bind(&lap.id)
    .bind(&lap.session_id)
    .bind(&lap.driver_id)
    .bind(&lap.pod_id)
    .bind(&sim_type_str)
    .bind(&normalized_track)
    .bind(&lap.car)
    .bind(lap.lap_number as i64)
    .bind(lap.lap_time_ms as i64)
    .bind(lap.sector1_ms.map(|v| v as i64))
    .bind(lap.sector2_ms.map(|v| v as i64))
    .bind(lap.sector3_ms.map(|v| v as i64))
    .bind(lap.valid)
    .bind(&car_class)
    .bind(suspect_flag)
    .bind(format!("{:?}", lap.session_type).to_lowercase())
    .bind(&assist_config_hash)
    .bind(&assist_tier)
    .bind(&billing_session_id)
    .bind(&group_session_id)
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = result {
        tracing::error!("Failed to insert lap: {}", e);
        let _ = tx.rollback().await;
        return false;
    }

    // LAP-02: check per-track minimum lap time floor — flag suspicious fast laps for staff review
    if let Some(min_ms) = catalog::get_min_lap_time_ms_for_track(&normalized_track)
        && lap.lap_time_ms < min_ms {
            let _ = sqlx::query("UPDATE laps SET review_required = 1 WHERE id = ?")
                .bind(&lap.id)
                .execute(&mut *tx)
                .await;
            tracing::info!(
                "[lap-filter] LAP-02 review_required: lap {} on {} is {}ms < floor {}ms",
                lap.id, normalized_track, lap.lap_time_ms, min_ms
            );
        }

    // 2. Check and update personal best for this driver+track+car+sim_type
    // PBs are scoped by sim_type — an F1 25 PB on monza is separate from an AC PB.
    let existing_pb = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ? AND sim_type = ?",
    )
    .bind(&lap.driver_id)
    .bind(&normalized_track)
    .bind(&lap.car)
    .bind(&sim_type_str)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    let is_pb = match existing_pb {
        Some((current_best,)) => (lap.lap_time_ms as i64) < current_best,
        None => true, // First lap on this track+car+sim_type
    };

    if is_pb {
        let _ = sqlx::query(
            "INSERT INTO personal_bests (driver_id, track, car, sim_type, best_lap_ms, lap_id, achieved_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(driver_id, track, car, sim_type) DO UPDATE SET
                best_lap_ms = excluded.best_lap_ms,
                lap_id = excluded.lap_id,
                achieved_at = excluded.achieved_at",
        )
        .bind(&lap.driver_id)
        .bind(&normalized_track)
        .bind(&lap.car)
        .bind(&sim_type_str)
        .bind(lap.lap_time_ms as i64)
        .bind(&lap.id)
        .bind(&state.config.venue.venue_id)
        .execute(&mut *tx)
        .await;

        tracing::info!(
            "New personal best for driver {} on {}/{} ({}): {}ms",
            lap.driver_id, normalized_track, lap.car, sim_type_str, lap.lap_time_ms
        );

        // Broadcast PB event for real-time PWA notification
        let _ = state.dashboard_tx.send(DashboardEvent::PbAchieved {
            driver_id: lap.driver_id.clone(),
            session_id: lap.session_id.clone(),
            track: normalized_track.clone(),
            car: lap.car.clone(),
            lap_time_ms: lap.lap_time_ms as i64,
            lap_id: lap.id.clone(),
        });

        // Phase 254: Broadcast PB broken for real-time leaderboard updates
        {
            let pb_driver_name: String = sqlx::query_as::<_, (String,)>(
                "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL
                            THEN nickname ELSE name END
                 FROM drivers WHERE id = ?",
            )
            .bind(&lap.driver_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|(n,)| n)
            .unwrap_or_else(|| "Unknown".to_string());

            let _ = state.dashboard_tx.send(DashboardEvent::RecordBroken {
                record_type: "personal_best".to_string(),
                track: normalized_track.clone(),
                car: lap.car.clone(),
                sim_type: sim_type_str.clone(),
                driver_name: pb_driver_name,
                lap_time_ms: lap.lap_time_ms as i64,
                previous_time_ms: existing_pb.map(|(t,)| t),
                driver_id: lap.driver_id.clone(),
            });
        }

        // Retention hooks: notify beaten PB holders + chance of surprise reward
        let state_clone = state.clone();
        let driver_id_clone = lap.driver_id.clone();
        let track_clone = lap.track.clone();
        let car_clone = lap.car.clone();
        let lap_time_clone = lap.lap_time_ms as i64;
        tokio::spawn(async move {
            crate::psychology::notify_pb_beaten_holders(
                &state_clone, &driver_id_clone, &track_clone, &car_clone, lap_time_clone
            ).await;
            crate::psychology::maybe_grant_variable_reward(
                &state_clone, &driver_id_clone, "pb"
            ).await;
        });
    }

    // 3. Check and update track record for this track+car+sim_type
    // Track records are scoped by sim_type — an F1 25 record is separate from AC.
    // STEP 1: Fetch previous record holder (name + email) BEFORE the UPSERT.
    // If fetched after, the UPSERT would have overwritten it with the new holder.
    let prev_record = get_previous_record_holder(&state.db, &normalized_track, &lap.car, &sim_type_str).await;

    let is_record = match &prev_record {
        Some((current_record, _, _)) => (lap.lap_time_ms as i64) < *current_record,
        None => true, // First lap on this track+car -- new record, but no one to notify
    };

    if is_record {
        // STEP 2: Fetch the new record holder's display name (nickname if opted in, else name).
        let new_holder_name: String = sqlx::query_as::<_, (String,)>(
            "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL
                        THEN nickname ELSE name END
             FROM drivers WHERE id = ?",
        )
        .bind(&lap.driver_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten()
        .map(|(n,)| n)
        .unwrap_or_else(|| "Unknown".to_string());

        // STEP 3: Execute the UPSERT (sim_type-scoped).
        let _ = sqlx::query(
            "INSERT INTO track_records (track, car, sim_type, driver_id, best_lap_ms, lap_id, achieved_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(track, car, sim_type) DO UPDATE SET
                driver_id = excluded.driver_id,
                best_lap_ms = excluded.best_lap_ms,
                lap_id = excluded.lap_id,
                achieved_at = excluded.achieved_at",
        )
        .bind(&normalized_track)
        .bind(&lap.car)
        .bind(&sim_type_str)
        .bind(&lap.driver_id)
        .bind(lap.lap_time_ms as i64)
        .bind(&lap.id)
        .bind(&state.config.venue.venue_id)
        .execute(&mut *tx)
        .await;

        tracing::info!(
            "NEW TRACK RECORD on {}/{} ({}): {}ms by driver {}",
            normalized_track, lap.car, sim_type_str, lap.lap_time_ms, lap.driver_id
        );

        // Phase 254: Broadcast track record broken for real-time leaderboard updates
        let _ = state.dashboard_tx.send(DashboardEvent::RecordBroken {
            record_type: "track_record".to_string(),
            track: normalized_track.clone(),
            car: lap.car.clone(),
            sim_type: sim_type_str.clone(),
            driver_name: new_holder_name.clone(),
            lap_time_ms: lap.lap_time_ms as i64,
            previous_time_ms: prev_record.as_ref().map(|(t, _, _)| *t),
            driver_id: lap.driver_id.clone(),
        });

        // STEP 4: Fire notification email to the previous record holder (if any).
        if let Some((old_time_ms, prev_name, Some(prev_email))) = prev_record {
            let track = normalized_track.clone();
            let car = lap.car.clone();
            let new_time_ms = lap.lap_time_ms as i64;
            let new_holder = new_holder_name.clone();
            let http = state.http_client.clone();
            let gmail = state.config.gmail.clone();

            // Format times as M:SS.mmm
            let old_display = format_lap_time(old_time_ms);
            let new_display = format_lap_time(new_time_ms);

            let subject = format!("Your {} record at {} has been beaten!", car, track);
            let body = format!(
                "Hi {},\n\n\
                 Your track record at {} in the {} has been broken.\n\n\
                 New record set by: {}\n\
                 Old time: {}\n\
                 New time: {}\n\n\
                 Come back and reclaim it!\n\n\
                 https://app.racingpoint.cloud/leaderboard/public",
                prev_name, track, car, new_holder, old_display, new_display
            );

            // Fire-and-forget: notification failure must not affect lap persistence
            tokio::spawn(async move {
                if let Err(e) = send_gmail(&http, &gmail, &prev_email, &subject, &body).await {
                    tracing::warn!(
                        "Track record notification failed for {}/{}: {}",
                        track, car, e
                    );
                } else {
                    tracing::info!(
                        "Track record notification sent to {} for {}/{}",
                        prev_email, track, car
                    );
                }
            });
        } else if prev_record.is_some() {
            // Previous holder exists but has no email -- skip silently
            tracing::debug!(
                "Previous record holder on {}/{} has no email, skipping notification",
                normalized_track, lap.car
            );
        }
        // If prev_record is None, this is the first record -- no one to notify
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
    .execute(&mut *tx)
    .await;

    // Commit the transaction (lap + PB + record + stats are now atomic)
    if let Err(e) = tx.commit().await {
        tracing::error!("Failed to commit lap transaction: {}", e);
        return false;
    }

    // Phase 253: Send rating recomputation request (non-blocking)
    if let Some(ref rating_tx) = state.rating_tx {
        let _ = rating_tx.try_send(crate::driver_rating::RatingRequest {
            driver_id: lap.driver_id.clone(),
            sim_type: sim_type_str.clone(),
        });
    }

    // Update driving passport with this track+car combo
    psychology::update_driving_passport(state, &lap.driver_id, &normalized_track, &lap.car, lap.lap_time_ms as i64).await;

    // Phase 14: Auto-enter into matching hotlap events
    if suspect_flag == 0
        && let Some(ref class) = car_class {
            auto_enter_event(
                &state.db,
                Some(lap.id.as_str()),
                &lap.driver_id,
                &normalized_track,
                class,
                &sim_type_str,
                lap.lap_time_ms,
                lap.sector1_ms,
                lap.sector2_ms,
                lap.sector3_ms,
                &state.config.venue.venue_id,
            )
            .await;
        }

    is_record
}

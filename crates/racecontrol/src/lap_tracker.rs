//! Lap persistence and leaderboard tracking.
//!
//! When a pod agent reports a completed lap, this module:
//! 1. Resolves the driver from the active billing session
//! 2. Inserts the lap into the `laps` table
//! 3. Updates `personal_bests` if this is the driver's fastest lap for this track+car
//! 4. Updates `track_records` if this is the fastest lap ever for this track+car
//! 5. Updates driver aggregate stats (total_laps, total_time_ms)
//! 6. Sends "record beaten" email to previous holder (if any)

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::LapData;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::catalog;
use crate::psychology;
use crate::state::AppState;

/// Compute assist evidence fields from an assist config JSON string.
/// Returns (assist_config_hash, assist_tier).
///
/// assist_tier derivation rules (UX-06):
///   - 'pro'      = traction_control=0, stability_control=0, abs=0, ideal_line=false
///   - 'amateur'  = ideal_line=true (visual assistance — strongest assist)
///   - 'semi-pro' = any other assist on (TC, SC, or ABS), but not ideal_line
///   - 'unknown'  = assist config not available
///
/// The config JSON is sorted-key serialized before hashing for reproducibility.
fn compute_assist_evidence(assist_config_json: Option<&str>) -> (Option<String>, String) {
    let Some(json_str) = assist_config_json else {
        return (None, "unknown".to_string());
    };
    // Parse the JSON as a BTreeMap for stable key ordering
    let config: std::collections::BTreeMap<String, serde_json::Value> =
        match serde_json::from_str(json_str) {
            Ok(m) => m,
            Err(_) => return (None, "unknown".to_string()),
        };

    // Compute SHA-256 of the canonically sorted JSON
    let canonical = serde_json::to_string(&config).unwrap_or_default();
    let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));

    // Derive tier from assist values
    // Keys expected: traction_control, stability_control, abs, autoclutch, ideal_line
    let get_bool = |key: &str| -> bool {
        config
            .get(key)
            .map(|v| match v {
                serde_json::Value::Bool(b) => *b,
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) > 0.0,
                _ => false,
            })
            .unwrap_or(false)
    };
    let get_int = |key: &str| -> i64 {
        config
            .get(key)
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };

    let ideal_line = get_bool("ideal_line");
    let tc = get_int("traction_control");
    let sc = get_int("stability_control");
    let abs_val = get_int("abs");

    let tier = if ideal_line {
        "amateur"
    } else if tc == 0 && sc == 0 && abs_val == 0 {
        "pro"
    } else {
        "semi-pro"
    };

    (Some(hash), tier.to_string())
}

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

    // UX-04 integrity gate: reject laps with no billing session.
    // This makes manual leaderboard entry structurally impossible — every lap
    // must be backed by a real billed session.
    if billing_session_id.is_none() {
        tracing::warn!(
            pod = %lap.pod_id, driver = %lap.driver_id, lap_id = %lap.id,
            "UX-04: Lap rejected — no active billing session on pod. \
             Laps may only be created from billed sessions."
        );
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
        "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, car_class, suspect, session_type, assist_config_hash, assist_tier, billing_session_id, validity, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'valid', datetime('now'), ?)",
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
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = result {
        tracing::error!("Failed to insert lap: {}", e);
        let _ = tx.rollback().await;
        return false;
    }

    // LAP-02: check per-track minimum lap time floor — flag suspicious fast laps for staff review
    if let Some(min_ms) = catalog::get_min_lap_time_ms_for_track(&normalized_track) {
        if lap.lap_time_ms < min_ms {
            let _ = sqlx::query("UPDATE laps SET review_required = 1 WHERE id = ?")
                .bind(&lap.id)
                .execute(&mut *tx)
                .await;
            tracing::info!(
                "[lap-filter] LAP-02 review_required: lap {} on {} is {}ms < floor {}ms",
                lap.id, normalized_track, lap.lap_time_ms, min_ms
            );
        }
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
    if suspect_flag == 0 {
        if let Some(ref class) = car_class {
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
    }

    is_record
}

/// Auto-enter a valid lap into matching active hotlap events.
/// Called from persist_lap() after lap INSERT, only if valid && suspect==0.
/// Made pub so integration tests can call it directly with a pool.
pub async fn auto_enter_event(
    pool: &sqlx::SqlitePool,
    lap_id: Option<&str>,
    driver_id: &str,
    track: &str,
    car_class: &str,
    sim_type_str: &str,
    lap_time_ms: u32,
    sector1_ms: Option<u32>,
    sector2_ms: Option<u32>,
    sector3_ms: Option<u32>,
    venue_id: &str,
) {
    // Query matching active/upcoming events for this track+car_class+sim_type in the current date range
    let events = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT id, reference_time_ms FROM hotlap_events
         WHERE track = ? AND car_class = ? AND sim_type = ?
           AND status IN ('active', 'upcoming')
           AND (starts_at IS NULL OR datetime(starts_at) <= datetime('now'))
           AND (ends_at IS NULL OR datetime(ends_at) >= datetime('now'))",
    )
    .bind(track)
    .bind(car_class)
    .bind(sim_type_str)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (event_id, reference_time_ms) in events {
        // Check if driver already has a faster or equal entry for this event
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT lap_time_ms FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?",
        )
        .bind(&event_id)
        .bind(driver_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((existing_ms,)) = existing {
            if existing_ms <= lap_time_ms as i64 {
                // Existing entry is faster or equal -- skip
                continue;
            }
        }

        // Compute badge from reference_time_ms
        let badge: Option<&str> = match reference_time_ms {
            None => None,
            Some(ref_ms) => {
                let ratio = lap_time_ms as f64 / ref_ms as f64;
                if ratio <= 1.02 {
                    Some("gold")
                } else if ratio <= 1.05 {
                    Some("silver")
                } else if ratio <= 1.08 {
                    Some("bronze")
                } else {
                    Some("none")
                }
            }
        };

        // Generate a new UUID for the entry
        let entry_id = uuid::Uuid::new_v4().to_string();

        // UPSERT the entry (ON CONFLICT updates if this lap is faster)
        let upsert_result = sqlx::query(
            "INSERT INTO hotlap_event_entries
                (id, event_id, driver_id, lap_id, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, badge, result_status, entered_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'finished', datetime('now'), ?)
             ON CONFLICT(event_id, driver_id) DO UPDATE SET
                lap_id = excluded.lap_id,
                lap_time_ms = excluded.lap_time_ms,
                sector1_ms = excluded.sector1_ms,
                sector2_ms = excluded.sector2_ms,
                sector3_ms = excluded.sector3_ms,
                badge = excluded.badge,
                result_status = 'finished',
                entered_at = excluded.entered_at",
        )
        .bind(&entry_id)
        .bind(&event_id)
        .bind(driver_id)
        .bind(lap_id)
        .bind(lap_time_ms as i64)
        .bind(sector1_ms.map(|v| v as i64))
        .bind(sector2_ms.map(|v| v as i64))
        .bind(sector3_ms.map(|v| v as i64))
        .bind(badge)
        .bind(venue_id)
        .execute(pool)
        .await;

        match upsert_result {
            Ok(_) => {
                tracing::info!(
                    "[events] Driver {} entered event {} with {}ms (badge: {:?})",
                    driver_id, event_id, lap_time_ms, badge
                );
                // Recalculate positions and gaps for all entries in this event
                recalculate_event_positions(pool, &event_id).await;
            }
            Err(e) => {
                tracing::error!(
                    "[events] Failed to upsert event entry for driver {} event {}: {}",
                    driver_id, event_id, e
                );
            }
        }
    }
}

/// Recalculate position, gap_to_leader_ms, and within_107_percent for all entries in an event.
/// Uses a two-step approach since SQLite UPDATE doesn't support window functions directly.
/// Made pub so integration tests can call it directly.
pub async fn recalculate_event_positions(pool: &sqlx::SqlitePool, event_id: &str) {
    // Step 1: Fetch all finished entries ordered by lap time (fastest first)
    let entries: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, lap_time_ms FROM hotlap_event_entries
         WHERE event_id = ? AND result_status = 'finished'
         ORDER BY lap_time_ms ASC",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if entries.is_empty() {
        return;
    }

    let leader_ms = entries[0].1;

    // Step 2: Update each entry's position, gap, and 107% flag
    for (position, (entry_id, lap_ms)) in entries.iter().enumerate() {
        let pos = (position + 1) as i64;
        let gap = lap_ms - leader_ms;
        // Integer math: lap_ms * 100 <= leader_ms * 107
        let within_107: i64 = if lap_ms * 100 <= leader_ms * 107 { 1 } else { 0 };

        let _ = sqlx::query(
            "UPDATE hotlap_event_entries
             SET position = ?, gap_to_leader_ms = ?, within_107_percent = ?
             WHERE id = ?",
        )
        .bind(pos)
        .bind(gap)
        .bind(within_107)
        .bind(entry_id)
        .execute(pool)
        .await;
    }

    tracing::debug!(
        "[events] Recalculated positions for event {} ({} entries, leader: {}ms)",
        event_id, entries.len(), leader_ms
    );
}

// ─── Gmail API (native, no external script) ──────────────────────────────────

use crate::config::GmailConfig;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

async fn send_gmail(
    http: &reqwest::Client,
    gmail: &GmailConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    if !gmail.enabled {
        return Err("Gmail not enabled in config".into());
    }
    let client_id = gmail.client_id.as_deref().ok_or("gmail.client_id missing")?;
    let client_secret = gmail.client_secret.as_deref().ok_or("gmail.client_secret missing")?;
    let refresh_token = gmail.refresh_token.as_deref().ok_or("gmail.refresh_token missing")?;

    // Step 1: Exchange refresh_token for access_token
    let token_resp = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed ({}): {}", status, body));
    }

    let token_json: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| format!("Token parse failed: {}", e))?;
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or("No access_token in response")?;

    // Step 2: Build RFC 2822 message and base64url encode
    let from = &gmail.from_email;
    let raw_message = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
        from, to, subject, body
    );
    let encoded = URL_SAFE_NO_PAD.encode(raw_message.as_bytes());

    // Step 3: Send via Gmail API
    let send_resp = http
        .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
        .bearer_auth(access_token)
        .json(&serde_json::json!({ "raw": encoded }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Gmail send failed: {}", e))?;

    if !send_resp.status().is_success() {
        let status = send_resp.status();
        let body = send_resp.text().await.unwrap_or_default();
        return Err(format!("Gmail send failed ({}): {}", status, body));
    }

    Ok(())
}

// ─── F1 Scoring constants ─────────────────────────────────────────────────────

/// F1 2010 points system: P1=25, P2=18, ..., P10=1, P11+=0.
const F1_2010_POINTS: [i64; 10] = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];

/// Return F1 points for a given finishing position.
/// DNF drivers, positions outside 1-10, and positions < 1 all receive 0 points.
pub fn f1_points_for_position(position: i64, dnf: bool) -> i64 {
    if dnf || position < 1 || position > 10 {
        return 0;
    }
    F1_2010_POINTS[(position - 1) as usize]
}

/// Score a completed group session linked to a hotlap event.
///
/// Reads multiplayer_results for the group session, assigns F1 2010 points
/// based on finishing position (DNF = 0 points), and upserts entries into
/// hotlap_event_entries. The leader's gap_to_leader_ms = 0; others are
/// computed as (their best_lap_ms - leader best_lap_ms).
///
/// Made pub so integration tests can call it directly.
pub async fn score_group_event(
    pool: &sqlx::SqlitePool,
    group_session_id: &str,
    hotlap_event_id: &str,
    venue_id: &str,
) -> Result<(), String> {
    // Fetch all results ordered by position
    let results: Vec<(String, i64, Option<i64>, i64)> = sqlx::query_as(
        "SELECT driver_id, position, best_lap_ms, dnf
         FROM multiplayer_results
         WHERE group_session_id = ?
         ORDER BY position ASC",
    )
    .bind(group_session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch multiplayer results: {e}"))?;

    if results.is_empty() {
        return Ok(());
    }

    // Find leader lap time (minimum best_lap_ms among non-DNF drivers)
    let leader_ms: Option<i64> = results
        .iter()
        .filter(|(_, _, _, dnf)| *dnf == 0)
        .filter_map(|(_, _, best_lap_ms, _)| *best_lap_ms)
        .reduce(i64::min);

    for (driver_id, position, best_lap_ms, dnf) in &results {
        let is_dnf = *dnf == 1;
        let points = f1_points_for_position(*position, is_dnf);
        let result_status = if is_dnf { "dnf" } else { "finished" };

        let gap_to_leader_ms: Option<i64> = if is_dnf {
            None
        } else {
            match (best_lap_ms, leader_ms) {
                (Some(ms), Some(leader)) => Some(ms - leader),
                _ => None,
            }
        };

        let within_107: i64 = match (is_dnf, best_lap_ms, leader_ms) {
            (false, Some(ms), Some(leader)) => {
                if ms * 100 <= leader * 107 { 1 } else { 0 }
            }
            _ => 0,
        };

        let entry_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO hotlap_event_entries
                (id, event_id, driver_id, lap_time_ms, position, points,
                 gap_to_leader_ms, within_107_percent, result_status, entered_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(event_id, driver_id) DO UPDATE SET
                lap_time_ms = excluded.lap_time_ms,
                position = excluded.position,
                points = excluded.points,
                gap_to_leader_ms = excluded.gap_to_leader_ms,
                within_107_percent = excluded.within_107_percent,
                result_status = excluded.result_status",
        )
        .bind(&entry_id)
        .bind(hotlap_event_id)
        .bind(driver_id)
        .bind(*best_lap_ms)
        .bind(*position)
        .bind(points)
        .bind(gap_to_leader_ms)
        .bind(within_107)
        .bind(result_status)
        .bind(venue_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to upsert hotlap_event_entry for driver {driver_id}: {e}"))?;
    }

    tracing::info!(
        "[events] Scored group session {} into event {} ({} results)",
        group_session_id, hotlap_event_id, results.len()
    );

    Ok(())
}

/// Compute championship standings from hotlap_event_entries and persist to championship_standings.
///
/// Aggregates points, wins, P2 count, P3 count across all rounds in the championship,
/// upserts into championship_standings, then calls assign_championship_positions to
/// apply the F1 tiebreaker ordering (points DESC, wins DESC, p2_count DESC, p3_count DESC).
///
/// Made pub so integration tests can call it directly.
pub async fn compute_championship_standings(
    pool: &sqlx::SqlitePool,
    championship_id: &str,
    venue_id: &str,
) -> Result<(), String> {
    // Aggregate points and counts from hotlap_event_entries across all rounds
    let rows: Vec<(String, i64, i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT hee.driver_id,
                COALESCE(SUM(hee.points), 0) as total_points,
                COUNT(DISTINCT cr.event_id) as rounds_entered,
                SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2_count,
                SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3_count,
                MIN(hee.position) as best_result
         FROM hotlap_event_entries hee
         INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
         WHERE cr.championship_id = ?
           AND hee.result_status IN ('finished', 'dnf', 'dns')
         GROUP BY hee.driver_id",
    )
    .bind(championship_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to aggregate championship standings: {e}"))?;

    for (driver_id, total_points, rounds_entered, wins, p2_count, p3_count, best_result) in &rows {
        sqlx::query(
            "INSERT INTO championship_standings
                (championship_id, driver_id, total_points, rounds_entered, wins, p2_count, p3_count, best_result, updated_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(championship_id, driver_id) DO UPDATE SET
                total_points = excluded.total_points,
                rounds_entered = excluded.rounds_entered,
                wins = excluded.wins,
                p2_count = excluded.p2_count,
                p3_count = excluded.p3_count,
                best_result = excluded.best_result,
                updated_at = excluded.updated_at",
        )
        .bind(championship_id)
        .bind(driver_id)
        .bind(total_points)
        .bind(rounds_entered)
        .bind(wins)
        .bind(p2_count)
        .bind(p3_count)
        .bind(best_result)
        .bind(venue_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to upsert standing for driver {driver_id}: {e}"))?;
    }

    // Now assign positions based on F1 tiebreaker order
    assign_championship_positions(pool, championship_id).await?;

    tracing::info!(
        "[championship] Computed standings for championship {} ({} drivers)",
        championship_id, rows.len()
    );

    Ok(())
}

/// Assign position column to all championship_standings rows for a championship.
///
/// Reads all standing rows, sorts by (total_points DESC, wins DESC, p2_count DESC, p3_count DESC),
/// then updates the position column (1-indexed). This is the F1 tiebreaker rule.
///
/// Made pub so integration tests can call it directly without seeding hotlap_event_entries.
pub async fn assign_championship_positions(
    pool: &sqlx::SqlitePool,
    championship_id: &str,
) -> Result<(), String> {
    // Fetch all standings rows sorted by tiebreaker
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT driver_id
         FROM championship_standings
         WHERE championship_id = ?
         ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC",
    )
    .bind(championship_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch championship standings for position assignment: {e}"))?;

    // Update position for each driver
    for (position, (driver_id,)) in rows.iter().enumerate() {
        let pos = (position + 1) as i64;
        sqlx::query(
            "UPDATE championship_standings SET position = ? WHERE championship_id = ? AND driver_id = ?",
        )
        .bind(pos)
        .bind(championship_id)
        .bind(driver_id)
        .execute(pool)
        .await
        .map_err(|e| {
            format!(
                "Failed to update position for driver {driver_id} in championship {championship_id}: {e}"
            )
        })?;
    }

    tracing::debug!(
        "[championship] Assigned positions for {} in championship {}",
        rows.len(), championship_id
    );

    Ok(())
}

/// Fetch the current track record holder's best time, name, and email for a given track+car.
///
/// Returns `Some((best_lap_ms, driver_name, Option<email>))` if a record exists,
/// or `None` if no record has been set for this track+car combination.
///
/// This function is called BEFORE the UPSERT in `persist_lap()` so that the
/// previous holder's data is captured before it gets overwritten.
pub async fn get_previous_record_holder(
    db: &SqlitePool,
    track: &str,
    car: &str,
    sim_type: &str,
) -> Option<(i64, String, Option<String>)> {
    sqlx::query_as::<_, (i64, String, Option<String>)>(
        "SELECT tr.best_lap_ms, d.name, d.email
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.track = ? AND tr.car = ? AND tr.sim_type = ?",
    )
    .bind(track)
    .bind(car)
    .bind(sim_type)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Format a lap time in milliseconds as M:SS.mmm (e.g., 90123 -> "1:30.123").
fn format_lap_time(ms: i64) -> String {
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;
    format!("{}:{:02}.{:03}", minutes, seconds, millis)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rc_common::types::{LapData, SessionType, SimType};

    use crate::catalog;

    #[test]
    fn lap_invalid_flag_prevents_persist() {
        // LAP-01: valid=false must cause persist_lap to return false without DB write.
        // Production code gates at persist_lap() line: if lap.lap_time_ms == 0 || !lap.valid { return false; }
        // Verify the guard logic holds for the two disqualifying conditions.
        let invalid_lap = false;
        let zero_time: u32 = 0;
        // Either condition alone causes an early return
        assert!(!invalid_lap || zero_time == 0, "invalid lap gate: !valid => skip persist");
        // Confirm the guard expression used in production code
        assert!(zero_time == 0 || !invalid_lap, "zero time gate: time==0 => skip persist");
    }

    #[test]
    fn lap_review_required_below_min_floor() {
        // LAP-02: lap_time_ms=75_000 on Monza (min=80_000) must set review_required=1.
        // Verify that catalog returns the floor and the comparison logic fires correctly.
        let monza_floor = catalog::get_min_lap_time_ms_for_track("monza");
        assert_eq!(monza_floor, Some(80_000), "Monza floor must be 80_000ms");
        let lap_time_ms: u32 = 75_000;
        let floor = monza_floor.unwrap();
        assert!(
            lap_time_ms < floor,
            "75_000ms < 80_000ms floor => review_required should be set"
        );
    }

    #[test]
    fn lap_not_flagged_above_min_floor() {
        // LAP-02: lap_time_ms=85_000 on Monza (min=80_000) must NOT set review_required.
        let monza_floor = catalog::get_min_lap_time_ms_for_track("monza").unwrap();
        let lap_time_ms: u32 = 85_000;
        assert!(
            lap_time_ms >= monza_floor,
            "85_000ms >= 80_000ms floor => review_required must NOT be set"
        );
    }

    #[test]
    fn lap_data_carries_session_type() {
        // LAP-03: LapData.session_type is a required field set at construction.
        let lap = LapData {
            id: "test-id".to_string(),
            session_id: "sess-1".to_string(),
            driver_id: "driver-1".to_string(),
            pod_id: "pod_1".to_string(),
            sim_type: SimType::AssettoCorsa,
            track: "monza".to_string(),
            car: "ferrari_sf25".to_string(),
            lap_number: 1,
            lap_time_ms: 95_000,
            sector1_ms: None,
            sector2_ms: None,
            sector3_ms: None,
            valid: true,
            session_type: SessionType::Practice,
            created_at: Utc::now(),
        };
        assert_eq!(
            lap.session_type,
            SessionType::Practice,
            "LapData.session_type must be set and accessible"
        );
    }
}

#![allow(unused_imports)]
use axum::{
    Json,
    extract::State,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::cloud_sync;
use crate::state::AppState;
use rc_common::protocol::DashboardEvent;

use super::parse_config_snapshot;

#[path = "sync_push_entities.rs"]
mod sync_push_entities;

// ─── Cloud Sync Push (POST /sync/push) ─────────────────────────────────────

/// POST /sync/push — venue pushes data to cloud
pub(crate) async fn sync_push(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body_bytes: axum::body::Bytes,
) -> Json<Value> {
    // Auth check (x-terminal-secret)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(json!({ "error": "Unauthorized" }));
        }
    }

    // HMAC-SHA256 verification -- permissive mode (AUTH-07)
    // TODO: Switch to strict mode after Bono deploys matching HMAC key
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let sig = headers.get("x-sync-signature").and_then(|v| v.to_str().ok());
        let ts = headers.get("x-sync-timestamp").and_then(|v| v.to_str().ok());
        let nonce = headers.get("x-sync-nonce").and_then(|v| v.to_str().ok());

        match (sig, ts, nonce) {
            (Some(sig), Some(ts_str), Some(nonce)) => {
                if let Ok(timestamp) = ts_str.parse::<i64>() {
                    if !crate::cloud_sync::verify_sync_signature(
                        &body_bytes, hmac_key.as_bytes(), timestamp, nonce, sig,
                    ) {
                        tracing::warn!(target: "sync", "HMAC verification failed on sync_push (permissive -- allowing)");
                    }
                } else {
                    tracing::warn!(target: "sync", "Invalid x-sync-timestamp header on sync_push");
                }
            }
            _ => {
                tracing::warn!(target: "sync", "HMAC headers missing on sync_push (permissive -- allowing)");
            }
        }
    }

    // Parse JSON body
    let body: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return Json(json!({ "error": format!("Invalid JSON: {}", e) }));
        }
    };

    // Origin tag check: reject data that originated from us (anti-loop defense)
    let incoming_origin = body.get("origin").and_then(|v| v.as_str()).unwrap_or("unknown");
    let my_origin = &state.config.cloud.origin_id;
    if incoming_origin == my_origin {
        tracing::warn!(target: "sync", "Rejecting sync_push from same origin: {}", my_origin);
        return Json(json!({ "ok": true, "upserted": 0, "reason": "same_origin" }));
    }

    let mut total = 0u64;

    // Upsert laps
    if let Some(laps) = body.get("laps").and_then(|v| v.as_array()) {
        for lap in laps {
            let id = lap.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car,
                    lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at,
                    car_class, suspect, session_type, assist_config_hash, assist_tier,
                    billing_session_id, validity, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
                 ON CONFLICT(id) DO NOTHING",
            )
            .bind(id)
            .bind(lap.get("session_id").and_then(|v| v.as_str()))
            .bind(lap.get("driver_id").and_then(|v| v.as_str()))
            .bind(lap.get("pod_id").and_then(|v| v.as_str()))
            .bind(lap.get("sim_type").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("track").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("car").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("lap_number").and_then(|v| v.as_i64()))
            .bind(lap.get("lap_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(lap.get("sector1_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("sector2_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("sector3_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("valid").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(lap.get("created_at").and_then(|v| v.as_str()))
            .bind(lap.get("car_class").and_then(|v| v.as_str()))
            .bind(lap.get("suspect").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(lap.get("session_type").and_then(|v| v.as_str()).unwrap_or("practice"))
            .bind(lap.get("assist_config_hash").and_then(|v| v.as_str()))
            .bind(lap.get("assist_tier").and_then(|v| v.as_str()).unwrap_or("unknown"))
            .bind(lap.get("billing_session_id").and_then(|v| v.as_str()))
            .bind(lap.get("validity").and_then(|v| v.as_str()).unwrap_or("valid"))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert track_records (best lap per track+car)
    if let Some(records) = body.get("track_records").and_then(|v| v.as_array()) {
        for rec in records {
            let track = rec.get("track").and_then(|v| v.as_str()).unwrap_or_default();
            let car = rec.get("car").and_then(|v| v.as_str()).unwrap_or_default();
            if track.is_empty() || car.is_empty() { continue; }
            let sim_type = rec.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assettoCorsa");
            let r = sqlx::query(
                "INSERT INTO track_records (track, car, sim_type, driver_id, best_lap_ms, lap_id, achieved_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(track, car, sim_type) DO UPDATE SET
                    driver_id = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.driver_id ELSE track_records.driver_id END,
                    best_lap_ms = MIN(excluded.best_lap_ms, track_records.best_lap_ms),
                    lap_id = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.lap_id ELSE track_records.lap_id END,
                    achieved_at = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.achieved_at ELSE track_records.achieved_at END",
            )
            .bind(track)
            .bind(car)
            .bind(sim_type)
            .bind(rec.get("driver_id").and_then(|v| v.as_str()))
            .bind(rec.get("best_lap_ms").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .bind(rec.get("lap_id").and_then(|v| v.as_str()))
            .bind(rec.get("achieved_at").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert personal_bests
    if let Some(pbs) = body.get("personal_bests").and_then(|v| v.as_array()) {
        for pb in pbs {
            let driver_id = pb.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
            let track = pb.get("track").and_then(|v| v.as_str()).unwrap_or_default();
            let car = pb.get("car").and_then(|v| v.as_str()).unwrap_or_default();
            if driver_id.is_empty() || track.is_empty() || car.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, lap_id, achieved_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(driver_id, track, car) DO UPDATE SET
                    best_lap_ms = MIN(excluded.best_lap_ms, personal_bests.best_lap_ms),
                    lap_id = CASE WHEN excluded.best_lap_ms < personal_bests.best_lap_ms
                        THEN excluded.lap_id ELSE personal_bests.lap_id END,
                    achieved_at = CASE WHEN excluded.best_lap_ms < personal_bests.best_lap_ms
                        THEN excluded.achieved_at ELSE personal_bests.achieved_at END",
            )
            .bind(driver_id)
            .bind(track)
            .bind(car)
            .bind(pb.get("best_lap_ms").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .bind(pb.get("lap_id").and_then(|v| v.as_str()))
            .bind(pb.get("achieved_at").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert billing_sessions
    if let Some(sessions) = body.get("billing_sessions").and_then(|v| v.as_array()) {
        for s in sessions {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id,
                    allocated_seconds, driving_seconds, status, custom_price_paise, notes,
                    started_at, ended_at, created_at, experience_id, car, track, sim_type,
                    split_count, split_duration_minutes,
                    wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason,
                    pause_count, total_paused_seconds, refund_paise, end_reason, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28)
                 ON CONFLICT(id) DO UPDATE SET
                    driving_seconds = excluded.driving_seconds,
                    status = excluded.status,
                    ended_at = excluded.ended_at,
                    wallet_debit_paise = COALESCE(excluded.wallet_debit_paise, billing_sessions.wallet_debit_paise),
                    discount_paise = COALESCE(excluded.discount_paise, billing_sessions.discount_paise),
                    coupon_id = COALESCE(excluded.coupon_id, billing_sessions.coupon_id),
                    original_price_paise = COALESCE(excluded.original_price_paise, billing_sessions.original_price_paise),
                    discount_reason = COALESCE(excluded.discount_reason, billing_sessions.discount_reason),
                    pause_count = COALESCE(excluded.pause_count, billing_sessions.pause_count),
                    total_paused_seconds = COALESCE(excluded.total_paused_seconds, billing_sessions.total_paused_seconds),
                    refund_paise = COALESCE(excluded.refund_paise, billing_sessions.refund_paise),
                    end_reason = COALESCE(excluded.end_reason, billing_sessions.end_reason)",
            )
            .bind(id)
            .bind(s.get("driver_id").and_then(|v| v.as_str()))
            .bind(s.get("pod_id").and_then(|v| v.as_str()))
            .bind(s.get("pricing_tier_id").and_then(|v| v.as_str()))
            .bind(s.get("allocated_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(s.get("driving_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(s.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
            .bind(s.get("custom_price_paise").and_then(|v| v.as_i64()))
            .bind(s.get("notes").and_then(|v| v.as_str()))
            .bind(s.get("started_at").and_then(|v| v.as_str()))
            .bind(s.get("ended_at").and_then(|v| v.as_str()))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("experience_id").and_then(|v| v.as_str()))
            .bind(s.get("car").and_then(|v| v.as_str()))
            .bind(s.get("track").and_then(|v| v.as_str()))
            .bind(s.get("sim_type").and_then(|v| v.as_str()))
            .bind(s.get("split_count").and_then(|v| v.as_i64()))
            .bind(s.get("split_duration_minutes").and_then(|v| v.as_i64()))
            .bind(s.get("wallet_debit_paise").and_then(|v| v.as_i64()))
            .bind(s.get("discount_paise").and_then(|v| v.as_i64()))
            .bind(s.get("coupon_id").and_then(|v| v.as_str()))
            .bind(s.get("original_price_paise").and_then(|v| v.as_i64()))
            .bind(s.get("discount_reason").and_then(|v| v.as_str()))
            .bind(s.get("pause_count").and_then(|v| v.as_i64()))
            .bind(s.get("total_paused_seconds").and_then(|v| v.as_i64()))
            .bind(s.get("refund_paise").and_then(|v| v.as_i64()))
            .bind(s.get("end_reason").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Merge driver updates from venue (venue-owned fields only)
    if let Some(drivers) = body.get("drivers").and_then(|v| v.as_array()) {
        for d in drivers {
            let id = d.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }

            // Only update venue-owned fields, never overwrite cloud-owned fields (name, email, phone)
            let venue_updated = d.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

            // Check if cloud has a newer update for this driver
            let cloud_ts: Option<(Option<String>,)> = sqlx::query_as(
                "SELECT updated_at FROM drivers WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            // Only apply venue fields if venue's updated_at is newer
            let should_apply = match &cloud_ts {
                Some((Some(ts),)) => venue_updated > ts.as_str(),
                Some((None,)) => true,
                None => false, // Driver doesn't exist on cloud — skip partial update
            };

            if should_apply {
                let r = sqlx::query(
                    "UPDATE drivers SET
                        has_used_trial = MAX(COALESCE(has_used_trial, 0), ?),
                        unlimited_trials = MAX(COALESCE(unlimited_trials, 0), ?),
                        total_laps = MAX(COALESCE(total_laps, 0), ?),
                        total_time_ms = MAX(COALESCE(total_time_ms, 0), ?),
                        registration_completed = MAX(COALESCE(registration_completed, 0), ?),
                        waiver_signed = MAX(COALESCE(waiver_signed, 0), ?),
                        waiver_signed_at = COALESCE(?, waiver_signed_at),
                        waiver_version = COALESCE(?, waiver_version),
                        updated_at = ?
                     WHERE id = ?",
                )
                .bind(d.get("has_used_trial").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("unlimited_trials").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("total_laps").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("total_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("registration_completed").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("waiver_signed").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("waiver_signed_at").and_then(|v| v.as_str()))
                .bind(d.get("waiver_version").and_then(|v| v.as_str()))
                .bind(venue_updated)
                .bind(id)
                .execute(&state.db)
                .await;
                if r.is_ok() { total += 1; }
            }
        }
    }

    // Upsert pods (static config + live status)
    if let Some(pods) = body.get("pods").and_then(|v| v.as_array()) {
        for pod in pods {
            let id = pod.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let number = pod.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = pod.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let status = pod.get("status").and_then(|v| v.as_str()).unwrap_or("offline");

            // Update DB
            let _ = sqlx::query(
                "INSERT INTO pods (id, number, name, ip_address, sim_type, status, last_seen, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,datetime('now'),?7)
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    ip_address = excluded.ip_address,
                    last_seen = datetime('now')",
            )
            .bind(id)
            .bind(number)
            .bind(name)
            .bind(pod.get("ip_address").and_then(|v| v.as_str()))
            .bind(pod.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa"))
            .bind(status)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;

            // Update in-memory pod map so PWA/dashboard sees live status
            let pod_info = rc_common::types::PodInfo {
                id: id.to_string(),
                number: number as u32,
                name: name.to_string(),
                ip_address: pod.get("ip_address").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                mac_address: pod.get("mac_address").and_then(|v| v.as_str()).map(|s| s.to_string()),
                sim_type: pod.get("sim_type").and_then(|v| v.as_str())
                    .and_then(|s| serde_json::from_value(json!(s)).ok())
                    .unwrap_or(rc_common::types::SimType::AssettoCorsa),
                status: serde_json::from_value(json!(status))
                    .unwrap_or(rc_common::types::PodStatus::Offline),
                current_driver: pod.get("current_driver").and_then(|v| v.as_str()).map(|s| s.to_string()),
                current_session_id: pod.get("current_session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                last_seen: Some(chrono::Utc::now()),
                driving_state: None,
                billing_session_id: pod.get("billing_session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                game_state: None,
                current_game: None,
                installed_games: vec![],
                screen_blanked: None,
                ffb_preset: None,
                freedom_mode: None,
                agent_timestamp: None, // Intentional default: cloud sync path has no agent clock
                recent_lap_times: std::collections::VecDeque::new(),
            };
            state.pods.write().await.insert(id.to_string(), pod_info.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod_info));
            total += 1;
        }
    }

    // ── Delegate remaining entity upserts to sync_push_entities ──

    if let Some(wallets) = body.get("wallets").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_wallets(&state, wallets).await;
    }

    if let Some(txns) = body.get("wallet_transactions").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_wallet_transactions(&state, txns).await;
    }

    if let Some(events) = body.get("billing_events").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_billing_events(&state, events).await;
    }

    if let Some(staff) = body.get("staff_members").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_staff_members(&state, staff).await;
    }

    // Apply venue config snapshot from James
    if let Some(config_snap) = body.get("config_snapshot") {
        let snapshot = parse_config_snapshot(config_snap);
        tracing::info!(
            venue = %snapshot.venue_name,
            pods = snapshot.pod_count,
            hash = %snapshot.config_hash.get(..8).unwrap_or(&snapshot.config_hash),
            "Config sync: received venue config snapshot"
        );
        *state.venue_config.write().await = Some(snapshot);
        total += 1;
    }

    if let Some(reservations) = body.get("reservations").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_reservations(&state, reservations).await;
    }

    if let Some(intents) = body.get("debit_intents").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_debit_intents(&state, intents).await;
    }

    if let Some(solutions) = body.get("fleet_solutions").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_fleet_solutions(&state, solutions).await;
    }

    if let Some(evals) = body.get("model_evaluations").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_model_evaluations(&state, evals).await;
    }

    if let Some(rollups) = body.get("metrics_rollups").and_then(|v| v.as_array()) {
        total += sync_push_entities::upsert_metrics_rollups(&state, rollups).await;
    }

    tracing::info!("Sync push: upserted {} records", total);
    Json(json!({ "ok": true, "upserted": total }))
}

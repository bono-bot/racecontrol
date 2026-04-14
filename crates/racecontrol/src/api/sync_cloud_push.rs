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
                    lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
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
            let r = sqlx::query(
                "INSERT INTO track_records (track, car, driver_id, best_lap_ms, lap_id, achieved_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(track, car) DO UPDATE SET
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

    // Upsert wallets (venue pushes balances after billing debits)
    // Handles ID mismatch: if direct driver_id doesn't match, resolve by phone/email
    if let Some(wallets) = body.get("wallets").and_then(|v| v.as_array()) {
        for w in wallets {
            let driver_id = w.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
            if driver_id.is_empty() { continue; }

            let balance = w.get("balance_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let credited = w.get("total_credited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let debited = w.get("total_debited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let updated = w.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

            // Try direct driver_id match first
            let r = sqlx::query(
                "UPDATE wallets SET
                    balance_paise = ?, total_credited_paise = ?,
                    total_debited_paise = ?, updated_at = ?
                 WHERE driver_id = ?",
            )
            .bind(balance).bind(credited).bind(debited).bind(updated)
            .bind(driver_id)
            .execute(&state.db)
            .await;

            let rows = r.as_ref().map(|r| r.rows_affected()).unwrap_or(0);
            if rows > 0 {
                total += 1;
                continue;
            }

            // ID didn't match — try to find local driver by phone or email
            let phone = w.get("phone").and_then(|v| v.as_str()).unwrap_or("");
            let email = w.get("email").and_then(|v| v.as_str()).unwrap_or("");

            let resolved: Option<(String,)> = if !phone.is_empty() {
                let ph = state.field_cipher.hash_phone(phone);
                sqlx::query_as("SELECT id FROM drivers WHERE phone_hash = ?")
                    .bind(&ph)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
            } else if !email.is_empty() {
                sqlx::query_as("SELECT id FROM drivers WHERE email = ?")
                    .bind(email)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            if let Some((local_id,)) = resolved {
                let r2 = sqlx::query(
                    "UPDATE wallets SET
                        balance_paise = ?, total_credited_paise = ?,
                        total_debited_paise = ?, updated_at = ?
                     WHERE driver_id = ?",
                )
                .bind(balance).bind(credited).bind(debited).bind(updated)
                .bind(&local_id)
                .execute(&state.db)
                .await;
                if r2.is_ok() {
                    tracing::info!("Wallet sync: resolved {} -> {} by phone/email", driver_id, local_id);
                    total += 1;
                }
            }
        }
    }

    // Upsert wallet_transactions (immutable — INSERT OR IGNORE by UUID for idempotency)
    if let Some(txns) = body.get("wallet_transactions").and_then(|v| v.as_array()) {
        for txn in txns {
            let id = txn.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT OR IGNORE INTO wallet_transactions
                    (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, created_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            )
            .bind(id)
            .bind(txn.get("driver_id").and_then(|v| v.as_str()))
            .bind(txn.get("amount_paise").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(txn.get("balance_after_paise").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(txn.get("txn_type").and_then(|v| v.as_str()).unwrap_or("adjustment"))
            .bind(txn.get("reference_id").and_then(|v| v.as_str()))
            .bind(txn.get("notes").and_then(|v| v.as_str()))
            .bind(txn.get("staff_id").and_then(|v| v.as_str()))
            .bind(txn.get("created_at").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
        tracing::info!("Sync push: {} wallet transactions", txns.len());

        // Shadow verification: compare latest transaction balance with wallet balance
        // Collect unique driver_ids from the pushed transactions
        let mut driver_ids: Vec<String> = txns.iter()
            .filter_map(|t| t.get("driver_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        driver_ids.sort();
        driver_ids.dedup();

        for did in &driver_ids {
            // Get the most recent transaction's balance_after_paise for this driver
            let txn_balance: Option<(i64,)> = sqlx::query_as(
                "SELECT balance_after_paise FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT 1",
            )
            .bind(did)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let wallet_balance: Option<(i64,)> = sqlx::query_as(
                "SELECT balance_paise FROM wallets WHERE driver_id = ?",
            )
            .bind(did)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let (Some((txn_bal,)), Some((wallet_bal,))) = (txn_balance, wallet_balance) {
                if txn_bal != wallet_bal {
                    tracing::warn!(
                        driver_id = %did,
                        wallet_balance = wallet_bal,
                        txn_balance = txn_bal,
                        diff = wallet_bal - txn_bal,
                        "Wallet balance discrepancy detected in shadow verification"
                    );
                }
            }
        }
    }

    // Insert billing events (immutable — INSERT OR IGNORE)
    if let Some(events) = body.get("billing_events").and_then(|v| v.as_array()) {
        for ev in events {
            let id = ev.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT OR IGNORE INTO billing_events
                    (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )
            .bind(id)
            .bind(ev.get("billing_session_id").and_then(|v| v.as_str()))
            .bind(ev.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown"))
            .bind(ev.get("driving_seconds_at_event").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(ev.get("metadata").and_then(|v| v.as_str()))
            .bind(ev.get("created_at").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
        tracing::info!("Sync push: {} billing events", events.len());
    }

    // Upsert staff_members (venue -> cloud or cloud -> venue)
    if let Some(staff) = body.get("staff_members").and_then(|v| v.as_array()) {
        for s in staff {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO staff_members (id, name, phone, pin, is_active, role, created_at, updated_at, last_login_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name, phone = excluded.phone, pin = excluded.pin,
                    is_active = excluded.is_active, role = excluded.role,
                    updated_at = excluded.updated_at, last_login_at = excluded.last_login_at",
            )
            .bind(id)
            .bind(s.get("name").and_then(|v| v.as_str()))
            .bind(s.get("phone").and_then(|v| v.as_str()))
            .bind(s.get("pin").and_then(|v| v.as_str()))
            .bind(s.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(s.get("role").and_then(|v| v.as_str()).unwrap_or("staff"))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("updated_at").and_then(|v| v.as_str()))
            .bind(s.get("last_login_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
        tracing::info!("Sync push: {} staff_members", staff.len());
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

    // Upsert reservations (cloud-authoritative: cloud creates, local updates status)
    if let Some(reservations) = body.get("reservations").and_then(|v| v.as_array()) {
        for r in reservations {
            let id = r.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let res = sqlx::query(
                "INSERT INTO reservations (id, driver_id, experience_id, pin, status,
                    pod_number, debit_intent_id, created_at, expires_at, redeemed_at,
                    cancelled_at, updated_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    pod_number = COALESCE(excluded.pod_number, reservations.pod_number),
                    debit_intent_id = COALESCE(excluded.debit_intent_id, reservations.debit_intent_id),
                    redeemed_at = COALESCE(excluded.redeemed_at, reservations.redeemed_at),
                    cancelled_at = COALESCE(excluded.cancelled_at, reservations.cancelled_at),
                    updated_at = excluded.updated_at",
            )
            .bind(id)
            .bind(r.get("driver_id").and_then(|v| v.as_str()))
            .bind(r.get("experience_id").and_then(|v| v.as_str()))
            .bind(r.get("pin").and_then(|v| v.as_str()))
            .bind(r.get("status").and_then(|v| v.as_str()).unwrap_or("pending_debit"))
            .bind(r.get("pod_number").and_then(|v| v.as_i64()))
            .bind(r.get("debit_intent_id").and_then(|v| v.as_str()))
            .bind(r.get("created_at").and_then(|v| v.as_str()))
            .bind(r.get("expires_at").and_then(|v| v.as_str()))
            .bind(r.get("redeemed_at").and_then(|v| v.as_str()))
            .bind(r.get("cancelled_at").and_then(|v| v.as_str()))
            .bind(r.get("updated_at").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if res.is_ok() { total += 1; }
        }
    }

    // Upsert debit_intents (cloud creates pending, local processes and updates status)
    if let Some(intents) = body.get("debit_intents").and_then(|v| v.as_array()) {
        for di in intents {
            let id = di.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let res = sqlx::query(
                "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id,
                    status, failure_reason, wallet_txn_id, origin, created_at,
                    processed_at, updated_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    failure_reason = COALESCE(excluded.failure_reason, debit_intents.failure_reason),
                    wallet_txn_id = COALESCE(excluded.wallet_txn_id, debit_intents.wallet_txn_id),
                    processed_at = COALESCE(excluded.processed_at, debit_intents.processed_at),
                    updated_at = excluded.updated_at",
            )
            .bind(id)
            .bind(di.get("driver_id").and_then(|v| v.as_str()))
            .bind(di.get("amount_paise").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(di.get("reservation_id").and_then(|v| v.as_str()))
            .bind(di.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
            .bind(di.get("failure_reason").and_then(|v| v.as_str()))
            .bind(di.get("wallet_txn_id").and_then(|v| v.as_str()))
            .bind(di.get("origin").and_then(|v| v.as_str()).unwrap_or("cloud"))
            .bind(di.get("created_at").and_then(|v| v.as_str()))
            .bind(di.get("processed_at").and_then(|v| v.as_str()))
            .bind(di.get("updated_at").and_then(|v| v.as_str()))
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;
            if res.is_ok() { total += 1; }
        }
    }

    // Phase 301: Upsert fleet_solutions with LWW + venue_id tiebreaker (SYNC-04/05)
    if let Some(solutions) = body.get("fleet_solutions").and_then(|v| v.as_array()) {
        let mut conflicts = 0i64;
        for sol in solutions {
            let id = sol.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let ts = crate::cloud_sync::normalize_timestamp(
                sol.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
            );
            let r = sqlx::query(
                "INSERT INTO fleet_solutions
                    (id, problem_key, problem_hash, symptoms, environment, root_cause,
                     fix_action, fix_type, status, success_count, fail_count, confidence,
                     cost_to_diagnose, models_used, diagnosis_tier, source_node, venue_id,
                     created_at, updated_at, version, ttl_days, tags)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
                 ON CONFLICT(id) DO UPDATE SET
                    root_cause = excluded.root_cause,
                    fix_action = excluded.fix_action,
                    status = excluded.status,
                    success_count = excluded.success_count,
                    fail_count = excluded.fail_count,
                    confidence = excluded.confidence,
                    cost_to_diagnose = excluded.cost_to_diagnose,
                    models_used = excluded.models_used,
                    updated_at = excluded.updated_at,
                    version = excluded.version,
                    venue_id = excluded.venue_id
                 WHERE excluded.updated_at > fleet_solutions.updated_at
                    OR (excluded.updated_at = fleet_solutions.updated_at
                        AND excluded.venue_id < fleet_solutions.venue_id)",
            )
            .bind(id)
            .bind(sol.get("problem_key").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("problem_hash").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("symptoms").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("environment").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("root_cause").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("fix_action").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("fix_type").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("status").and_then(|v| v.as_str()).unwrap_or("candidate"))
            .bind(sol.get("success_count").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(sol.get("fail_count").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(sol.get("confidence").and_then(|v| v.as_f64()).unwrap_or(1.0))
            .bind(sol.get("cost_to_diagnose").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .bind(sol.get("models_used").and_then(|v| v.as_str()))
            .bind(sol.get("diagnosis_tier").and_then(|v| v.as_str()).unwrap_or("deterministic"))
            .bind(sol.get("source_node").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(sol.get("venue_id").and_then(|v| v.as_str()))
            .bind(sol.get("created_at").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(&ts)
            .bind(sol.get("version").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(sol.get("ttl_days").and_then(|v| v.as_i64()).unwrap_or(90))
            .bind(sol.get("tags").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            match r {
                Ok(res) => {
                    if res.rows_affected() > 0 { total += 1; }
                    else { conflicts += 1; }
                }
                Err(e) => { tracing::warn!("fleet_solutions upsert error: {}", e); }
            }
        }
        if conflicts > 0 {
            let _ = sqlx::query(
                "UPDATE sync_state SET conflict_count = COALESCE(conflict_count, 0) + ?1
                 WHERE table_name = 'fleet_solutions'"
            ).bind(conflicts).execute(&state.db).await;
        }
        tracing::info!("Sync push: {} fleet_solutions ({} conflicts)", solutions.len(), conflicts);
    }

    // Phase 301: Upsert model_evaluations with LWW + venue_id tiebreaker (SYNC-04/05)
    if let Some(evals) = body.get("model_evaluations").and_then(|v| v.as_array()) {
        let mut conflicts = 0i64;
        for ev in evals {
            let id = ev.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let ts = crate::cloud_sync::normalize_timestamp(
                ev.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
            );
            let r = sqlx::query(
                "INSERT INTO model_evaluations
                    (id, model_name, pod_id, problem_key, prediction, actual,
                     correct, cost_usd, diagnosis_tier, created_at, updated_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(id) DO UPDATE SET
                    prediction = excluded.prediction,
                    actual = excluded.actual,
                    correct = excluded.correct,
                    cost_usd = excluded.cost_usd,
                    diagnosis_tier = excluded.diagnosis_tier,
                    updated_at = excluded.updated_at,
                    venue_id = excluded.venue_id
                 WHERE excluded.updated_at > model_evaluations.updated_at
                    OR (excluded.updated_at = model_evaluations.updated_at
                        AND excluded.venue_id < model_evaluations.venue_id)",
            )
            .bind(id)
            .bind(ev.get("model_name").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(ev.get("pod_id").and_then(|v| v.as_str()))
            .bind(ev.get("problem_key").and_then(|v| v.as_str()))
            .bind(ev.get("prediction").and_then(|v| v.as_str()))
            .bind(ev.get("actual").and_then(|v| v.as_str()))
            .bind(ev.get("correct").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(ev.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .bind(ev.get("diagnosis_tier").and_then(|v| v.as_str()))
            .bind(ev.get("created_at").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(&ts)
            .bind(ev.get("venue_id").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            match r {
                Ok(res) => {
                    if res.rows_affected() > 0 { total += 1; }
                    else { conflicts += 1; }
                }
                Err(e) => { tracing::warn!("model_evaluations upsert error: {}", e); }
            }
        }
        if conflicts > 0 {
            let _ = sqlx::query(
                "UPDATE sync_state SET conflict_count = COALESCE(conflict_count, 0) + ?1
                 WHERE table_name = 'model_evaluations'"
            ).bind(conflicts).execute(&state.db).await;
        }
        tracing::info!("Sync push: {} model_evaluations ({} conflicts)", evals.len(), conflicts);
    }

    // Phase 301: Upsert metrics_rollups using UNIQUE constraint (not id) with LWW (SYNC-04/05)
    // Do NOT include id — AUTOINCREMENT, target DB assigns its own
    if let Some(rollups) = body.get("metrics_rollups").and_then(|v| v.as_array()) {
        let mut conflicts = 0i64;
        for ru in rollups {
            let resolution = ru.get("resolution").and_then(|v| v.as_str()).unwrap_or_default();
            let metric_name = ru.get("metric_name").and_then(|v| v.as_str()).unwrap_or_default();
            let period_start = ru.get("period_start").and_then(|v| v.as_str()).unwrap_or_default();
            if resolution.is_empty() || metric_name.is_empty() || period_start.is_empty() { continue; }
            let ts = crate::cloud_sync::normalize_timestamp(
                ru.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
            );
            let r = sqlx::query(
                "INSERT INTO metrics_rollups
                    (resolution, metric_name, pod_id, min_value, max_value, avg_value,
                     sample_count, period_start, updated_at, venue_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                 ON CONFLICT(resolution, metric_name, pod_id, period_start) DO UPDATE SET
                    avg_value = CASE WHEN excluded.sample_count > metrics_rollups.sample_count
                                THEN excluded.avg_value ELSE metrics_rollups.avg_value END,
                    min_value = MIN(excluded.min_value, metrics_rollups.min_value),
                    max_value = MAX(excluded.max_value, metrics_rollups.max_value),
                    sample_count = MAX(excluded.sample_count, metrics_rollups.sample_count),
                    updated_at = excluded.updated_at,
                    venue_id = excluded.venue_id
                 WHERE excluded.updated_at > metrics_rollups.updated_at
                    OR metrics_rollups.updated_at IS NULL",
            )
            .bind(resolution)
            .bind(metric_name)
            .bind(ru.get("pod_id").and_then(|v| v.as_str()))
            .bind(ru.get("min_value").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .bind(ru.get("max_value").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .bind(ru.get("avg_value").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .bind(ru.get("sample_count").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(period_start)
            .bind(&ts)
            .bind(ru.get("venue_id").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            match r {
                Ok(res) => {
                    if res.rows_affected() > 0 { total += 1; }
                    else { conflicts += 1; }
                }
                Err(e) => { tracing::warn!("metrics_rollups upsert error: {}", e); }
            }
        }
        if conflicts > 0 {
            let _ = sqlx::query(
                "UPDATE sync_state SET conflict_count = COALESCE(conflict_count, 0) + ?1
                 WHERE table_name = 'metrics_rollups'"
            ).bind(conflicts).execute(&state.db).await;
        }
        tracing::info!("Sync push: {} metrics_rollups ({} conflicts)", rollups.len(), conflicts);
    }

    tracing::info!("Sync push: upserted {} records", total);
    Json(json!({ "ok": true, "upserted": total }))
}

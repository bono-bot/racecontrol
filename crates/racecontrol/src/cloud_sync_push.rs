//! Cloud sync push logic — collecting venue data and pushing to cloud.
//! Extracted from cloud_sync.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::cloud_sync::{sign_sync_request, normalize_timestamp, SYNC_TABLES};
use crate::state::AppState;

/// Schema version bumped when tables/columns change.
/// Cloud side can reject pushes if it hasn't migrated yet.
const SCHEMA_VERSION: u32 = 4;

/// Push sync deltas via the comms-link relay (localhost HTTP).
/// In relay mode, only pushes are needed — the other side pushes to us independently
/// via the /sync/push endpoint (called by comms-link when it receives WS sync_push).
///
/// ## Anti-loop protection
///
/// Sync loops are prevented by the `_push` timestamp in `sync_state`:
/// 1. After a successful push (relay or HTTP), `update_push_state()` records the current time.
/// 2. The next `collect_push_payload()` call queries `WHERE created_at > last_push` (or `updated_at >`).
/// 3. When the OTHER side pushes data to us via `/sync/push` (routes.rs), that handler does NOT
///    call `update_push_state()` — it only upserts received data into the DB.
/// 4. The received data has timestamps older than "now", and since our `_push` was updated after
///    our last outbound push, the received data's timestamps fall before `_push` and won't be
///    re-collected in our next push cycle.
///
/// This means: Cloud pushes to Venue -> Venue receives via /sync/push -> Venue's own push cycle
/// won't re-push that data because its timestamps are older than Venue's `_push` marker.
/// The same logic works in reverse (Venue -> Cloud).
pub(crate) async fn push_via_relay(state: &Arc<AppState>) -> anyhow::Result<()> {
    let relay_url = state
        .config
        .cloud
        .comms_link_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("comms_link_url not configured"))?;

    let (payload, has_data) = collect_push_payload(state).await?;
    if !has_data {
        tracing::debug!("Cloud sync relay: nothing to push");
        return Ok(());
    }

    let url = format!("{}/relay/sync", relay_url);
    let resp = state
        .http_client
        .post(&url)
        .json(&payload)
        .timeout(Duration::from_secs(2))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Relay sync returned status {}", resp.status());
    }

    // Update push timestamp on success
    update_push_state(state).await;

    tracing::debug!("Cloud sync relay: push successful");
    Ok(())
}

/// Process pending debit intents received from cloud.
/// Called after sync pull to process wallet debits on the local server.
/// Local is the single writer for wallet debits -- cloud NEVER directly modifies wallet.
pub(crate) async fn process_debit_intents(state: &Arc<AppState>) -> anyhow::Result<u64> {
    let pending = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT id, driver_id, amount_paise, reservation_id
         FROM debit_intents WHERE status = 'pending' ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await?;

    if pending.is_empty() {
        return Ok(0);
    }

    let mut processed = 0u64;
    for (intent_id, driver_id, amount, reservation_id) in &pending {
        let balance = sqlx::query_as::<_, (i64,)>(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await?;

        match balance {
            Some((bal,)) if bal >= *amount => {
                let new_balance = bal - amount;
                let txn_id = uuid::Uuid::new_v4().to_string();

                // Debit wallet
                sqlx::query(
                    "UPDATE wallets SET balance_paise = ?, total_debited_paise = total_debited_paise + ?,
                     updated_at = datetime('now') WHERE driver_id = ?",
                )
                .bind(new_balance).bind(amount).bind(driver_id)
                .execute(&state.db).await?;

                // Record wallet transaction (per D-20: include currency_type = 'credit' for all debits per D-06)
                sqlx::query(
                    "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise,
                     txn_type, reference_id, notes, created_at, venue_id, currency_type)
                     VALUES (?, ?, ?, ?, 'debit_session', ?, 'Remote booking debit', datetime('now'), ?, 'credit')",
                )
                .bind(&txn_id).bind(driver_id).bind(-amount).bind(new_balance).bind(reservation_id)
                .bind(&state.config.venue.venue_id)
                .execute(&state.db).await?;

                // Mark intent completed
                sqlx::query(
                    "UPDATE debit_intents SET status = 'completed', wallet_txn_id = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(&txn_id).bind(intent_id)
                .execute(&state.db).await?;

                // Update reservation to confirmed
                sqlx::query(
                    "UPDATE reservations SET status = 'confirmed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                tracing::info!(target: "sync", "Debit intent {} completed: {} paise from driver {}",
                    intent_id, amount, driver_id);
                processed += 1;
            }
            _ => {
                // Insufficient balance or no wallet
                let reason = if balance.is_none() { "no_wallet" } else { "insufficient_balance" };
                sqlx::query(
                    "UPDATE debit_intents SET status = 'failed', failure_reason = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(reason).bind(intent_id)
                .execute(&state.db).await?;

                sqlx::query(
                    "UPDATE reservations SET status = 'failed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                tracing::warn!(target: "sync", "Debit intent {} failed ({}): {} paise from driver {}",
                    intent_id, reason, amount, driver_id);
                processed += 1;
            }
        }
    }

    if processed > 0 {
        tracing::info!(target: "sync", "Processed {} debit intents", processed);
    }
    Ok(processed)
}

/// Collect the push payload (shared between relay and HTTP push paths).
/// Returns (payload, has_data).
pub(crate) async fn collect_push_payload(state: &Arc<AppState>) -> anyhow::Result<(Value, bool)> {
    let last_push = normalize_timestamp(&get_last_push_time(state).await);
    let origin = &state.config.cloud.origin_id;
    let mut payload = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "origin": origin,
    });
    let mut has_data = false;

    // Collect laps since last push
    let laps = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'session_id', session_id, 'driver_id', driver_id,
            'pod_id', pod_id, 'sim_type', sim_type, 'track', track, 'car', car,
            'lap_number', lap_number, 'lap_time_ms', lap_time_ms,
            'sector1_ms', sector1_ms, 'sector2_ms', sector2_ms, 'sector3_ms', sector3_ms,
            'valid', valid, 'created_at', created_at
        ) FROM laps WHERE created_at > ? ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !laps.is_empty() {
        let items: Vec<serde_json::Value> = laps.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} laps", items.len());
        payload["laps"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect track records (always push all — small table)
    let records = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'track', track, 'car', car, 'driver_id', driver_id,
            'best_lap_ms', best_lap_ms, 'lap_id', lap_id, 'achieved_at', achieved_at
        ) FROM track_records",
    )
    .fetch_all(&state.db)
    .await?;

    if !records.is_empty() {
        let items: Vec<serde_json::Value> = records.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        payload["track_records"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect personal bests (always push all — small table)
    let pbs = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'driver_id', driver_id, 'track', track, 'car', car,
            'best_lap_ms', best_lap_ms, 'lap_id', lap_id, 'achieved_at', achieved_at
        ) FROM personal_bests",
    )
    .fetch_all(&state.db)
    .await?;

    if !pbs.is_empty() {
        let items: Vec<serde_json::Value> = pbs.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        payload["personal_bests"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect billing sessions since last push
    let sessions = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'pod_id', pod_id,
            'pricing_tier_id', pricing_tier_id, 'allocated_seconds', allocated_seconds,
            'driving_seconds', driving_seconds, 'status', status,
            'custom_price_paise', custom_price_paise, 'notes', notes,
            'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at,
            'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type,
            'split_count', split_count, 'split_duration_minutes', split_duration_minutes,
            'wallet_debit_paise', wallet_debit_paise,
            'discount_paise', discount_paise, 'coupon_id', coupon_id,
            'original_price_paise', original_price_paise, 'discount_reason', discount_reason,
            'pause_count', pause_count, 'total_paused_seconds', total_paused_seconds, 'refund_paise', refund_paise,
            'end_reason', end_reason,
            'lap_count_expected', lap_count_expected,
            'lap_count_actual', lap_count_actual,
            'lap_count_flag', COALESCE(lap_count_flag, 'UNVERIFIED'),
            'telemetry_coverage_pct', telemetry_coverage_pct,
            'suspect', COALESCE(suspect, 0),
            'suspect_reasons', suspect_reasons,
            'csv_fallback_received_at', csv_fallback_received_at,
            'lap_reject_grace_until', lap_reject_grace_until
        ) FROM billing_sessions WHERE created_at >= ? OR ended_at >= ?
        ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !sessions.is_empty() {
        let items: Vec<serde_json::Value> = sessions.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} billing sessions", items.len());
        payload["billing_sessions"] = serde_json::json!(items);
        has_data = true;
    }

    // Push driver changes (has_used_trial, total_laps, total_time_ms, registration)
    let drivers = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'has_used_trial', COALESCE(has_used_trial, 0),
            'unlimited_trials', COALESCE(unlimited_trials, 0),
            'total_laps', COALESCE(total_laps, 0),
            'total_time_ms', COALESCE(total_time_ms, 0),
            'registration_completed', COALESCE(registration_completed, 0),
            'waiver_signed', COALESCE(waiver_signed, 0),
            'waiver_signed_at', waiver_signed_at,
            'waiver_version', waiver_version,
            'updated_at', updated_at
        ) FROM drivers WHERE updated_at > ?
        ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !drivers.is_empty() {
        let items: Vec<serde_json::Value> = drivers.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} driver updates", items.len());
        payload["drivers"] = serde_json::json!(items);
        has_data = true;
    }

    // Push live pod status from in-memory state
    let pods = state.pods.read().await;
    if !pods.is_empty() {
        let pod_list: Vec<serde_json::Value> = pods.values().map(|p| {
            serde_json::json!({
                "id": p.id,
                "number": p.number,
                "name": p.name,
                "ip_address": p.ip_address,
                "mac_address": p.mac_address,
                "sim_type": p.sim_type,
                "status": p.status,
                "game_state": p.game_state,
                "current_game": p.current_game,
                "current_driver": p.current_driver,
                "current_session_id": p.current_session_id,
                "billing_session_id": p.billing_session_id,
            })
        }).collect();
        payload["pods"] = serde_json::json!(pod_list);
        has_data = true;
    }
    drop(pods);

    // Push wallet balances (venue is authoritative for debits)
    // Include driver phone/email so cloud can match by identity when IDs differ
    let wallets = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'driver_id', w.driver_id, 'balance_paise', w.balance_paise,
            'total_credited_paise', w.total_credited_paise,
            'total_debited_paise', w.total_debited_paise,
            'rupee_deposited_paise', w.rupee_deposited_paise,
            'rupee_refunded_paise', w.rupee_refunded_paise,
            'bonus_credited_paise', w.bonus_credited_paise,
            'updated_at', w.updated_at,
            'phone', d.phone, 'email', d.email
        ) FROM wallets w JOIN drivers d ON d.id = w.driver_id
        WHERE w.updated_at > ?",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !wallets.is_empty() {
        let items: Vec<serde_json::Value> = wallets.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} wallets", items.len());
        payload["wallets"] = serde_json::json!(items);
        has_data = true;
    }

    // Push wallet transactions (immutable, use >= to avoid missing same-timestamp rows)
    let wallet_txns = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
            'balance_after_paise', balance_after_paise, 'txn_type', txn_type,
            'reference_id', reference_id, 'notes', notes, 'staff_id', staff_id,
            'created_at', created_at
        ) FROM wallet_transactions WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !wallet_txns.is_empty() {
        let items: Vec<serde_json::Value> = wallet_txns.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} wallet transactions", items.len());
        payload["wallet_transactions"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect billing events since last push
    let billing_events = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'billing_session_id', billing_session_id,
            'event_type', event_type, 'driving_seconds_at_event', driving_seconds_at_event,
            'metadata', metadata, 'created_at', created_at
        ) FROM billing_events WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !billing_events.is_empty() {
        let items: Vec<serde_json::Value> = billing_events.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} billing events", items.len());
        payload["billing_events"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect reservation status updates (local updates: redeemed, expired status changes)
    let reservations = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'experience_id', experience_id,
            'pin', pin, 'status', status, 'pod_number', pod_number,
            'debit_intent_id', debit_intent_id,
            'created_at', created_at, 'expires_at', expires_at,
            'redeemed_at', redeemed_at, 'cancelled_at', cancelled_at,
            'updated_at', updated_at
        ) FROM reservations WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !reservations.is_empty() {
        let items: Vec<serde_json::Value> = reservations.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} reservations", items.len());
        payload["reservations"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect debit intent status updates (local processes: completed/failed results)
    let intents = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
            'reservation_id', reservation_id, 'status', status,
            'failure_reason', failure_reason, 'wallet_txn_id', wallet_txn_id,
            'origin', origin,
            'created_at', created_at, 'processed_at', processed_at,
            'updated_at', updated_at
        ) FROM debit_intents WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !intents.is_empty() {
        let items: Vec<serde_json::Value> = intents.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} debit_intents", items.len());
        payload["debit_intents"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect staff_members changes since last push
    let staff = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'name', name, 'phone', phone, 'pin', pin,
            'is_active', is_active, 'role', COALESCE(role, 'staff'),
            'created_at', created_at, 'updated_at', updated_at,
            'last_login_at', last_login_at
        ) FROM staff_members
        WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
        ORDER BY COALESCE(updated_at, created_at) ASC LIMIT 500",
    )
    .bind(&last_push)
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !staff.is_empty() {
        let items: Vec<serde_json::Value> = staff.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} staff_members", items.len());
        payload["staff_members"] = serde_json::json!(items);
        has_data = true;
    }

    // Phase 301: Push fleet_solutions (AI knowledge base) since last push (SYNC-01)
    let solutions = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'problem_key', problem_key, 'problem_hash', problem_hash,
            'symptoms', symptoms, 'environment', environment, 'root_cause', root_cause,
            'fix_action', fix_action, 'fix_type', fix_type, 'status', status,
            'success_count', success_count, 'fail_count', fail_count,
            'confidence', confidence, 'cost_to_diagnose', cost_to_diagnose,
            'models_used', models_used, 'diagnosis_tier', diagnosis_tier,
            'source_node', source_node, 'venue_id', venue_id,
            'created_at', created_at, 'updated_at', updated_at,
            'version', version, 'ttl_days', ttl_days, 'tags', tags
        ) FROM fleet_solutions WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !solutions.is_empty() {
        let items: Vec<serde_json::Value> = solutions.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} fleet_solutions", items.len());
        payload["fleet_solutions"] = serde_json::json!(items);
        has_data = true;
    }

    // Phase 301: Push model_evaluations (AI diagnosis accuracy) since last push (SYNC-02)
    let evaluations = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'model_name', model_name, 'pod_id', pod_id,
            'problem_key', problem_key, 'prediction', prediction, 'actual', actual,
            'correct', correct, 'cost_usd', cost_usd, 'diagnosis_tier', diagnosis_tier,
            'created_at', created_at, 'updated_at', updated_at, 'venue_id', venue_id
        ) FROM model_evaluations WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !evaluations.is_empty() {
        let items: Vec<serde_json::Value> = evaluations.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} model_evaluations", items.len());
        payload["model_evaluations"] = serde_json::json!(items);
        has_data = true;
    }

    // Phase 366: Push content_drift_events (fleet intelligence audit log) since last push
    let drift_events = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'pod_id', pod_id, 'detected_at', detected_at,
            'game_key', game_key, 'delta_type', delta_type, 'item', item,
            'resolved_at', resolved_at, 'resolution_note', resolution_note
        ) FROM content_drift_events WHERE detected_at > ? ORDER BY detected_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await;

    if let Ok(rows) = drift_events {
        if !rows.is_empty() {
            let items: Vec<serde_json::Value> = rows.iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            tracing::info!("Cloud sync push: {} content_drift_events", items.len());
            payload["content_drift_events"] = serde_json::json!(items);
            has_data = true;
        }
    }

    // Phase 301: Push metrics_rollups (operational metrics) since last push (SYNC-03)
    // Do NOT include id (AUTOINCREMENT) — target DB assigns its own
    let rollups = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'resolution', resolution, 'metric_name', metric_name, 'pod_id', pod_id,
            'min_value', min_value, 'max_value', max_value, 'avg_value', avg_value,
            'sample_count', sample_count, 'period_start', period_start,
            'updated_at', COALESCE(updated_at, datetime('now')), 'venue_id', venue_id
        ) FROM metrics_rollups WHERE COALESCE(updated_at, datetime('now')) > ? ORDER BY COALESCE(updated_at, datetime('now')) ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !rollups.is_empty() {
        let items: Vec<serde_json::Value> = rollups.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} metrics_rollups", items.len());
        payload["metrics_rollups"] = serde_json::json!(items);
        has_data = true;
    }

    Ok((payload, has_data))
}

/// Push venue-generated data (laps, billing, pods, leaderboard) to cloud via direct HTTP.
pub(crate) async fn push_to_cloud(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
    let (payload, has_data) = collect_push_payload(state).await?;

    if !has_data {
        tracing::debug!("Cloud sync push: nothing to push");
        return Ok(());
    }

    // POST to cloud
    let push_url = format!("{}/sync/push", cloud_url);
    let body_bytes = serde_json::to_vec(&payload)?;
    let mut req = state.http_client
        .post(&push_url)
        .header("content-type", "application/json")
        .body(body_bytes.clone())
        .timeout(std::time::Duration::from_secs(30));

    if let Some(secret) = &state.config.cloud.terminal_secret {
        req = req.header("x-terminal-secret", secret);
    }

    // HMAC-SHA256 signing (AUTH-07)
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let (signature, timestamp, nonce) = sign_sync_request(&body_bytes, hmac_key.as_bytes());
        req = req
            .header("x-sync-timestamp", timestamp.to_string())
            .header("x-sync-nonce", &nonce)
            .header("x-sync-signature", &signature);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Cloud push failed (network): {e} — will retry next cycle");
            return Ok(());
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!("Cloud push rejected (HTTP {status}): {body} — skipping until next cycle");
        return Ok(());
    }

    let body_bytes = resp.bytes().await?;
    let result: serde_json::Value = if body_bytes.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_slice(&body_bytes)?
    };
    let upserted = result.get("upserted").and_then(|v| v.as_u64()).unwrap_or(0);

    if upserted > 0 {
        tracing::info!("Cloud sync push: cloud accepted {} records", upserted);
    }

    // Update push timestamp
    update_push_state(state).await;

    Ok(())
}

pub(crate) async fn get_last_push_time(state: &Arc<AppState>) -> String {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT last_synced_at FROM sync_state WHERE table_name = '_push'",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    row.map(|r| r.0)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

pub(crate) async fn update_push_state(state: &Arc<AppState>) {
    let now = chrono::Utc::now().to_rfc3339();
    if let Err(e) = sqlx::query(
        "INSERT INTO sync_state (table_name, last_synced_at, last_sync_count, updated_at)
         VALUES ('_push', ?, 0, datetime('now'))
         ON CONFLICT(table_name) DO UPDATE SET
            last_synced_at = excluded.last_synced_at,
            updated_at = datetime('now')",
    )
    .bind(&now)
    .execute(&state.db)
    .await
    {
        tracing::error!("Cloud sync: failed to update push state: {}", e);
    }
}

pub(crate) async fn get_last_sync_time(state: &Arc<AppState>) -> String {
    // AUTH-03 fix: exclude _push sentinel row from MIN() — only track pull windows
    let row = match sqlx::query_as::<_, (String,)>(
        "SELECT MIN(last_synced_at) FROM sync_state WHERE table_name != '_push'",
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Cloud sync: failed to read last sync time: {}", e);
            return "1970-01-01T00:00:00Z".to_string();
        }
    };

    row.map(|r| r.0)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

pub(crate) async fn update_sync_state(state: &Arc<AppState>, synced_at: &str, count: u64) {
    for table in SYNC_TABLES.split(',') {
        if let Err(e) = sqlx::query(
            "INSERT INTO sync_state (table_name, last_synced_at, last_sync_count, updated_at)
             VALUES (?, ?, ?, datetime('now'))
             ON CONFLICT(table_name) DO UPDATE SET
                last_synced_at = excluded.last_synced_at,
                last_sync_count = excluded.last_sync_count,
                updated_at = datetime('now')",
        )
        .bind(table)
        .bind(synced_at)
        .bind(count as i64)
        .execute(&state.db)
        .await
        {
            tracing::error!("Cloud sync: failed to update sync state for '{}': {}", table, e);
        }
    }
}

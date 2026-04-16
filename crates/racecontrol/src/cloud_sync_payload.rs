//! Cloud sync push payload collection — builds the JSON payload of venue deltas.
//! Extracted from cloud_sync_push.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use serde_json::Value;

use crate::cloud_sync::normalize_timestamp;
use crate::cloud_sync_push::{get_last_push_time, SCHEMA_VERSION};
use crate::state::AppState;

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
            'valid', valid, 'created_at', created_at,
            'car_class', car_class, 'suspect', COALESCE(suspect, 0),
            'session_type', COALESCE(session_type, 'practice'),
            'assist_config_hash', assist_config_hash,
            'assist_tier', COALESCE(assist_tier, 'unknown'),
            'billing_session_id', billing_session_id,
            'validity', COALESCE(validity, 'valid'),
            'venue_id', venue_id
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
            'track', track, 'car', car, 'sim_type', sim_type,
            'driver_id', driver_id,
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

    if let Ok(rows) = drift_events
        && !rows.is_empty() {
            let items: Vec<serde_json::Value> = rows.iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            tracing::info!("Cloud sync push: {} content_drift_events", items.len());
            payload["content_drift_events"] = serde_json::json!(items);
            has_data = true;
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

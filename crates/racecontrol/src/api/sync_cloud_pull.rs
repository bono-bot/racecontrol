#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Cloud Sync Pull (GET /sync/changes) ───────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SyncChangesQuery {
    since: Option<String>,
    tables: Option<String>,
    limit: Option<i64>,
}

pub(crate) async fn sync_changes(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SyncChangesQuery>,
) -> Json<Value> {
    // Require terminal secret for sync endpoint (exposes customer PII)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(serde_json::json!({ "error": "Unauthorized" }));
        }
    }

    // HMAC-SHA256 verification on GET -- permissive mode (AUTH-07)
    // TODO: Switch to strict mode after Bono deploys matching HMAC key
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let sig = headers.get("x-sync-signature").and_then(|v| v.to_str().ok());
        let ts = headers.get("x-sync-timestamp").and_then(|v| v.to_str().ok());
        let nonce = headers.get("x-sync-nonce").and_then(|v| v.to_str().ok());

        match (sig, ts, nonce) {
            (Some(sig), Some(ts_str), Some(nonce)) => {
                if let Ok(timestamp) = ts_str.parse::<i64>() {
                    // For GET requests, reconstruct query string as signed body
                    let since_val = params.since.as_deref().unwrap_or("1970-01-01T00:00:00Z");
                    let tables_val = params.tables.as_deref().unwrap_or("drivers,wallets,pricing_tiers,kiosk_experiences");
                    let query_body = format!("since={}&tables={}", since_val, tables_val);
                    if !crate::cloud_sync::verify_sync_signature(
                        query_body.as_bytes(), hmac_key.as_bytes(), timestamp, nonce, sig,
                    ) {
                        tracing::warn!(target: "sync", "HMAC verification failed on sync_changes (permissive -- allowing)");
                    }
                } else {
                    tracing::warn!(target: "sync", "Invalid x-sync-timestamp header on sync_changes");
                }
            }
            _ => {
                tracing::warn!(target: "sync", "HMAC headers missing on sync_changes (permissive -- allowing)");
            }
        }
    }

    // Normalize ISO timestamps (2026-03-07T23:48:38Z) to SQLite format (2026-03-07 23:48:38)
    // SQLite's datetime('now') uses space, but sync_state stores ISO with 'T'.
    // String comparison: space (0x20) < 'T' (0x54), so "2026-03-07 23:59" < "2026-03-07T00:00"
    // Without normalization, updated records are never returned after first sync cycle.
    let since = params
        .since
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
        .replace('T', " ")
        .trim_end_matches('Z')
        .trim_end_matches('+')
        .split('+')
        .next()
        .unwrap_or("1970-01-01 00:00:00")
        .to_string();
    let tables: Vec<&str> = params
        .tables
        .as_deref()
        .unwrap_or("drivers,wallets,pricing_tiers,kiosk_experiences")
        .split(',')
        .map(|s| s.trim())
        .collect();
    let limit = params.limit.unwrap_or(500);

    let mut result = json!({});

    for table in &tables {
        match *table {
            "drivers" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'customer_id', customer_id,
                        'name', name, 'email', email, 'phone', phone,
                        'steam_guid', steam_guid, 'iracing_id', iracing_id,
                        'avatar_url', avatar_url, 'total_laps', total_laps,
                        'total_time_ms', total_time_ms,
                        'has_used_trial', COALESCE(has_used_trial, 0),
                        'unlimited_trials', COALESCE(unlimited_trials, 0),
                        'pin_hash', pin_hash, 'phone_verified', COALESCE(phone_verified, 0),
                        'dob', dob, 'waiver_signed', COALESCE(waiver_signed, 0),
                        'waiver_signed_at', waiver_signed_at, 'waiver_version', waiver_version,
                        'guardian_name', guardian_name, 'guardian_phone', guardian_phone,
                        'registration_completed', COALESCE(registration_completed, 0),
                        'signature_data', signature_data,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM drivers
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["drivers"] = json!(items);
                }
            }
            "wallets" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'driver_id', w.driver_id, 'balance_paise', w.balance_paise,
                        'total_credited_paise', w.total_credited_paise,
                        'total_debited_paise', w.total_debited_paise,
                        'updated_at', w.updated_at,
                        'phone', d.phone, 'email', d.email
                    ) FROM wallets w
                    LEFT JOIN drivers d ON d.id = w.driver_id
                    WHERE w.updated_at > ?
                    ORDER BY w.updated_at ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["wallets"] = json!(items);
                }
            }
            "pricing_tiers" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'duration_minutes', duration_minutes,
                        'price_paise', price_paise, 'is_trial', is_trial,
                        'is_active', is_active, 'sort_order', sort_order,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM pricing_tiers
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["pricing_tiers"] = json!(items);
                }
            }
            "kiosk_experiences" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'game', game, 'track', track,
                        'car', car, 'car_class', car_class,
                        'duration_minutes', duration_minutes, 'start_type', start_type,
                        'ac_preset_id', ac_preset_id, 'sort_order', sort_order,
                        'is_active', is_active, 'pricing_tier_id', pricing_tier_id,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM kiosk_experiences
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["kiosk_experiences"] = json!(items);
                }
            }
            "pricing_rules" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'rule_name', rule_name, 'rule_type', rule_type,
                        'day_of_week', day_of_week, 'hour_start', hour_start,
                        'hour_end', hour_end, 'multiplier', multiplier,
                        'flat_adjustment_paise', flat_adjustment_paise,
                        'is_active', is_active
                    ) FROM pricing_rules",
                )
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["pricing_rules"] = json!(items);
                }
            }
            "kiosk_settings" => {
                // kiosk_settings is a key-value table, return as a flat object
                let rows = sqlx::query_as::<_, (String, String)>(
                    "SELECT key, value FROM kiosk_settings",
                )
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let mut settings = json!({});
                    for (key, value) in &rows {
                        settings[key] = json!(value);
                    }
                    result["kiosk_settings"] = settings;
                }
            }
            "auth_tokens" => {
                // Only sync pending/unexpired tokens — venue needs these for kiosk PIN validation
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'pod_id', pod_id, 'driver_id', driver_id,
                        'pricing_tier_id', pricing_tier_id, 'auth_type', auth_type,
                        'token', token, 'status', status,
                        'custom_price_paise', custom_price_paise,
                        'custom_duration_minutes', custom_duration_minutes,
                        'experience_id', experience_id,
                        'custom_launch_args', custom_launch_args,
                        'created_at', created_at, 'expires_at', expires_at
                    ) FROM auth_tokens
                    WHERE status = 'pending' AND expires_at > datetime('now')
                    ORDER BY created_at ASC
                    LIMIT ?",
                )
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["auth_tokens"] = json!(items);
                }
            }
            "reservations" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'driver_id', driver_id, 'experience_id', experience_id,
                        'pin', pin, 'status', status, 'pod_number', pod_number,
                        'debit_intent_id', debit_intent_id,
                        'created_at', created_at, 'expires_at', expires_at,
                        'redeemed_at', redeemed_at, 'cancelled_at', cancelled_at,
                        'updated_at', updated_at
                    ) FROM reservations
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["reservations"] = json!(items);
                }
            }
            "debit_intents" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
                        'reservation_id', reservation_id, 'status', status,
                        'failure_reason', failure_reason, 'wallet_txn_id', wallet_txn_id,
                        'origin', origin,
                        'created_at', created_at, 'processed_at', processed_at,
                        'updated_at', updated_at
                    ) FROM debit_intents
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["debit_intents"] = json!(items);
                }
            }
            "staff_members" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'phone', phone, 'pin', pin,
                        'is_active', is_active, 'role', COALESCE(role, 'staff'),
                        'created_at', created_at, 'updated_at', updated_at,
                        'last_login_at', last_login_at
                    ) FROM staff_members
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["staff_members"] = json!(items);
                }
            }
            // Phase 301: Cloud Data Sync v2 — intelligence tables (SYNC-04)
            "fleet_solutions" => {
                let rows = sqlx::query_as::<_, (String,)>(
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
                    ) FROM fleet_solutions WHERE updated_at > ? ORDER BY updated_at ASC LIMIT ?",
                )
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["fleet_solutions"] = json!(items);
                }
            }
            "model_evaluations" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'model_name', model_name, 'pod_id', pod_id,
                        'problem_key', problem_key, 'prediction', prediction, 'actual', actual,
                        'correct', correct, 'cost_usd', cost_usd, 'diagnosis_tier', diagnosis_tier,
                        'created_at', created_at, 'updated_at', updated_at, 'venue_id', venue_id
                    ) FROM model_evaluations WHERE updated_at > ? ORDER BY updated_at ASC LIMIT ?",
                )
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["model_evaluations"] = json!(items);
                }
            }
            "metrics_rollups" => {
                // Omit id (AUTOINCREMENT) — target DB assigns its own
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'resolution', resolution, 'metric_name', metric_name, 'pod_id', pod_id,
                        'min_value', min_value, 'max_value', max_value, 'avg_value', avg_value,
                        'sample_count', sample_count, 'period_start', period_start,
                        'updated_at', COALESCE(updated_at, datetime('now')), 'venue_id', venue_id
                    ) FROM metrics_rollups
                    WHERE COALESCE(updated_at, datetime('now')) > ?
                    ORDER BY COALESCE(updated_at, datetime('now')) ASC LIMIT ?",
                )
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["metrics_rollups"] = json!(items);
                }
            }
            _ => {}
        }
    }

    result["synced_at"] = json!(chrono::Utc::now().to_rfc3339());
    Json(result)
}

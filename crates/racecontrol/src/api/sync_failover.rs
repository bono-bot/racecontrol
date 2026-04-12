#![allow(unused_imports)]
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Failover Orchestration (Phase 69) ───────────────────────────────────

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct FailoverBroadcastRequest {
    target_url: String,
}

/// POST /api/v1/failover/broadcast
/// Body: { "target_url": "ws://100.70.177.44:8080/ws/agent" }
/// Auth: x-terminal-secret header (same as sync endpoints).
/// Iterates agent_senders and sends SwitchController to all connected pods.
pub(crate) async fn failover_broadcast(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<FailoverBroadcastRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // Auth: x-terminal-secret check (consistent with sync_push and other service routes)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized"})),
            )
                .into_response();
        }
    }

    let target_url = body.target_url;
    let agent_senders = state.agent_senders.read().await;
    let mut sent = 0usize;
    let total = agent_senders.len();

    for (pod_id, sender) in agent_senders.iter() {
        if sender
            .send(CoreMessage::wrap(rc_common::protocol::CoreToAgentMessage::SwitchController {
                target_url: target_url.clone(),
            }))
            .await
            .is_ok()
        {
            sent += 1;
            tracing::info!("[failover] SwitchController sent to pod {}", pod_id);
        } else {
            tracing::warn!("[failover] Failed to send SwitchController to pod {}", pod_id);
        }
    }

    tracing::info!(
        "[failover] Broadcast SwitchController to {}/{} agents, target: {}",
        sent,
        total,
        target_url
    );
    Json(serde_json::json!({ "ok": true, "sent": sent, "total": total })).into_response()
}

// ─── Failback Data Reconciliation (Phase 70) ─────────────────────────────

/// POST /api/v1/sync/import-sessions
/// Body: { "sessions": [ { ...billing_session fields... } ] }
/// Auth: x-terminal-secret header (same as sync_push).
/// Inserts cloud-created billing sessions that were created during failover.
/// Uses INSERT OR IGNORE so duplicate UUIDs are silently skipped.
pub(crate) async fn import_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Auth: x-terminal-secret check (consistent with sync_push pattern)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(json!({ "error": "Unauthorized" }));
        }
    }

    let sessions = match body.get("sessions").and_then(|v| v.as_array()) {
        Some(s) => s,
        None => return Json(json!({ "error": "missing sessions array" })),
    };

    let mut imported = 0u64;
    let mut skipped = 0u64;

    for s in sessions {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }

        let r = sqlx::query(
            "INSERT OR IGNORE INTO billing_sessions (
                id, driver_id, pod_id, pricing_tier_id,
                allocated_seconds, driving_seconds, status, custom_price_paise, notes,
                started_at, ended_at, created_at, experience_id, car, track, sim_type,
                split_count, split_duration_minutes,
                wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason,
                pause_count, total_paused_seconds, refund_paise)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)",
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
        .execute(&state.db)
        .await;

        match r {
            Ok(result) if result.rows_affected() > 0 => imported += 1,
            Ok(_) => skipped += 1,
            Err(e) => {
                tracing::warn!("[import_sessions] Failed to insert session {}: {}", id, e);
                skipped += 1;
            }
        }
    }

    Json(json!({
        "imported": imported,
        "skipped": skipped,
        "synced_at": chrono::Utc::now().to_rfc3339(),
    }))
}

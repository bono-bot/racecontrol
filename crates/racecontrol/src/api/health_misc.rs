#![allow(unused_imports)]
use super::routes::BUILD_ID;
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

pub(crate) async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Check Evolution API (WhatsApp) reachability — non-blocking, 2s timeout
    let whatsapp_status = check_evolution_health(&state).await;

    // Phase 352: Per-subsystem probes (cached, zero latency — reads from background task)
    let subsystems = crate::subsystem_health::get_current_status();

    // Derive overall status from subsystem probes (closes "probes that lie" standing rule)
    let all_ok = subsystems.is_empty() || subsystems.values().all(|s| s.ok);
    let overall_status = if all_ok { "ok" } else { "degraded" };

    Json(json!({
        "status": overall_status,
        "service": "racecontrol",
        "version": env!("CARGO_PKG_VERSION"),
        "build_id": BUILD_ID,
        "whatsapp": whatsapp_status,
        "deploy_context": "v34-v39 merged: metrics TSDB, config management, data durability, security hardening, meshed intelligence v2, session trace ID. Skip-once pod offline detection. Audit hash chain. 44-table venue_id migration.",
        "subsystems": subsystems,
    }))
}

// ─── Client error telemetry (MI integration) ────────────────────────────────
/// POST /api/v1/telemetry/client-error — receive error reports from kiosk/web browsers.
/// Public endpoint (no auth) — rate-limited by caller (browser-side).
/// Logs errors and increments API error counters for MI anomaly detection.
pub(crate) async fn client_error_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> axum::http::StatusCode {
    let page = body.get("page").and_then(|v| v.as_str()).unwrap_or("unknown");
    let endpoint = body.get("endpoint").and_then(|v| v.as_str()).unwrap_or("unknown");
    let status = body.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let error = body.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
    let source = body.get("source").and_then(|v| v.as_str()).unwrap_or("kiosk");

    tracing::warn!(
        target: "client-telemetry",
        "CLIENT_ERROR: {} on {} → {} (HTTP {}, error: {})",
        source, page, endpoint, status, error
    );

    // Feed into API error counter so detect_fleet_anomalies picks it up
    state.record_api_error(&format!("client/{}/{}", source, endpoint));

    axum::http::StatusCode::OK
}

// ─── Phase 300-02: Backup Status endpoint ────────────────────────────────────
/// GET /api/v1/backup/status — returns current BackupStatus as JSON.
///
/// Staff JWT required (registered in staff_routes). Backup health data should not
/// be visible to unauthenticated clients on venue WiFi.
pub(crate) async fn get_backup_status(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Snapshot without holding lock across .await (standing rule: no lock held across .await)
    let status = { state.backup_status.read().await.clone() };
    Json(status)
}

/// Probe Evolution API health. Returns "ok", "unreachable", or "not_configured".
pub(crate) async fn check_evolution_health(state: &Arc<AppState>) -> &'static str {
    let evo_url = match &state.config.auth.evolution_url {
        Some(u) => u.clone(),
        None => return "not_configured",
    };

    // Probe the base URL — Evolution API returns 200 on GET /
    // 5s timeout: external hostname DNS resolution can exceed 2s from venue server
    match state.http_client
        .get(&evo_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => "ok",
        Ok(_) => "ok", // Any HTTP response means Evolution is reachable
        Err(_) => "unreachable",
    }
}

/// GET /api/v1/debug/db-stats — AI debugger database statistics (public, no auth).
/// Returns counts for ai_suggestions, ai_training_pairs, and recent entries.
pub(crate) async fn debug_db_stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    let db = &state.db;

    let suggestion_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_suggestions")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let active_suggestions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_suggestions WHERE dismissed = 0",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let training_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_training_pairs")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    // Recent suggestions (last 10)
    let recent: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String, String, String, i32, String)>(
        "SELECT id, pod_id, sim_type, source, model, dismissed, created_at FROM ai_suggestions ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, pod_id, sim_type, source, model, dismissed, created_at)| {
        json!({
            "id": id,
            "pod_id": pod_id,
            "sim_type": sim_type,
            "source": source,
            "model": model,
            "dismissed": dismissed != 0,
            "created_at": created_at,
        })
    })
    .collect();

    Json(json!({
        "ai_suggestions": {
            "total": suggestion_count,
            "active": active_suggestions,
            "dismissed": suggestion_count - active_suggestions,
        },
        "ai_training_pairs": {
            "total": training_count,
        },
        "recent_suggestions": recent,
    }))
}

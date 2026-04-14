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

// ─── Phase 367: Staff Tools handlers ─────────────────────────────────────────

/// GET /api/v1/admin/suspect-sessions?page=0&limit=50&from=YYYY-MM-DD&to=YYYY-MM-DD
///
/// Lists billing_sessions with suspect=1, paginated, optional date filter.
/// Requires manager role (RBAC SEC-04). Phase 367-01 (GLD-G-01).
pub(crate) async fn list_suspect_sessions_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let page: i64 = params.get("page").and_then(|v| v.parse().ok()).unwrap_or(0);
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50).min(200);
    let offset = page * limit;

    let from_filter = params.get("from").cloned().unwrap_or_default();
    let to_filter = params.get("to").cloned().unwrap_or_default();

    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>, Option<i64>, Option<i64>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id,
                d.name AS driver_name,
                bs.driver_id,
                bs.pod_id,
                bs.suspect_reasons,
                bs.telemetry_coverage_pct,
                bs.lap_count_actual,
                bs.lap_count_expected,
                bs.lap_count_flag,
                bs.started_at,
                bs.ended_at
         FROM billing_sessions bs
         LEFT JOIN drivers d ON d.id = bs.driver_id
         WHERE bs.suspect = 1
           AND (? = '' OR DATE(bs.ended_at) >= ?)
           AND (? = '' OR DATE(bs.ended_at) <= ?)
         ORDER BY bs.ended_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(&from_filter)
    .bind(&from_filter)
    .bind(&to_filter)
    .bind(&to_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE suspect = 1"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let sessions: Vec<Value> = rows.into_iter().map(|(id, driver_name, driver_id, pod_id, suspect_reasons, coverage_pct, lap_actual, lap_expected, lap_flag, started_at, ended_at)| {
        json!({
            "session_id": id,
            "driver_name": driver_name,
            "driver_id": driver_id,
            "pod_id": pod_id,
            "suspect_reasons": suspect_reasons.as_deref().and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()).unwrap_or_default(),
            "telemetry_coverage_pct": coverage_pct,
            "lap_count_actual": lap_actual,
            "lap_count_expected": lap_expected,
            "lap_count_flag": lap_flag,
            "started_at": started_at,
            "ended_at": ended_at
        })
    }).collect();

    Json(json!({ "sessions": sessions, "total": total, "page": page, "limit": limit }))
}

/// GET /api/v1/admin/sessions/{id}/telemetry-heatmap
///
/// Returns per-lap telemetry sample counts for a billing session.
/// Used to render the heatmap drill-down in the suspect sessions admin page.
/// Requires manager role. Phase 367-01 (GLD-G-01).
pub(crate) async fn session_telemetry_heatmap_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let laps = sqlx::query_as::<_, (String, i64, i64, i64, Option<i64>)>(
        "SELECT id, lap_number, lap_time_ms, valid, suspect
         FROM laps
         WHERE billing_session_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if laps.is_empty() {
        return Json(json!({ "session_id": session_id, "laps": [], "error": null }));
    }

    let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);

    let mut lap_data = Vec::new();
    for (lap_id, lap_number, lap_time_ms, valid, suspect) in &laps {
        let sample_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM telemetry_samples WHERE lap_id = ?",
        )
        .bind(lap_id)
        .fetch_one(telem_pool)
        .await
        .unwrap_or(0);

        lap_data.push(json!({
            "lap_id": lap_id,
            "lap_number": lap_number,
            "lap_time_ms": lap_time_ms,
            "valid": valid == &1,
            "suspect": suspect.unwrap_or(0) == 1,
            "sample_count": sample_count,
        }));
    }

    Json(json!({
        "session_id": session_id,
        "lap_count": laps.len(),
        "laps": lap_data
    }))
}

/// POST /api/v1/admin/pods/{pod_id}/verify
///
/// On-demand synthetic verification of a pod's connectivity and config-mismatch verifier status.
/// Returns pass/fail within 15 seconds. Requires manager role. Phase 367-02 (GLD-G-02).
pub(crate) async fn admin_verify_pod_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let start = tokio::time::Instant::now();

    let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        // Check 1: Pod WS connected (agent_senders registry)
        let ws_connected = {
            let senders = state.agent_senders.read().await;
            senders.contains_key(&pod_id)
        };

        // Check 2: Last config mismatch check timestamp
        let last_check: Option<String> = sqlx::query_scalar(
            "SELECT MAX(detected_at) FROM config_mismatches WHERE pod_id = ?",
        )
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let detail = if ws_connected {
            format!("Pod {} WS connected and active", pod_id)
        } else {
            format!("Pod {} WS disconnected -- check pod health", pod_id)
        };

        (ws_connected, detail, last_check)
    })
    .await;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok((pass, detail, last_mismatch_check)) => Json(json!({
            "pod_id": pod_id,
            "pass": pass,
            "detail": detail,
            "last_mismatch_check": last_mismatch_check,
            "last_seen": null,
            "elapsed_ms": elapsed_ms
        })),
        Err(_) => Json(json!({
            "pod_id": pod_id,
            "pass": false,
            "detail": "Timeout after 15s",
            "elapsed_ms": 15000
        })),
    }
}

/// POST /api/v1/internal/test/config-mismatch
///
/// Superadmin-only synthetic test endpoint for GLD-G-05 retro-validation.
/// Fires a WhatsApp alert and persists a test record to config_mismatches.
/// Used to verify the Phase 362 alert path end-to-end without a real pod.
/// Phase 367-05 (GLD-G-05).
pub(crate) async fn internal_test_config_mismatch_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let pod_id = req.get("pod_id")
        .and_then(|v| v.as_str())
        .unwrap_or("test-pod")
        .to_string();
    let detail = req.get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("GLD-G-05 retro-validation test")
        .to_string();

    let alert_msg = format!(
        "TEST CONFIG MISMATCH: Pod: {} | {} | This is a GLD-G-05 retro-validation test. No real mismatch occurred.",
        pod_id, detail
    );

    // Fire WhatsApp alert -- same path as the real mismatch alert
    whatsapp_alerter::send_admin_alert(&state.config, "ConfigMismatchTest", &alert_msg).await;

    // Persist to config_mismatches for audit trail
    let event_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Ensure config_mismatches table exists (idempotent)
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS config_mismatches (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            mismatched_fields TEXT,
            detected_at TEXT NOT NULL
        )"
    )
    .execute(&state.db)
    .await;

    let _ = sqlx::query(
        "INSERT OR IGNORE INTO config_mismatches (id, pod_id, sim_type, mismatched_fields, detected_at)
         VALUES (?, ?, 'TestSim', '[\"test_field\"]', ?)"
    )
    .bind(&event_id)
    .bind(&pod_id)
    .bind(&now)
    .execute(&state.db)
    .await;

    tracing::info!("GLD-G-05 test mismatch fired: pod={} event_id={}", pod_id, event_id);

    Json(json!({
        "ok": true,
        "event_id": event_id,
        "pod_id": pod_id,
        "message": alert_msg,
        "note": "WhatsApp alert fired. Verify receipt on staff phone within 30s."
    }))
}

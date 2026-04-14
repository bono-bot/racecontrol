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

// ── Deploy endpoints ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DeployRequest {
    binary_url: String,
    /// DEPLOY-03: Set to true to override weekend peak-hour deploy lock.
    #[serde(default)]
    force: bool,
}

/// POST /api/deploy/:pod_id — Deploy rc-agent binary to a single pod.
/// Returns 202 Accepted immediately; deploy runs as background task.
/// Returns 409 Conflict if deploy is already in progress or pod has active billing.
/// Returns 404 if pod not found.
/// Returns 423 Locked if weekend peak hours and force=false (DEPLOY-03).
pub(crate) async fn deploy_single_pod(
    Path(pod_id): Path<String>,
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<DeployRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(req.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Check pod exists and get IP
    let pod_ip = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.ip_address.clone())
    };

    let pod_ip = match pod_ip {
        Some(ip) => ip,
        None => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Pod not found", "pod_id": pod_id })),
            );
        }
    };

    // Check for active billing session — cannot deploy to a pod mid-session
    let has_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(&pod_id);
    if has_billing {
        return (
            axum::http::StatusCode::CONFLICT,
            Json(json!({
                "error": "Pod has active billing session. Cannot deploy during active session.",
                "pod_id": pod_id
            })),
        );
    }

    // Check for concurrent deploy in progress
    {
        let deploy_states = state.pod_deploy_states.read().await;
        if let Some(ds) = deploy_states.get(&pod_id) {
            if ds.is_active() {
                return (
                    axum::http::StatusCode::CONFLICT,
                    Json(json!({
                        "error": "Deploy already in progress",
                        "pod_id": pod_id,
                        "current_state": format!("{:?}", ds)
                    })),
                );
            }
        }
    }

    // Spawn deploy as background task (non-blocking)
    let deploy_state = Arc::clone(&state);
    let deploy_pod_id = pod_id.clone();
    let deploy_binary_url = req.binary_url.clone();
    tokio::spawn(async move {
        crate::deploy::deploy_pod(deploy_state, deploy_pod_id, pod_ip, deploy_binary_url).await;
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({
            "status": "deploy_started",
            "pod_id": pod_id,
            "binary_url": req.binary_url
        })),
    )
}

/// GET /api/deploy/status — Get deploy state for all pods.
pub(crate) async fn deploy_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let deploy_states = state.pod_deploy_states.read().await;
    let statuses: Vec<Value> = deploy_states
        .iter()
        .map(|(pod_id, ds)| {
            json!({
                "pod_id": pod_id,
                "state": ds,
            })
        })
        .collect();
    Json(json!({ "pods": statuses }))
}

/// POST /api/deploy/rolling — Start a canary-first rolling deploy to all pods.
/// Returns 202 Accepted immediately; rolling deploy runs as background task.
/// Returns 409 Conflict if any deploy is already active.
/// Returns 423 Locked if weekend peak hours and force=false (DEPLOY-03).
///
/// Body: { "binary_url": "http://192.168.31.27:9998/rc-agent.exe", "force": false }
pub(crate) async fn deploy_rolling_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<DeployRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(req.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Reject if any deploy is already in progress (guards against double-trigger)
    {
        let deploy_states = state.pod_deploy_states.read().await;
        let any_active = deploy_states
            .values()
            .any(|s| s.is_active());
        if any_active {
            return (
                axum::http::StatusCode::CONFLICT,
                Json(json!({
                    "error": "A deploy is already in progress on one or more pods",
                    "hint": "Check GET /api/deploy/status for current state"
                })),
            );
        }
    }

    let state_clone = Arc::clone(&state);
    let binary_url = req.binary_url.clone();
    let force = req.force;
    let actor = claims.sub.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::deploy::deploy_rolling(state_clone, binary_url, force, &actor).await {
            tracing::error!("Rolling deploy failed: {}", e);
        }
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({
            "status": "rolling_deploy_started",
            "canary": "pod_8",
            "binary_url": req.binary_url,
            "force_override": req.force,
        })),
    )
}

/// Phase 304 DEPLOY-01: Initiate fleet deploy with canary-first safety gates.
/// Returns 202 Accepted with deploy_id. Background task handles orchestration.
/// Returns 409 Conflict if a deploy is already Running or Pending.
/// Returns 423 Locked if weekend peak hours and force=false.
pub(crate) async fn fleet_deploy_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<crate::fleet_deploy::FleetDeployRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(req.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Check-and-set fleet_deploy_session atomically (prevents double-trigger — Pitfall 4)
    let session_lock = std::sync::Arc::clone(&state.fleet_deploy_session);
    let deploy_id = {
        let mut guard = session_lock.write().await;
        if let Some(ref existing) = *guard {
            if existing.overall_status == crate::fleet_deploy::DeployOverallStatus::Running
                || existing.overall_status == crate::fleet_deploy::DeployOverallStatus::Pending
            {
                return (
                    axum::http::StatusCode::CONFLICT,
                    Json(json!({
                        "error": "Fleet deploy already in progress",
                        "deploy_id": existing.deploy_id
                    })),
                );
            }
        }
        // Create new session and store it
        let new_session = crate::fleet_deploy::create_session(&req, &claims.sub);
        let id = new_session.deploy_id.clone();
        *guard = Some(new_session);
        id
    }; // write guard dropped here — safe to .await below

    // Spawn background orchestration task
    let state_clone = std::sync::Arc::clone(&state);
    let session_lock_clone = std::sync::Arc::clone(&state.fleet_deploy_session);
    tokio::spawn(async move {
        crate::fleet_deploy::run_fleet_deploy(state_clone, session_lock_clone).await;
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({
            "status": "started",
            "deploy_id": deploy_id,
            "canary": "pod_8"
        })),
    )
}

/// Phase 304 DEPLOY-05: Fleet deploy status with per-pod results and rollback log.
/// Returns the active or most-recently-completed session, or "idle" if none.
pub(crate) async fn fleet_deploy_status_handler(
    State(state): State<Arc<AppState>>,
    _claims: axum::Extension<crate::auth::middleware::StaffClaims>,
) -> Json<Value> {
    let guard = state.fleet_deploy_session.read().await;
    match &*guard {
        Some(session) => Json(
            serde_json::to_value(session)
                .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
        ),
        None => Json(json!({
            "status": "idle",
            "message": "No fleet deploy in progress or completed"
        })),
    }
}

// ─── OTA Pipeline (v22.0 Phase 179) ──────────────────────────────────────────

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub(crate) struct OtaDeployQuery {
    /// DEPLOY-03: Set to true to override weekend peak-hour deploy lock.
    #[serde(default)]
    force: bool,
}

/// POST /api/v1/ota/deploy — Start an OTA pipeline deploy with a TOML manifest.
/// Returns 202 Accepted; pipeline runs as background task.
/// Returns 409 if a pipeline is already running.
/// Returns 423 Locked if weekend peak hours and force=false (DEPLOY-03).
///
/// Query params: ?force=true to override weekend peak-hour lock.
pub(crate) async fn ota_deploy_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    axum::extract::Query(query): axum::extract::Query<OtaDeployQuery>,
    body: String,
) -> impl IntoResponse {
    use crate::ota_pipeline;

    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(query.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Parse manifest from TOML body
    let manifest = match ota_pipeline::parse_manifest(&body) {
        Ok(m) => m,
        Err(e) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": e })),
            );
        }
    };

    // Check if a pipeline is already running
    if let Some(record) = ota_pipeline::load_pipeline_state() {
        if !record.state.is_terminal() {
            return (
                axum::http::StatusCode::CONFLICT,
                Json(json!({
                    "error": "Pipeline already running",
                    "state": format!("{:?}", record.state),
                })),
            );
        }
    }

    // Phase 307 AUDIT-03: Log deploy initiation for hash chain coverage
    let deploy_version = manifest.release.version.clone();
    let deploy_actor = claims.sub.clone();
    crate::activity_log::log_pod_activity(
        &state,
        "server",
        "deploy",
        "OTA Deploy Initiated",
        &format!("version={} by={}", deploy_version, deploy_actor),
        "staff",
        None,
    );

    // Spawn pipeline as background task
    let version = manifest.release.version.clone();
    tokio::spawn(async move {
        tracing::info!("OTA pipeline started for version {}", version);
        // Pipeline orchestration will be wired here in future iteration
        // For now: persist initial state
        let record = ota_pipeline::DeployRecord::new(&version);
        if let Err(e) = ota_pipeline::persist_pipeline_state(&record) {
            tracing::error!("Failed to persist initial pipeline state: {e}");
        }
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({ "status": "pipeline_started" })),
    )
}

/// GET /api/v1/ota/status — Current OTA pipeline state.
pub(crate) async fn ota_status_handler() -> impl IntoResponse {
    use crate::ota_pipeline;

    match ota_pipeline::load_pipeline_state() {
        Some(record) => match serde_json::to_value(&record) {
            Ok(json) => (axum::http::StatusCode::OK, Json(json)),
            Err(e) => {
                tracing::warn!("Failed to serialize pipeline state: {e}");
                (
                    axum::http::StatusCode::OK,
                    Json(json!({ "state": "error", "message": format!("Serialization error: {e}") })),
                )
            }
        },
        None => (
            axum::http::StatusCode::OK,
            Json(json!({ "state": "idle", "message": "No pipeline state" })),
        ),
    }
}

// ─── Watchdog ────────────────────────────────────────────────────────────────

pub(crate) async fn watchdog_crash_report(
    Path(pod_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(report): Json<WatchdogCrashReport>,
) -> axum::http::StatusCode {
    tracing::warn!(
        pod_id = %pod_id,
        exit_code = ?report.exit_code,
        restart_count = report.restart_count,
        crash_time = %report.crash_time,
        watchdog_version = %report.watchdog_version,
        "Watchdog crash report: rc-agent restarted on {}",
        pod_id
    );

    crate::activity_log::log_pod_activity(
        &state,
        &pod_id,
        "system",
        "Watchdog Crash Report",
        &format!(
            "exit_code={:?} restart_count={} crash_time={} watchdog_version={}",
            report.exit_code, report.restart_count, report.crash_time, report.watchdog_version
        ),
        "watchdog",
        None,
    );

    axum::http::StatusCode::OK
}

// Deploy audit log types/handlers (CreateDeployLog, list_deploy_logs,
// DeployLogRow, get_app_health) are in deploy_audit.rs

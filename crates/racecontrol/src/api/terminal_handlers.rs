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

// ─── Terminal (remote command execution) ─────────────────────────────────────

pub(crate) async fn check_terminal_auth(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), String> {
    // 1. Check PIN session token (x-terminal-session header)
    if let Some(token) = headers.get("x-terminal-session").and_then(|v| v.to_str().ok()) {
        let sessions = state.terminal_sessions.read().await;
        if let Some(expiry) = sessions.get(token) {
            if *expiry > chrono::Utc::now() {
                return Ok(());
            }
        }
    }

    // 2. Check legacy shared secret (x-terminal-secret header — for cloud polling)
    let secret = state.config.cloud.terminal_secret.as_deref();
    if let Some(secret) = secret {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided == Some(secret) {
            return Ok(());
        }
    }

    // 3. If no secret AND no pin configured, allow (local dev)
    if state.config.cloud.terminal_secret.is_none() && state.config.cloud.terminal_pin.is_none() {
        return Ok(());
    }

    Err("Unauthorized. Use POST /terminal/auth with your PIN.".to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct TerminalAuthRequest {
    pin: String,
}

/// POST /terminal/auth — authenticate with PIN, returns a 24h session token
pub(crate) async fn terminal_auth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TerminalAuthRequest>,
) -> Json<Value> {
    let expected = state.config.cloud.terminal_pin.as_deref();
    match expected {
        None => {
            return Json(json!({ "error": "Terminal PIN not configured on server." }));
        }
        Some(pin) => {
            if req.pin != pin {
                tracing::warn!("Terminal auth failed — wrong PIN");
                return Json(json!({ "error": "Invalid PIN." }));
            }
        }
    }

    // Generate session token valid for 24 hours
    let token = uuid::Uuid::new_v4().to_string();
    let expiry = chrono::Utc::now() + chrono::Duration::hours(24);

    // Clean up expired sessions while we're here
    let mut sessions = state.terminal_sessions.write().await;
    let now = chrono::Utc::now();
    sessions.retain(|_, exp| *exp > now);
    sessions.insert(token.clone(), expiry);
    drop(sessions);

    tracing::info!("Terminal session created (expires {})", expiry.format("%Y-%m-%d %H:%M UTC"));

    Json(json!({
        "session": token,
        "expires_at": expiry.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct TerminalSubmitRequest {
    cmd: String,
    timeout_ms: Option<i64>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct TerminalResultRequest {
    exit_code: Option<i64>,
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct TerminalListQuery {
    limit: Option<i64>,
}

pub(crate) async fn terminal_submit(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TerminalSubmitRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let timeout_ms = req.timeout_ms.unwrap_or(30000).min(120000);

    let result = sqlx::query(
        "INSERT INTO terminal_commands (id, cmd, status, timeout_ms) VALUES (?, ?, 'pending', ?)",
    )
    .bind(&id)
    .bind(&req.cmd)
    .bind(timeout_ms)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Terminal command queued: {} ({})", id, req.cmd);

            // Audit trail for terminal command (MEDIUM sensitivity)
            let cmd_truncated: String = req.cmd.chars().take(200).collect();
            accounting::log_admin_action(
                &state, "terminal_command",
                &json!({"command_id": id, "command": cmd_truncated}).to_string(),
                None, None,
            ).await;

            // Execute locally in background for instant results (no cloud poll delay)
            let exec_state = state.clone();
            let exec_id = id.clone();
            let exec_cmd = req.cmd.clone();
            let exec_timeout = timeout_ms as u64;
            tokio::spawn(async move {
                use tokio::time::{timeout, Duration};
                use tokio::process::Command;

                // Mark as running
                let _ = sqlx::query(
                    "UPDATE terminal_commands SET status = 'running', started_at = datetime('now') WHERE id = ? AND status = 'pending'",
                )
                .bind(&exec_id)
                .execute(&exec_state.db)
                .await;

                let max_output: usize = 100 * 1024;
                let result = timeout(Duration::from_millis(exec_timeout), async {
                    #[cfg(windows)]
                    { Command::new("cmd").args(["/C", &exec_cmd]).kill_on_drop(true).output().await }
                    #[cfg(not(windows))]
                    { Command::new("sh").args(["-c", &exec_cmd]).kill_on_drop(true).output().await }
                }).await;

                let (exit_code, stdout, stderr) = match result {
                    Ok(Ok(output)) => {
                        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if stdout.len() > max_output { stdout.truncate(max_output); stdout.push_str("\n... [truncated]"); }
                        if stderr.len() > max_output { stderr.truncate(max_output); stderr.push_str("\n... [truncated]"); }
                        (output.status.code(), stdout, stderr)
                    }
                    Ok(Err(e)) => (None, String::new(), format!("Failed to execute: {}", e)),
                    Err(_) => (Some(124), String::new(), format!("Timed out after {}ms", exec_timeout)),
                };

                let _ = sqlx::query(
                    "UPDATE terminal_commands SET status = 'completed', exit_code = ?, stdout = ?, stderr = ?, completed_at = datetime('now') WHERE id = ?",
                )
                .bind(exit_code)
                .bind(&stdout)
                .bind(&stderr)
                .bind(&exec_id)
                .execute(&exec_state.db)
                .await;

                tracing::info!("Terminal command {} executed locally (exit: {:?})", exec_id, exit_code);
            });

            Json(json!({ "status": "queued", "id": id }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn terminal_list(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<TerminalListQuery>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let limit = params.limit.unwrap_or(50).min(200);

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'cmd', cmd, 'status', status,
            'exit_code', exit_code, 'stdout', stdout, 'stderr', stderr,
            'timeout_ms', timeout_ms,
            'created_at', created_at, 'started_at', started_at, 'completed_at', completed_at
        ) FROM terminal_commands
        ORDER BY created_at DESC
        LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let commands: Vec<Value> = rows
                .iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            Json(json!({ "commands": commands }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn terminal_pending(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'cmd', cmd, 'timeout_ms', timeout_ms, 'created_at', created_at
        ) FROM terminal_commands
        WHERE status = 'pending'
        ORDER BY created_at ASC
        LIMIT 10",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let commands: Vec<Value> = rows
                .iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            Json(json!({ "commands": commands }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn terminal_result(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TerminalResultRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status = if req.exit_code == Some(124) { "timeout" }
        else if req.exit_code.is_some() && req.exit_code != Some(0) { "failed" }
        else { "completed" };

    let result = sqlx::query(
        "UPDATE terminal_commands SET
            status = ?, exit_code = ?, stdout = ?, stderr = ?, completed_at = datetime('now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(req.exit_code)
    .bind(&req.stdout)
    .bind(&req.stderr)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Terminal command {} completed ({})", id, status);
            Json(json!({ "status": "ok" }))
        }
        Ok(_) => Json(json!({ "error": "Command not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Terminal Multiplayer ─────────────────────────────────────────────────────

/// POST /terminal/book-multiplayer — Staff-initiated multiplayer booking (skips friendship checks)
pub(crate) async fn terminal_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let driver_ids: Vec<String> = match req.get("driver_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'driver_ids' array" })),
    };

    let pod_ids: Vec<String> = match req.get("pod_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'pod_ids' array" })),
    };

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pricing_tier_id'" })),
    };

    let experience_id = req.get("experience_id").and_then(|v| v.as_str());
    let game = req.get("game").and_then(|v| v.as_str());
    let track = req.get("track").and_then(|v| v.as_str());
    let car = req.get("car").and_then(|v| v.as_str());

    match multiplayer::staff_book_multiplayer(
        &state,
        driver_ids,
        pod_ids,
        experience_id,
        &pricing_tier_id,
        game,
        track,
        car,
    )
    .await
    {
        Ok(info) => Json(json!({ "status": "ok", "group_session": info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// GET /terminal/group-sessions — List recent group sessions for POS dashboard
pub(crate) async fn terminal_group_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let sessions = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
        "SELECT gs.id, gs.host_driver_id, gs.status, gs.shared_pin,
                COALESCE(ke.name, 'Unknown'), gs.total_members, gs.validated_count,
                gs.created_at
         FROM group_sessions gs
         LEFT JOIN kiosk_experiences ke ON ke.id = gs.experience_id
         ORDER BY gs.created_at DESC
         LIMIT 20",
    )
    .fetch_all(&state.db)
    .await;

    match sessions {
        Ok(rows) => {
            let mut sessions_json = Vec::new();
            for (id, host_id, status, pin, exp_name, total, validated, created) in &rows {
                let host_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
                    .bind(host_id)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "Unknown".to_string());

                // Get members
                let members = sqlx::query_as::<_, (String, String, String, Option<String>, Option<u32>)>(
                    "SELECT gsm.driver_id, COALESCE(d.name, 'Unknown'), gsm.status, gsm.pod_id,
                            (SELECT number FROM pods WHERE id = gsm.pod_id)
                     FROM group_session_members gsm
                     LEFT JOIN drivers d ON d.id = gsm.driver_id
                     WHERE gsm.group_session_id = ?
                     ORDER BY gsm.role DESC, gsm.invited_at",
                )
                .bind(id)
                .fetch_all(&state.db)
                .await
                .unwrap_or_default();

                let members_json: Vec<Value> = members
                    .iter()
                    .map(|(did, dname, mstatus, pod_id, pod_num)| {
                        json!({
                            "driver_id": did,
                            "driver_name": dname,
                            "status": mstatus,
                            "pod_id": pod_id,
                            "pod_number": pod_num,
                        })
                    })
                    .collect();

                sessions_json.push(json!({
                    "id": id,
                    "host_driver_id": host_id,
                    "host_name": host_name,
                    "status": status,
                    "shared_pin": pin,
                    "experience_name": exp_name,
                    "total_members": total,
                    "validated_count": validated,
                    "created_at": created,
                    "members": members_json,
                }));
            }
            Json(json!({ "group_sessions": sessions_json }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

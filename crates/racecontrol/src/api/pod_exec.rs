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

/// POST /pods/{id}/exec — Execute a command on a pod via WebSocket proxy.
/// Body: { "cmd": "...", "timeout_ms": 30000 }
/// Returns: { "success": bool, "stdout": "...", "stderr": "..." }
/// Works even when pod's HTTP :8090 is down — only requires WebSocket connection.
pub(crate) async fn ws_exec_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let cmd = match body["cmd"].as_str() {
        Some(c) => c,
        None => return Json(json!({ "error": "missing 'cmd' field" })),
    };
    let timeout_ms = body["timeout_ms"].as_u64().unwrap_or(30_000);

    // SEC-P0-10: Block dangerous command patterns (defense-in-depth)
    // MMA iter2-4: normalize aggressively before checking:
    //   1. Strip ^ (cmd.exe escape), collapse whitespace, lowercase
    //   2. Strip .exe/.com suffixes from binary names so sc.exe = sc
    //   3. Block dangerous BINARIES (not just command+args patterns)
    // MMA-R2-2 + ITER1-#1/#2/#3: Block env var expansion, FOR loops, -enc, ADS, substring
    let cmd_lower = cmd.to_lowercase();
    // Block env var patterns
    if cmd_lower.contains('%') || cmd_lower.contains("$env:") {
        let has_env_bypass = cmd_lower.contains("%comspec%")
            || cmd_lower.contains("%systemroot%")
            || cmd_lower.contains("%windir%")
            || cmd_lower.contains("%temp%")
            || cmd_lower.contains("$env:")
            // ITER1-#1: Block substring expansion %var:~0,3%
            || cmd_lower.contains(":~");
        if has_env_bypass {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked env var/substring expansion bypass");
            return Json(json!({ "error": "Command blocked: environment variable expansion not allowed" }));
        }
    }
    // ITER1-#1: Block FOR /F loops (cmd shell command injection)
    if cmd_lower.contains("for /f") || cmd_lower.contains("for /l") || cmd_lower.contains("for /d") || cmd_lower.contains("for /r") {
        tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked FOR loop command");
        return Json(json!({ "error": "Command blocked: FOR loops not allowed" }));
    }
    // ITER1-#2 + ITER2: Block PowerShell encoded commands (including partial params -e, -ec, -en)
    if cmd_lower.contains("-encodedcommand") || cmd_lower.contains("-enc ") || cmd_lower.contains("-en ") || cmd_lower.contains("-ec ") || cmd_lower.contains("-e ") && cmd_lower.contains("powershell") {
        tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked encoded command");
        return Json(json!({ "error": "Command blocked: encoded commands not allowed" }));
    }
    // ITER1-#3: Block Alternate Data Streams (file.exe:stream)
    // Allow legitimate colon uses (C:\, http:) but block exe:stream pattern
    {
        let stripped = cmd_lower.replace("c:\\", "").replace("d:\\", "").replace("http:", "").replace("https:", "");
        if stripped.contains(".exe:") || stripped.contains(".dll:") || stripped.contains(".bat:") {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked Alternate Data Stream");
            return Json(json!({ "error": "Command blocked: alternate data streams not allowed" }));
        }
    }
    // MMA-R2-2: Block UNC paths that could execute remote binaries
    if cmd_lower.contains("\\\\") {
        tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked UNC path execution");
        return Json(json!({ "error": "Command blocked: UNC paths not allowed" }));
    }
    let cmd_normalized: String = cmd
        .replace('^', "")
        .replace('\t', " ")
        .to_lowercase();
    let cmd_collapsed: String = cmd_normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    // Strip .exe/.com suffixes for binary-level blocking
    let cmd_no_exe: String = cmd_collapsed
        .replace(".exe", "")
        .replace(".com", "");

    // Blocked dangerous binaries (checked against .exe-stripped command)
    // MMA-ITER4: Extended LOLBin blocklist (3 models flagged gaps)
    const BLOCKED_BINARIES: &[&str] = &[
        "powershell", "pwsh", "mshta", "wscript", "cscript",
        "regsvr32", "rundll32", "msiexec", "odbcconf", "pcalua",
        "certutil", "bitsadmin", "bash", "wsl",
        // LOLBins added ITER4:
        "forfiles", "msdt", "hh", "infdefaultinstall", "diskshadow",
        "esentutl", "expand", "extrac32", "replace", "ieexec",
        "installutil", "msbuild", "msconfig", "msdeploy", "msxsl",
    ];
    for bin in BLOCKED_BINARIES {
        if cmd_no_exe.contains(bin) {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked binary: {}", bin);
            return Json(json!({ "error": format!("Command blocked: '{}' is not allowed", bin) }));
        }
    }

    // Blocked dangerous command patterns (checked against collapsed command)
    const BLOCKED_PATTERNS: &[&str] = &[
        "net user", "net localgroup", "net1 user", "net1 localgroup",
        "net use \\\\", "net start", "net stop",
        "reg add", "reg delete", "reg import", "reg load", "reg restore",
        "format c:", "rd /s /q c:", "del /s /q c:",
        "schtasks /create", "schtasks /change", "schtasks /delete",
        "sc create", "sc config", "sc stop", "sc delete",
        "netsh advfirewall", "netsh firewall",
        "wmic process call create", "wmic /node",
        "iex(", "invoke-expression", "invoke-webrequest",
        "downloadstring", "downloadfile", "new-object net.webclient",
    ];
    for pattern in BLOCKED_PATTERNS {
        if cmd_no_exe.contains(pattern) {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked pattern: {}", pattern);
            return Json(json!({ "error": format!("Command blocked: contains '{}'", pattern) }));
        }
    }

    // Truncate command preview to 100 chars for audit
    let cmd_preview: String = cmd.chars().take(100).collect();

    // Audit trail + WhatsApp alert for fleet exec (HIGH sensitivity)
    accounting::log_admin_action(
        &state, "fleet_exec",
        &json!({"pod_id": id, "command": cmd_preview}).to_string(),
        None, None,
    ).await;
    whatsapp_alerter::send_admin_alert(
        &state.config, "Fleet Exec",
        &format!("Pod {}: {}", id, cmd_preview),
    ).await;

    match crate::ws::ws_exec_on_pod(&state, &id, cmd, timeout_ms).await {
        Ok((success, stdout, stderr)) => {
            Json(json!({ "success": success, "stdout": stdout, "stderr": stderr }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

/// Phase 50: GET /pods/{id}/self-test — Trigger self-test on a pod via WS, return probe results + LLM verdict.
/// Timeout: 30s (probes run ~10s, LLM verdict adds ~5s).
pub(crate) async fn pod_self_test(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // 1. Get the WS sender for this pod (normalize to canonical pod_N format)
    let pod_id = normalize_pod_id(&pod_id).unwrap_or(pod_id);
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    let Some(sender) = sender else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": format!("pod {} not connected", pod_id)})),
        ).into_response();
    };

    // 2. Register a one-shot channel for the response
    let request_id = format!("selftest-{}", uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();
    {
        let mut pending = state.pending_self_tests.write().await;
        pending.insert(request_id.clone(), (pod_id.clone(), tx));
    }

    // 3. Send RunSelfTest command
    if sender.send(CoreMessage::wrap(CoreToAgentMessage::RunSelfTest { request_id: request_id.clone() })).await.is_err() {
        let mut pending = state.pending_self_tests.write().await;
        pending.remove(&request_id);
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "failed to send command to agent"})),
        ).into_response();
    }

    // 4. Await response with 30s timeout
    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(report)) => Json(report).into_response(),
        Ok(Err(_)) => {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "response channel dropped"}))).into_response()
        }
        Err(_) => {
            // Clean up timed-out entry
            let mut pending = state.pending_self_tests.write().await;
            pending.remove(&request_id);
            (axum::http::StatusCode::GATEWAY_TIMEOUT, Json(json!({"error": "self-test timed out after 30s"}))).into_response()
        }
    }
}

// POST /pods/{id}/clear-maintenance — Send ClearMaintenance to pod agent (STAFF-02)
//
// Clears the pod's maintenance state both on the server (optimistic) and by sending
// ClearMaintenance to the agent so it can re-run pre-flight checks on next session start.
pub(crate) async fn clear_maintenance_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Send ClearMaintenance via WS.
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&id).cloned()
    };
    match sender {
        Some(sender) => {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearMaintenance)).await;
        }
        None => {
            return Json(json!({ "error": format!("Pod {} not connected", id) }));
        }
    }

    // Also clear server-side maintenance state immediately (optimistic update).
    {
        let mut fleet = state.pod_fleet_health.write().await;
        if let Some(store) = fleet.get_mut(&id) {
            store.in_maintenance = false;
            store.maintenance_failures.clear();
        }
    }

    tracing::info!("ClearMaintenance sent to pod {} (STAFF-02)", id);
    crate::activity_log::log_pod_activity(&state, &id, "system", "Maintenance Cleared", "Staff cleared maintenance via dashboard", "staff", None);

    Json(json!({ "ok": true, "pod_id": id }))
}

// ─── v29.0 Phase 9: Maintenance & Analytics Handlers ────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct MaintenanceEventQuery {
    pod_id: Option<u8>,
    severity: Option<String>,
    hours: Option<u32>,
}

/// POST /api/v1/maintenance/events — Insert a MaintenanceEvent
pub(crate) async fn maintenance_create_event(
    State(state): State<Arc<AppState>>,
    Json(event): Json<crate::maintenance_models::MaintenanceEvent>,
) -> impl IntoResponse {
    match maintenance_store::insert_event(&state.db, &event).await {
        Ok(()) => Json(json!({ "ok": true, "id": event.id.to_string() })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// GET /api/v1/maintenance/events — Query events with filters (pod_id, severity, hours)
pub(crate) async fn maintenance_list_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MaintenanceEventQuery>,
) -> impl IntoResponse {
    let since = params.hours.map(|h| {
        chrono::Utc::now() - chrono::Duration::hours(h as i64)
    });
    let limit = 200u32;
    match maintenance_store::query_events(&state.db, params.pod_id, since, limit).await {
        Ok(events) => {
            // Optional severity filter (post-query since store doesn't support it directly)
            let filtered: Vec<_> = if let Some(ref sev) = params.severity {
                events.into_iter().filter(|e| {
                    let s = serde_json::to_string(&e.severity).unwrap_or_default().replace('"', "");
                    s.eq_ignore_ascii_case(sev)
                }).collect()
            } else {
                events
            };
            Json(json!({ "ok": true, "events": filtered, "count": filtered.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// GET /api/v1/maintenance/summary — Get MaintenanceSummary
pub(crate) async fn maintenance_summary(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match maintenance_store::get_summary(&state.db).await {
        Ok(summary) => Json(json!({ "ok": true, "summary": summary })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// POST /api/v1/maintenance/tasks — Create a maintenance task
pub(crate) async fn maintenance_create_task(
    State(state): State<Arc<AppState>>,
    Json(task): Json<crate::maintenance_models::MaintenanceTask>,
) -> impl IntoResponse {
    match maintenance_store::insert_task(&state.db, &task).await {
        Ok(()) => Json(json!({ "ok": true, "id": task.id.to_string() })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct MaintenanceTaskQuery {
    status: Option<String>,
    pod_id: Option<u8>,
}

/// GET /api/v1/maintenance/tasks — Query tasks (status, pod_id)
pub(crate) async fn maintenance_list_tasks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MaintenanceTaskQuery>,
) -> impl IntoResponse {
    let limit = 200u32;
    match maintenance_store::query_tasks(&state.db, params.status.as_deref(), limit).await {
        Ok(tasks) => {
            // Optional pod_id filter (post-query)
            let filtered: Vec<_> = if let Some(pid) = params.pod_id {
                tasks.into_iter().filter(|t| t.pod_id == Some(pid)).collect()
            } else {
                tasks
            };
            Json(json!({ "ok": true, "tasks": filtered, "count": filtered.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct TaskStatusUpdate {
    status: crate::maintenance_models::TaskStatus,
}

/// PATCH /api/v1/maintenance/tasks/:id — Update task status
pub(crate) async fn maintenance_update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TaskStatusUpdate>,
) -> impl IntoResponse {
    let task_id = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(json!({ "ok": false, "error": "Invalid UUID" })),
    };
    match maintenance_store::update_task_status(&state.db, task_id, &body.status).await {
        Ok(true) => Json(json!({ "ok": true })),
        Ok(false) => Json(json!({ "ok": false, "error": "Task not found" })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct TelemetryQuery {
    pod_id: Option<String>,
    hours: Option<u32>,
    limit: Option<u32>,
}

/// GET /api/v1/analytics/telemetry — Query hardware telemetry history
pub(crate) async fn analytics_telemetry(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TelemetryQuery>,
) -> impl IntoResponse {
    let pool = match &state.telemetry_db {
        Some(p) => p,
        None => return Json(json!({ "ok": false, "error": "Telemetry DB not initialized" })),
    };
    let hours = params.hours.unwrap_or(24);
    let limit = params.limit.unwrap_or(500).min(2000);
    let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours as i64)).to_rfc3339();

    let query = if let Some(ref pid) = params.pod_id {
        sqlx::query(
            "SELECT pod_id, collected_at, gpu_temp_celsius, cpu_temp_celsius, gpu_power_watts,
                    cpu_usage_pct, gpu_usage_pct, memory_usage_pct, disk_usage_pct,
                    network_latency_ms, process_handle_count, disk_smart_health_pct
             FROM hardware_telemetry
             WHERE collected_at > ?1 AND pod_id = ?2
             ORDER BY collected_at DESC
             LIMIT ?3"
        )
        .bind(&cutoff)
        .bind(pid)
        .bind(limit as i64)
    } else {
        sqlx::query(
            "SELECT pod_id, collected_at, gpu_temp_celsius, cpu_temp_celsius, gpu_power_watts,
                    cpu_usage_pct, gpu_usage_pct, memory_usage_pct, disk_usage_pct,
                    network_latency_ms, process_handle_count, disk_smart_health_pct
             FROM hardware_telemetry
             WHERE collected_at > ?1
             ORDER BY collected_at DESC
             LIMIT ?2"
        )
        .bind(&cutoff)
        .bind(limit as i64)
    };

    match query.fetch_all(pool).await {
        Ok(rows) => {
            use sqlx::Row;
            let data: Vec<Value> = rows.iter().map(|r| {
                json!({
                    "pod_id": r.get::<String, _>("pod_id"),
                    "collected_at": r.get::<String, _>("collected_at"),
                    "gpu_temp_celsius": r.get::<Option<f64>, _>("gpu_temp_celsius"),
                    "cpu_temp_celsius": r.get::<Option<f64>, _>("cpu_temp_celsius"),
                    "gpu_power_watts": r.get::<Option<f64>, _>("gpu_power_watts"),
                    "cpu_usage_pct": r.get::<Option<f64>, _>("cpu_usage_pct"),
                    "gpu_usage_pct": r.get::<Option<f64>, _>("gpu_usage_pct"),
                    "memory_usage_pct": r.get::<Option<f64>, _>("memory_usage_pct"),
                    "disk_usage_pct": r.get::<Option<f64>, _>("disk_usage_pct"),
                    "network_latency_ms": r.get::<Option<i64>, _>("network_latency_ms"),
                    "process_handle_count": r.get::<Option<i64>, _>("process_handle_count"),
                    "disk_smart_health_pct": r.get::<Option<i64>, _>("disk_smart_health_pct"),
                })
            }).collect();
            Json(json!({ "ok": true, "data": data, "count": data.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct TrendQuery {
    pod_id: String,
    metric: String,
    window_days: Option<u32>,
}

/// GET /api/v1/analytics/trends — Get metric trend for a pod
pub(crate) async fn analytics_trends(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TrendQuery>,
) -> impl IntoResponse {
    let pool = match &state.telemetry_db {
        Some(p) => p,
        None => return Json(json!({ "ok": false, "error": "Telemetry DB not initialized" })),
    };
    let window = params.window_days.unwrap_or(30);
    match crate::telemetry_store::get_metric_trend(pool, &params.pod_id, &params.metric, window).await {
        Ok(trend) => Json(json!({ "ok": true, "trend": trend })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

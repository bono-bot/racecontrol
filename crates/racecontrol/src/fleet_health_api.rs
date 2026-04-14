//! Fleet health API handlers — extracted from fleet_health.rs (Phase 385 ARCH-03).
//!
//! - `fleet_health_handler`: GET /api/v1/fleet/health
//! - `sentry_crash_handler`: POST /api/v1/sentry/crash
//! - `blocked_start_handler`: POST /api/v1/fleet/blocked-start
//! - `PodFleetStatus`: API response shape per pod

use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::fleet_health::FleetHealthStore;
use crate::state::AppState;

/// API response shape for a single pod in GET /api/v1/fleet/health.
#[derive(Debug, Clone, Serialize)]
pub struct PodFleetStatus {
    pub pod_number: u32,
    pub pod_id: Option<String>,
    /// Display name from DB (e.g. "Pod 1", "POS 1"). Used by status page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Node type: "pod" for racing simulators (1-8), "pos" for point-of-sale terminal (9).
    /// Used by frontends to filter POS out of racing pod views.
    pub node_type: String,
    pub ws_connected: bool,
    pub http_reachable: bool,
    pub version: Option<String>,
    /// Git commit hash from the running rc-agent binary's /health endpoint.
    /// null = old binary (pre-build-ID) or pod not yet probed.
    pub build_id: Option<String>,
    /// Live uptime in seconds, computed from `agent_started_at`. None if no StartupReport yet.
    pub uptime_secs: Option<i64>,
    pub crash_recovery: Option<bool>,
    pub ip_address: Option<String>,
    /// ISO-8601 timestamp of when the pod was last seen active.
    pub last_seen: Option<String>,
    /// ISO-8601 timestamp of the most recent HTTP probe attempt.
    pub last_http_check: Option<String>,
    /// Phase 100: True if the pod is in maintenance state (PreFlightFailed and not cleared).
    pub in_maintenance: bool,
    /// Phase 100: Check names from the most recent PreFlightFailed event.
    pub maintenance_failures: Vec<String>,
    /// Phase 104: Number of process violations in the last 24 hours.
    pub violation_count_24h: u32,
    /// Phase 104: ISO-8601 timestamp of most recent violation.
    pub last_violation_at: Option<String>,
    /// Phase 138: Consecutive idle health failures reported by this pod (0 = healthy).
    pub idle_health_fail_count: u32,
    /// Phase 138: Check names from most recent IdleHealthFailed.
    pub idle_health_failures: Vec<String>,
    /// Phase 206 (OBS-04): Currently active sentinel files on this pod.
    /// Empty if no sentinels are active. Populated from SentinelChange WS events.
    #[serde(default)]
    pub active_sentinels: Vec<String>,
    /// SHA256 of start-rcagent.bat on this pod. Used to detect bat drift.
    /// null = old agent without bat_sha256 or probe hasn't succeeded yet.
    pub bat_sha256: Option<String>,
    /// Phase 9b: True if the pod is crash-looping (>3 short-uptime restarts in 5 min).
    #[serde(default)]
    pub crash_loop: bool,
    /// RESIL-06: True if pod auto-flagged for maintenance (>3 crashes in 1 hour).
    #[serde(default)]
    pub maintenance_flag: bool,
    /// RESIL-06: Number of crashes recorded for this pod in the last hour.
    #[serde(default)]
    pub crashes_last_hour: i32,
    /// RESIL-08: Clock drift in seconds (server_time - agent_time) from last heartbeat.
    /// null = no heartbeat with agent_timestamp received yet.
    #[serde(default)]
    pub clock_drift_secs: Option<i64>,
    /// CX-06: Pod experience score (0-100) from experience_collector. Updated every 5 min.
    #[serde(default)]
    pub experience_score: Option<f64>,
    /// CX-06: "Healthy", "Maintenance", or "RemoveFromRotation"
    #[serde(default)]
    pub experience_status: Option<String>,
    /// Phase 284: Average ready_delay (duration_to_playable_ms) for this pod over last 7 days.
    pub avg_ready_delay_ms: Option<f64>,
    /// Phase 284: Number of crash recovery events for this pod in last 24 hours.
    #[serde(default)]
    pub crash_recovery_count: i64,
    /// Windows session ID: 0 = Session 0 (Services, GUI broken), 1+ = interactive (Console).
    /// null = old agent or no StartupReport received yet.
    pub windows_session_id: Option<u32>,
    /// WS reconnect count in the last 5 minutes.
    #[serde(default)]
    pub ws_reconnects_5m: u32,
    /// Total WS reconnect count since last server restart.
    #[serde(default)]
    pub ws_reconnect_count: u32,
    /// Whether freedom mode is active on this pod (lock screen dismissed, no restrictions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freedom_mode: Option<bool>,
    /// Whether the pod screen is blanked (black screen between sessions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_blanked: Option<bool>,
    /// Current game state: "idle", "loading", "running", "error", etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_state: Option<String>,
}

// ── GET /api/v1/fleet/health ──────────────────────────────────────────────────

/// RESIL-07: Fleet health response cache — avoids repeated DB queries + 4 lock reads.
/// Dashboard polls every 5s; cache ensures only 1 actual computation per 5s window.
static FLEET_HEALTH_CACHE: std::sync::LazyLock<tokio::sync::RwLock<(Option<Value>, std::time::Instant)>> =
    std::sync::LazyLock::new(|| tokio::sync::RwLock::new((None, std::time::Instant::now())));

const FLEET_HEALTH_CACHE_TTL_SECS: u64 = 5;

/// GET /api/v1/fleet/health handler.
///
/// Returns a JSON object with `pods` (9 entries sorted by pod_number 1–9,
/// pod 9 = POS) and `timestamp`.
/// No authentication required — designed for Uday's phone on the LAN.
///
/// Pods that have never sent a WS message still appear with
/// ws_connected=false, http_reachable=false, and all optional fields null.
pub async fn fleet_health_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // RESIL-07: Return cached response if fresh (< 5s old)
    {
        let cache = FLEET_HEALTH_CACHE.read().await;
        if let (Some(ref cached), ts) = *cache {
            if ts.elapsed().as_secs() < FLEET_HEALTH_CACHE_TTL_SECS {
                return Json(cached.clone());
            }
        }
    }

    // Bug #9: Acquire and release each lock sequentially to avoid holding 4 read locks.
    let pods_snapshot = { state.pods.read().await.clone() };
    let senders_snapshot: HashMap<String, bool> = {
        let senders = state.agent_senders.read().await;
        senders.iter().map(|(k, v)| (k.clone(), v.is_closed())).collect()
    };
    let fleet_snapshot = { state.pod_fleet_health.read().await.clone() };
    let violations_snapshot = { state.pod_violations.read().await.clone() };

    // Phase 284: Query avg ready_delay and crash_recovery_count per pod from DB.
    let ready_delay_rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT pod_id, AVG(CAST(duration_to_playable_ms AS REAL))
         FROM launch_events
         WHERE duration_to_playable_ms IS NOT NULL
           AND created_at >= datetime('now', '-7 days')
         GROUP BY pod_id",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let ready_delay_map: HashMap<String, f64> = ready_delay_rows.into_iter().collect();

    let crash_recovery_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT pod_id, COUNT(*)
         FROM launch_events
         WHERE outcome != '\"Success\"'
           AND error_taxonomy = 'CrashRecovery'
           AND created_at >= datetime('now', '-1 day')
         GROUP BY pod_id",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let crash_recovery_map: HashMap<String, i64> = crash_recovery_rows.into_iter().collect();

    // Include pods 1-8 + pod 9 (POS). Standing rule: never exclude POS from fleet view.
    // Pod 9 slot is empty if POS hasn't connected, just like any unregistered pod.
    let mut result: Vec<PodFleetStatus> = Vec::with_capacity(9);

    for pod_number in 1u32..=9 {
        // Find registered PodInfo for this slot (if any).
        let pod_info = pods_snapshot
            .values()
            .find(|p| p.number == pod_number);

        match pod_info {
            None => {
                // Pod slot not registered yet — return all-false defaults.
                let node_type = if pod_number >= 9 { "pos" } else { "pod" };
                result.push(PodFleetStatus {
                    pod_number,
                    pod_id: None,
                    name: None,
                    node_type: node_type.to_string(),
                    ws_connected: false,
                    http_reachable: false,
                    version: None,
                    build_id: None,
                    uptime_secs: None,
                    crash_recovery: None,
                    ip_address: None,
                    last_seen: None,
                    last_http_check: None,
                    active_sentinels: vec![],
                    in_maintenance: false,
                    maintenance_failures: vec![],
                    violation_count_24h: 0,
                    last_violation_at: None,
                    idle_health_fail_count: 0,
                    idle_health_failures: vec![],
                    bat_sha256: None,
                    crash_loop: false,
                    maintenance_flag: false,
                    crashes_last_hour: 0,
                    clock_drift_secs: None,
                    experience_score: None,
                    experience_status: None,
                    avg_ready_delay_ms: None,
                    crash_recovery_count: 0,
                    // Phase 318 (Rule 1 - Bug): missing field in None branch — pod not yet registered
                    windows_session_id: None,
                    ws_reconnects_5m: 0,
                    ws_reconnect_count: 0,
                    freedom_mode: None,
                    screen_blanked: None,
                    game_state: None,
                });
            }
            Some(info) => {
                let pod_id = &info.id;

                // WS connected = sender exists and channel is still open.
                let ws_connected = senders_snapshot
                    .get(pod_id)
                    .map(|closed| !closed)
                    .unwrap_or(false);

                // Fleet health store for version, uptime, http state.
                let store = fleet_snapshot.get(pod_id);

                let http_reachable = store.map(|s| s.http_reachable).unwrap_or(false);
                let version = store.and_then(|s| s.version.clone());
                let build_id = store.and_then(|s| s.build_id.clone());
                let crash_recovery = store.and_then(|s| s.crash_recovery);
                let last_http_check = store
                    .and_then(|s| s.last_http_check)
                    .map(|t| t.to_rfc3339());

                // Compute live uptime from agent_started_at.
                let uptime_secs = store
                    .and_then(|s| s.agent_started_at)
                    .map(|started| {
                        let secs = (Utc::now() - started).num_seconds();
                        secs.max(0)
                    });

                let last_seen = info
                    .last_seen
                    .map(|t| t.to_rfc3339());

                let in_maintenance = store.map(|s| s.in_maintenance).unwrap_or(false);
                let maintenance_failures = store.map(|s| s.maintenance_failures.clone()).unwrap_or_default();

                let vstore = violations_snapshot.get(pod_id.as_str());
                let now = Utc::now();
                let violation_count_24h = vstore.map(|vs| vs.violation_count_24h(now)).unwrap_or(0);
                let last_violation_at = vstore.and_then(|vs| vs.last_violation_at()).map(String::from);

                let idle_health_fail_count = store.map(|s| s.idle_health_fail_count).unwrap_or(0);
                let idle_health_failures = store.map(|s| s.idle_health_failures.clone()).unwrap_or_default();
                let active_sentinels = store.map(|s| s.active_sentinels.clone()).unwrap_or_default();
                let bat_sha256 = store.and_then(|s| s.bat_sha256.clone());
                let crash_loop = store.map(|s| s.crash_loop).unwrap_or(false);
                let maintenance_flag = store.map(|s| s.maintenance_flag).unwrap_or(false);
                let crashes_last_hour = store.map(|s| s.crashes_last_hour).unwrap_or(0);
                let clock_drift_secs = store.and_then(|s| s.clock_drift_secs);
                let avg_ready_delay_ms = ready_delay_map.get(pod_id).copied();
                let crash_recovery_count = crash_recovery_map.get(pod_id).copied().unwrap_or(0);
                let windows_session_id = store.and_then(|s| s.windows_session_id);

                let five_min_ago = Utc::now() - chrono::Duration::seconds(300);
                let ws_reconnects_5m = store
                    .map(|s| s.ws_reconnect_times.iter().filter(|t| **t > five_min_ago).count() as u32)
                    .unwrap_or(0);
                let ws_reconnect_count = store.map(|s| s.ws_reconnect_count).unwrap_or(0);

                let node_type = if pod_number >= 9 { "pos" } else { "pod" };
                result.push(PodFleetStatus {
                    pod_number,
                    pod_id: Some(pod_id.clone()),
                    name: Some(info.name.clone()),
                    node_type: node_type.to_string(),
                    ws_connected,
                    http_reachable,
                    version,
                    build_id,
                    uptime_secs,
                    crash_recovery,
                    ip_address: Some(info.ip_address.clone()),
                    last_seen,
                    last_http_check,
                    in_maintenance,
                    maintenance_failures,
                    violation_count_24h,
                    last_violation_at,
                    idle_health_fail_count,
                    idle_health_failures,
                    active_sentinels,
                    bat_sha256,
                    crash_loop,
                    maintenance_flag,
                    crashes_last_hour,
                    clock_drift_secs,
                    experience_score: store.and_then(|s| s.experience_score),
                    experience_status: store.and_then(|s| s.experience_status.clone()),
                    avg_ready_delay_ms,
                    crash_recovery_count,
                    windows_session_id,
                    ws_reconnects_5m,
                    ws_reconnect_count,
                    freedom_mode: info.freedom_mode,
                    screen_blanked: info.screen_blanked,
                    game_state: info.game_state.as_ref().map(|g| format!("{:?}", g).to_lowercase()),
                });
            }
        }
    }

    // Read services health from app_health_monitor (30s probe cycle, WhatsApp alerts, DB logging).
    // Single source of truth — no duplicate probing.
    let services = {
        let entries = crate::app_health_monitor::get_current_health().await;
        let mut m = serde_json::Map::new();
        if entries.is_empty() {
            // Monitor hasn't run first cycle yet — report "pending" not "down".
            for name in &["kiosk", "web", "admin"] {
                m.insert(name.to_string(), json!("pending"));
            }
        } else {
            for entry in &entries {
                m.insert(entry.app.clone(), json!({
                    "status": entry.status,
                    "response_ms": entry.response_ms,
                    "last_checked": entry.last_checked,
                }));
            }
        }
        Value::Object(m)
    };

    // Phase 255: Display machine heartbeat status
    let display_status: Vec<Value> = {
        let heartbeats = state.display_heartbeats.read().await;
        let now = std::time::Instant::now();
        heartbeats.iter().map(|(id, (last_ping, uptime_s))| {
            let elapsed_secs = now.duration_since(*last_ping).as_secs();
            let online = elapsed_secs < 120; // 2 minute threshold
            json!({
                "display_id": id,
                "online": online,
                "uptime_s": uptime_s,
                "last_ping_secs_ago": elapsed_secs,
            })
        }).collect()
    };

    let (ws_connects, ws_disconnects) = crate::ws::dashboard_ws_churn();
    let churn_json = json!({
        "connects_per_min": ws_connects,
        "disconnects_per_min": ws_disconnects,
        "healthy": ws_connects < 10,
    });

    let response = json!({
        "pods": result,
        "services": services,
        "displays": display_status,
        "dashboard_clients": crate::ws::dashboard_client_count(),
        "dashboard_ws_churn": churn_json,
        "venue_open": crate::venue_state::venue_is_open(),
        "timestamp": Utc::now().to_rfc3339(),
    });

    // RESIL-07: Cache the response for 5s
    {
        let mut cache = FLEET_HEALTH_CACHE.write().await;
        *cache = (Some(response.clone()), std::time::Instant::now());
    }

    Json(response)
}

// ── Phase 105 (v11.2): Sentry crash report endpoint ──────────────────────────

/// POST /api/v1/sentry/crash — accept crash report from rc-sentry on a pod.
/// LAN-only, no auth (consistent with all internal fleet endpoints).
pub async fn sentry_crash_handler(
    State(state): State<Arc<AppState>>,
    Json(report): Json<rc_common::types::SentryCrashReport>,
) -> axum::http::StatusCode {
    tracing::info!(
        target: "fleet-health",
        "sentry crash report from {}: tier={}, escalated={}, restarts={}",
        report.pod_id, report.resolution_tier, report.escalated, report.restart_count
    );

    // Store in fleet health
    let mut fleet = state.pod_fleet_health.write().await;
    if let Some(store) = fleet.get_mut(&report.pod_id) {
        store.last_sentry_crash = Some(report);
    } else {
        let mut new_store = FleetHealthStore::default();
        let pod_id = report.pod_id.clone();
        new_store.last_sentry_crash = Some(report);
        fleet.insert(pod_id, new_store);
    }

    axum::http::StatusCode::OK
}

// ── Blocked start notification (prevents silent deploy failures) ─────────────

/// POST /api/v1/fleet/blocked-start — receive notification when rc-agent
/// is blocked from starting on an unknown hostname.
///
/// This makes silent deploy failures visible: if someone deploys rc-agent
/// to a machine not in the allowlist, the server logs a warning and sends
/// a WhatsApp alert instead of the failure being silently hidden.
pub async fn blocked_start_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> axum::http::StatusCode {
    let hostname = body.get("hostname").and_then(|v| v.as_str()).unwrap_or("unknown");
    let reason = body.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown");

    tracing::warn!(
        target: "fleet-health",
        "BLOCKED START: rc-agent on '{}' refused to start (reason: {})",
        hostname, reason
    );

    let msg = format!(
        "⚠ BLOCKED START: rc-agent on '{}' refused to start.\nReason: {}\nAdd hostname to ALLOWED_HOSTS in rc-agent/src/main.rs and redeploy.",
        hostname, reason
    );
    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;

    axum::http::StatusCode::OK
}

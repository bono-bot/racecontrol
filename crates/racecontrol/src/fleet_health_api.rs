//! Fleet health API handlers — extracted from fleet_health.rs (Phase 385 ARCH-03).
//!
//! - `fleet_health_handler`: GET /api/v1/fleet/health
//! - `sentry_crash_handler`: POST /api/v1/sentry/crash
//! - `blocked_start_handler`: POST /api/v1/fleet/blocked-start
//! - `PodFleetStatus`: API response shape per pod

use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::fleet_health::FleetHealthStore;
use crate::state::AppState;

// Phase 445 Plan 02a Step A-C: definition moved to `rc_common::fleet_health_types`
// so gen-types can emit a TypeScript binding without crossing crate boundaries.
// Re-exported here for source-compat with existing consumers (handler builder,
// fleet_health.rs re-export). Field list and serde attrs preserved byte-for-byte.
pub use rc_common::fleet_health_types::PodFleetStatus;

/// Pattern I Part 5 Commit 6 (D3 invariant): pure derivation for
/// `stuck_session_candidate`. Extracted for documentation + single point
/// of change if the rule evolves (e.g. adding a `fallback_version`
/// freshness gate in Part 5.1).
pub(crate) fn compute_stuck_session_candidate(
    silent_reconnect_suspected: bool,
    has_active_session: bool,
) -> bool {
    silent_reconnect_suspected && has_active_session
}

#[cfg(test)]
mod pattern_i_part5_tests {
    use super::compute_stuck_session_candidate;

    #[test]
    fn flags_when_silent_reconnect_and_active_session() {
        // The canonical stuck case: server has a session, pod's WS is
        // silently dead, HTTP responds. Part-5-patched pods self-heal
        // within one T2 tick; a pre-patch pod would stay stuck here.
        assert!(compute_stuck_session_candidate(true, true));
    }

    #[test]
    fn no_flag_when_ws_connected() {
        // Normal racing customer on a live WS — never flag.
        assert!(!compute_stuck_session_candidate(false, true));
    }

    #[test]
    fn no_flag_when_no_active_session() {
        // Pod in silent-reconnect with NO active session — still worth
        // investigating (Pattern I class) but not a stuck-session case.
        assert!(!compute_stuck_session_candidate(true, false));
    }

    #[test]
    fn no_flag_when_both_false() {
        assert!(!compute_stuck_session_candidate(false, false));
    }
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
#[cfg_attr(feature = "gen-types", utoipa::path(
    get,
    path = "/api/v1/fleet/health",
    tag = "fleet",
    responses(
        (status = 200, description = "Fleet health snapshot (array of PodFleetStatus + timestamp)", body = serde_json::Value),
    )
))]
pub async fn fleet_health_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // RESIL-07: Return cached response if fresh (< 5s old)
    {
        let cache = FLEET_HEALTH_CACHE.read().await;
        if let (Some(ref cached), ts) = *cache
            && ts.elapsed().as_secs() < FLEET_HEALTH_CACHE_TTL_SECS {
                return Json(cached.clone());
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

    // Pattern I Part 5 Commit 6: snapshot active billing timers once, build
    // a `pod_id → session_id` map so each pod's entry can surface its
    // server-tracked active session id + the `stuck_session_candidate` flag
    // without re-acquiring the billing RwLock per-pod.
    let active_session_by_pod: HashMap<String, String> = {
        let timers = state.billing.active_timers.read().await;
        timers.values()
            .map(|t| (t.pod_id.clone(), t.session_id.clone()))
            .collect()
    };

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
                    silent_reconnect_suspected: false,
                    // Pattern I Part 5 Commit 6: unregistered pod slot has
                    // no active session by definition — both fields default
                    // to None/false.
                    active_session_id: None,
                    stuck_session_candidate: false,
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
                // Pattern I part 3: flag silent-reconnect-forever — HTTP ok + WS down
                // is the observable signature of the 2026-04-18 incident.
                let silent_reconnect_suspected = http_reachable && !ws_connected;
                // Pattern I Part 5 Commit 6 (D3): pod flagged stuck when server has
                // an active session for it BUT WS is down (silent-reconnect class).
                // A Part-5 rc-agent self-heals via HTTP fallback within one T2 tick
                // (~5 min); sustained flag after 5 min implies pre-patch binary
                // (rollback suspected) or silent-loop-death (Part 4 future work).
                let active_session_id = active_session_by_pod.get(pod_id).cloned();
                let stuck_session_candidate = compute_stuck_session_candidate(
                    silent_reconnect_suspected,
                    active_session_id.is_some(),
                );
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
                    silent_reconnect_suspected,
                    active_session_id,
                    stuck_session_candidate,
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

//! Fleet health monitoring for all 8 pods.
//!
//! Provides:
//! - `FleetHealthStore`: per-pod state updated by WS events and HTTP probes
//! - `PodFleetStatus`: API response shape per pod
//! - `store_startup_report`: called from WS StartupReport handler
//! - `clear_on_disconnect`: called from WS Disconnect and ungraceful socket-drop
//! - `start_probe_loop`: background task probing :8090/health every 15s
//! - `fleet_health_handler`: GET /api/v1/fleet/health

use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;

/// Per-pod health state maintained by WS events and HTTP probes.
/// Stored in `AppState::pod_fleet_health` keyed by pod_id.
#[derive(Debug, Clone, Default)]
pub struct FleetHealthStore {
    /// True if the most recent HTTP probe to :8090/health returned 200.
    /// Set by the background probe loop; NOT cleared on disconnect.
    pub http_reachable: bool,
    /// When the last HTTP probe was attempted.
    pub last_http_check: Option<DateTime<Utc>>,
    /// rc-agent binary version string from the most recent StartupReport.
    /// Cleared on disconnect.
    pub version: Option<String>,
    /// Git commit hash of the running rc-agent binary.
    /// Populated by the HTTP probe loop parsing :8090/health JSON.
    /// None = probe hasn't succeeded yet or old binary without build_id.
    pub build_id: Option<String>,
    /// Computed as `Utc::now() - uptime_secs` when StartupReport arrives.
    /// Used to compute live uptime_secs in the API response.
    /// Cleared on disconnect.
    pub agent_started_at: Option<DateTime<Utc>>,
    /// Whether the agent recovered from a crash on this boot.
    /// Cleared on disconnect.
    pub crash_recovery: Option<bool>,
    /// Phase 46: Whether the lock screen HTTP server (:18923) bound on last startup.
    pub lock_screen_port_bound: Option<bool>,
    /// Phase 46: Whether the remote ops HTTP server (:8090) bound on last startup.
    pub remote_ops_port_bound: Option<bool>,
    /// Phase 46: Whether the OpenFFBoard HID device was detected on last startup.
    pub hid_detected: Option<bool>,
    /// Phase 46: UDP telemetry ports that bound successfully on last startup.
    pub udp_ports_bound: Option<Vec<u16>>,
    /// Phase 100: True if the pod sent PreFlightFailed and has not yet cleared maintenance.
    pub in_maintenance: bool,
    /// Phase 100: Check names from the most recent PreFlightFailed message.
    pub maintenance_failures: Vec<String>,
    /// Phase 104: 24-hour violation count (populated by fleet_health_handler from pod_violations).
    pub violation_count_24h: u32,
    /// Phase 104: ISO-8601 timestamp of most recent violation for this pod.
    pub violation_count_last_at: Option<String>,
    /// Phase 105 (v11.2): Last crash report from rc-sentry on this pod.
    pub last_sentry_crash: Option<rc_common::types::SentryCrashReport>,
    /// Phase 138: Consecutive idle health check failures on this pod.
    /// Reset to 0 when a passing tick is observed (not tracked server-side — just stores last reported count).
    pub idle_health_fail_count: u32,
    /// Phase 138: Check names from the most recent IdleHealthFailed message (e.g. ["lock_screen_http", "window_rect"]).
    pub idle_health_failures: Vec<String>,
    /// Phase 206 (OBS-04): Currently active sentinel files on this pod.
    /// Keyed by file name, value is the action that made it active ("created").
    /// Cleared entry on "deleted". Serialized as a Vec<String> for API response.
    pub active_sentinels: Vec<String>,

    /// SHA256 of start-rcagent.bat on this pod (bat drift detection).
    /// Populated from agent /health response by probe loop.
    pub bat_sha256: Option<String>,

    // ─── RESIL-04/06/08 fields ─────────────────────────────────────────────
    /// RESIL-06: True if this pod has been auto-flagged for maintenance due to >3 crashes in 1 hour.
    pub maintenance_flag: bool,
    /// RESIL-06: Crash count in the last hour (updated by WS handler after each GameCrashed event).
    pub crashes_last_hour: i32,
    /// RESIL-08: Clock drift in seconds detected on last heartbeat (server_time - agent_time).
    /// None = no heartbeat with agent_timestamp received yet.
    pub clock_drift_secs: Option<i64>,

    // ─── Crash loop detection (Phase 9b) ─────────────────────────────────
    /// Timestamps of recent StartupReports (sliding window, max 10 entries).
    /// Used to detect crash loops: >3 reports in 5 minutes with uptime < 30s.
    pub startup_timestamps: Vec<DateTime<Utc>>,
    /// True if the pod is in a detected crash loop (>3 short-uptime restarts in 5 min).
    pub crash_loop: bool,
    /// CX-06: Pod experience score (0-100) from experience_collector.
    pub experience_score: Option<f64>,
    /// CX-06: "Healthy", "Maintenance", or "RemoveFromRotation"
    pub experience_status: Option<String>,
    /// Windows session ID from last StartupReport: 0 = Session 0 (broken GUI), 1+ = interactive.
    pub windows_session_id: Option<u32>,
    /// WS reconnect count: incremented each time the agent re-registers over WS.
    pub ws_reconnect_count: u32,
    /// Timestamps of recent WS reconnects for this pod (sliding window, max 20).
    pub ws_reconnect_times: Vec<DateTime<Utc>>,
}

// ViolationStore moved to fleet_violation_store.rs (ARCH-03).
pub use crate::fleet_violation_store::ViolationStore;

// PodFleetStatus + API handlers moved to fleet_health_api.rs (ARCH-03).
pub use crate::fleet_health_api::{PodFleetStatus, fleet_health_handler, sentry_crash_handler, blocked_start_handler};

/// Called from the WS StartupReport handler.
///
/// Updates `version`, `agent_started_at` (computed as now - uptime_secs),
/// `crash_recovery`, and Phase 46 boot verification fields in the store.
/// Does NOT touch `http_reachable` — that is probe-driven.
pub fn store_startup_report(
    store: &mut FleetHealthStore,
    version: &str,
    uptime_secs: u64,
    crash_recovery: bool,
    lock_screen_port_bound: bool,
    remote_ops_port_bound: bool,
    hid_detected: bool,
    udp_ports_bound: &[u16],
    windows_session_id: Option<u32>,
) {
    store.version = Some(version.to_string());
    store.agent_started_at = Some(
        Utc::now() - chrono::Duration::seconds(uptime_secs as i64),
    );
    store.crash_recovery = Some(crash_recovery);
    store.lock_screen_port_bound = Some(lock_screen_port_bound);
    store.remote_ops_port_bound = Some(remote_ops_port_bound);
    store.hid_detected = Some(hid_detected);
    store.udp_ports_bound = Some(udp_ports_bound.to_vec());
    store.windows_session_id = windows_session_id;

    // Bug #8: Clear last_sentry_crash on recovery — pod is healthy again
    if store.last_sentry_crash.is_some() {
        tracing::info!(target: "fleet-health", "Clearing last_sentry_crash — pod recovered (StartupReport received)");
        store.last_sentry_crash = None;
    }

    // ─── Phase 9b: Crash loop detection ──────────────────────────────────
    // Track startup timestamps for short-uptime restarts (uptime < 30s).
    // If >3 such restarts in a 5-minute window → crash loop detected.
    let now = Utc::now();
    if uptime_secs < 30 {
        store.startup_timestamps.push(now);
        // Keep only last 10 entries
        if store.startup_timestamps.len() > 10 {
            store.startup_timestamps.remove(0);
        }
        // Count entries within last 5 minutes
        let window = now - chrono::Duration::minutes(5);
        let recent_count = store.startup_timestamps.iter()
            .filter(|t| **t > window)
            .count();
        if recent_count > 3 && !store.crash_loop {
            store.crash_loop = true;
            tracing::error!(
                target: "fleet-health",
                "CRASH LOOP DETECTED: {} short-uptime restarts in 5 minutes (uptime={}s). \
                 Requires investigation — reboot pod if OS state is corrupt.",
                recent_count, uptime_secs
            );
        }
    } else {
        // Healthy startup (uptime >= 30s) — clear crash loop state
        store.crash_loop = false;
        store.startup_timestamps.clear();
    }
}

/// Called from both the graceful Disconnect handler and the ungraceful socket-drop cleanup.
///
/// Clears version, agent_started_at, and crash_recovery — fields that are only valid
/// while an agent is connected. Does NOT clear http_reachable, which is probe-driven
/// and remains valid until the next probe cycle.
pub fn clear_on_disconnect(store: &mut FleetHealthStore) {
    store.version = None;
    store.build_id = None;
    store.agent_started_at = None;
    store.crash_recovery = None;
    store.lock_screen_port_bound = None;
    store.remote_ops_port_bound = None;
    store.hid_detected = None;
    store.udp_ports_bound = None;
    // Disconnected pods are offline, not "in maintenance" from the server's perspective.
    store.in_maintenance = false;
    store.maintenance_failures.clear();
    // Do NOT clear active_sentinels on disconnect — sentinel files persist on disk.
    // They will re-sync when the agent reconnects and sentinel_watcher detects the files.
}

/// Phase 206 (OBS-04): Update sentinel file state for a pod.
///
/// Called from the WS handler when a `SentinelChange` message is received.
/// Adds the file name to `active_sentinels` on "created", removes it on "deleted".
pub fn update_sentinel(store: &mut FleetHealthStore, file: &str, action: &str) {
    match action {
        "created" => {
            if !store.active_sentinels.contains(&file.to_string()) {
                store.active_sentinels.push(file.to_string());
            }
        }
        "deleted" => {
            store.active_sentinels.retain(|s| s != file);
        }
        _ => {} // unknown action — ignore
    }
}

/// Phase 206 (OBS-04): Returns a snapshot of active sentinel files for a pod.
/// Used by the fleet_health_handler to populate active_sentinels in PodFleetStatus.
pub fn get_active_sentinels(store: &FleetHealthStore) -> Vec<String> {
    store.active_sentinels.clone()
}

/// Spawns the background HTTP probe loop.
///
/// Every 15 seconds, probes all registered pods at `http://<ip>:8090/health` in
/// parallel using a dedicated reqwest::Client with a 3-second timeout. Results
/// are written to `state.pod_fleet_health`.
///
/// IMPORTANT: Uses a dedicated client (3s timeout), NOT `state.http_client` (30s timeout).
pub fn start_probe_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        // Dedicated short-timeout client for health probes.
        // Bug #20: Replace .expect() with graceful error handling
        let probe_client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .connect_timeout(Duration::from_secs(3))
            .pool_max_idle_per_host(0)
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to build fleet probe HTTP client: {} — probe loop will not run", e);
                return;
            }
        };

        let mut ticker = tokio::time::interval(Duration::from_secs(15));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;

            // Snapshot pod IPs to probe (avoid holding lock during probes).
            let pod_targets: Vec<(String, String)> = {
                let pods = state.pods.read().await;
                pods.values()
                    .map(|p| (p.id.clone(), p.ip_address.clone()))
                    .collect()
            };

            if pod_targets.is_empty() {
                continue;
            }

            // Probe all pods in parallel.
            let probe_futs: Vec<_> = pod_targets
                .iter()
                .map(|(pod_id, ip)| {
                    let client = probe_client.clone();
                    let url = format!("http://{}:8090/health", ip);
                    let pod_id = pod_id.clone();
                    async move {
                        let result = client
                            .get(&url)
                            .timeout(Duration::from_secs(3))
                            .send()
                            .await;
                        let (reachable, build_id, uptime_secs, bat_sha256) = match result {
                            Ok(r) if r.status().is_success() => {
                                // Parse JSON to extract build_id, uptime_secs, bat_sha256.
                                match r.json::<serde_json::Value>().await.ok() {
                                    Some(v) => {
                                        let build_id = v.get("build_id").and_then(|b| b.as_str().map(String::from));
                                        let uptime = v.get("uptime_secs").and_then(|u| u.as_u64());
                                        let bat = v.get("bat_sha256").and_then(|b| b.as_str().map(String::from));
                                        (true, build_id, uptime, bat)
                                    }
                                    None => (true, None, None, None),
                                }
                            }
                            _ => (false, None, None, None),
                        };
                        (pod_id, reachable, build_id, uptime_secs, bat_sha256)
                    }
                })
                .collect();

            let results = futures_util::future::join_all(probe_futs).await;
            let now = Utc::now();

            // Write probe results into pod_fleet_health.
            let mut fleet = state.pod_fleet_health.write().await;
            for (pod_id, reachable, build_id, uptime_secs, bat_sha256) in results {
                let store = fleet.entry(pod_id.clone()).or_default();
                store.http_reachable = reachable;
                store.last_http_check = Some(now);
                if let Some(id) = build_id {
                    store.build_id = Some(id);
                }
                if let Some(bat) = bat_sha256 {
                    store.bat_sha256 = Some(bat);
                }

                // Phase 9b fix: Auto-clear stale crash_loop flag.
                // The StartupReport path can only SET crash_loop (uptime always <30s at boot).
                // This probe-based clearing provides the self-healing path:
                // if the pod has been stable for 5+ minutes, it's no longer crash-looping.
                if store.crash_loop {
                    if let Some(uptime) = uptime_secs {
                        if uptime >= 300 {
                            store.crash_loop = false;
                            store.startup_timestamps.clear();
                            tracing::info!(
                                target: "fleet-health",
                                "Crash loop cleared for {}: stable uptime {}s (probe-based auto-clear)",
                                pod_id, uptime
                            );
                        }
                    }
                }
            }

            // ── Fleet anomaly detection (Phase 310+) ────────────────────────
            // Snapshot fleet state, drop write lock, then check for anomalies.
            // Never hold lock across async WhatsApp calls (standing rule).
            let fleet_snapshot_for_anomalies = fleet.clone();
            drop(fleet);
            crate::fleet_anomaly_detection::detect_fleet_anomalies(&state, &fleet_snapshot_for_anomalies).await;

            // Services health is handled by app_health_monitor (30s, WhatsApp alerts, DB logging).
            // No duplicate probing needed here.
        }
    });
}

#[cfg(test)]
#[path = "fleet_health_tests.rs"]
mod tests;

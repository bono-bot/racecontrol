//! Fleet health monitoring for all 8 pods.
//!
//! Provides:
//! - `FleetHealthStore`: per-pod state updated by WS events and HTTP probes
//! - `PodFleetStatus`: API response shape per pod
//! - `store_startup_report`: called from WS StartupReport handler
//! - `clear_on_disconnect`: called from WS Disconnect and ungraceful socket-drop
//! - `start_probe_loop`: background task probing :8090/health every 15s
//! - `fleet_health_handler`: GET /api/v1/fleet/health

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
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
}

/// API response shape for a single pod in GET /api/v1/fleet/health.
#[derive(Debug, Clone, Serialize)]
pub struct PodFleetStatus {
    pub pod_number: u32,
    pub pod_id: Option<String>,
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
}

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
        let probe_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .connect_timeout(Duration::from_secs(3))
            .pool_max_idle_per_host(0)
            .build()
            .expect("Failed to build fleet probe HTTP client");

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
                        let (reachable, build_id) = match result {
                            Ok(r) if r.status().is_success() => {
                                // Parse JSON to extract build_id.
                                let build_id = r.json::<serde_json::Value>().await
                                    .ok()
                                    .and_then(|v| v.get("build_id")?.as_str().map(String::from));
                                (true, build_id)
                            }
                            _ => (false, None),
                        };
                        (pod_id, reachable, build_id)
                    }
                })
                .collect();

            let results = futures_util::future::join_all(probe_futs).await;
            let now = Utc::now();

            // Write probe results into pod_fleet_health.
            let mut fleet = state.pod_fleet_health.write().await;
            for (pod_id, reachable, build_id) in results {
                let store = fleet.entry(pod_id).or_default();
                store.http_reachable = reachable;
                store.last_http_check = Some(now);
                if let Some(id) = build_id {
                    store.build_id = Some(id);
                }
            }
        }
    });
}

/// GET /api/v1/fleet/health handler.
///
/// Returns a JSON object with `pods` (8 entries sorted by pod_number 1–8) and
/// `timestamp`. No authentication required — designed for Uday's phone on the LAN.
///
/// Pods that have never sent a WS message still appear with
/// ws_connected=false, http_reachable=false, and all optional fields null.
pub async fn fleet_health_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let pods_snapshot = state.pods.read().await;
    let senders = state.agent_senders.read().await;
    let fleet = state.pod_fleet_health.read().await;

    let mut result: Vec<PodFleetStatus> = Vec::with_capacity(8);

    for pod_number in 1u32..=8 {
        // Find registered PodInfo for this slot (if any).
        let pod_info = pods_snapshot
            .values()
            .find(|p| p.number == pod_number);

        match pod_info {
            None => {
                // Pod slot not registered yet — return all-false defaults.
                result.push(PodFleetStatus {
                    pod_number,
                    pod_id: None,
                    ws_connected: false,
                    http_reachable: false,
                    version: None,
                    build_id: None,
                    uptime_secs: None,
                    crash_recovery: None,
                    ip_address: None,
                    last_seen: None,
                    last_http_check: None,
                });
            }
            Some(info) => {
                let pod_id = &info.id;

                // WS connected = sender exists and channel is still open.
                let ws_connected = senders
                    .get(pod_id)
                    .map(|s| !s.is_closed())
                    .unwrap_or(false);

                // Fleet health store for version, uptime, http state.
                let store = fleet.get(pod_id);

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

                result.push(PodFleetStatus {
                    pod_number,
                    pod_id: Some(pod_id.clone()),
                    ws_connected,
                    http_reachable,
                    version,
                    build_id,
                    uptime_secs,
                    crash_recovery,
                    ip_address: Some(info.ip_address.clone()),
                    last_seen,
                    last_http_check,
                });
            }
        }
    }

    Json(json!({
        "pods": result,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // ── FleetHealthStore default ──────────────────────────────────────────────

    #[test]
    fn fleet_health_store_default_is_all_false_and_none() {
        let store = FleetHealthStore::default();
        assert!(!store.http_reachable, "http_reachable defaults to false");
        assert!(store.last_http_check.is_none());
        assert!(store.version.is_none());
        assert!(store.agent_started_at.is_none());
        assert!(store.crash_recovery.is_none());
    }

    // ── store_startup_report ─────────────────────────────────────────────────

    #[test]
    fn fleet_health_store_startup_report_sets_version() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 3600, false, false, false, false, &[]);
        assert_eq!(store.version, Some("0.5.2".to_string()));
    }

    #[test]
    fn fleet_health_store_startup_report_computes_agent_started_at() {
        let before = Utc::now();
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 100, false, false, false, false, &[]);
        let after = Utc::now();

        let started = store.agent_started_at.expect("agent_started_at should be set");
        // started_at should be ~100 seconds before now
        let delta_before = (before - started).num_seconds();
        let delta_after = (after - started).num_seconds();
        assert!(delta_before >= 99 && delta_before <= 101,
            "started_at should be ~100s before call time, got delta={}", delta_before);
        assert!(delta_after >= 99 && delta_after <= 101,
            "started_at should be ~100s before call time, got delta={}", delta_after);
    }

    #[test]
    fn fleet_health_store_startup_report_sets_crash_recovery() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 0, true, false, false, false, &[]);
        assert_eq!(store.crash_recovery, Some(true));

        let mut store2 = FleetHealthStore::default();
        store_startup_report(&mut store2, "0.5.2", 0, false, false, false, false, &[]);
        assert_eq!(store2.crash_recovery, Some(false));
    }

    #[test]
    fn fleet_health_store_startup_report_does_not_clear_http_reachable() {
        let mut store = FleetHealthStore::default();
        store.http_reachable = true;
        store_startup_report(&mut store, "0.5.2", 0, false, false, false, false, &[]);
        assert!(store.http_reachable, "http_reachable must not be modified by store_startup_report");
    }

    // ── clear_on_disconnect ───────────────────────────────────────────────────

    #[test]
    fn fleet_health_clear_on_disconnect_clears_version_and_started_at() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 100, true, false, false, false, &[]);

        // Verify preconditions
        assert!(store.version.is_some());
        assert!(store.agent_started_at.is_some());
        assert!(store.crash_recovery.is_some());

        clear_on_disconnect(&mut store);

        assert!(store.version.is_none(), "version should be cleared");
        assert!(store.agent_started_at.is_none(), "agent_started_at should be cleared");
        assert!(store.crash_recovery.is_none(), "crash_recovery should be cleared");
    }

    #[test]
    fn fleet_health_clear_on_disconnect_preserves_http_reachable() {
        let mut store = FleetHealthStore::default();
        store.http_reachable = true;
        store.last_http_check = Some(Utc::now());
        store_startup_report(&mut store, "0.5.2", 100, false, false, false, false, &[]);

        clear_on_disconnect(&mut store);

        assert!(store.http_reachable, "http_reachable should NOT be cleared by clear_on_disconnect");
        assert!(store.last_http_check.is_some(), "last_http_check should NOT be cleared");
    }

    // ── uptime_secs computed live ─────────────────────────────────────────────

    #[test]
    fn fleet_health_uptime_computed_live_increases_over_time() {
        let mut store = FleetHealthStore::default();
        // Simulate: agent started 300 seconds ago
        store.agent_started_at =
            Some(Utc::now() - chrono::Duration::seconds(300));

        let uptime = (Utc::now() - store.agent_started_at.unwrap()).num_seconds();
        assert!(uptime >= 299 && uptime <= 302,
            "uptime computed live should be ~300s, got {}", uptime);
    }

    // ── PodFleetStatus version/http_reachable from store ─────────────────────

    #[test]
    fn fleet_health_version_from_store_is_propagated() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 0, false, false, false, false, &[]);
        // Verify the store correctly holds the version for handler use
        assert_eq!(store.version.as_deref(), Some("0.5.2"));
    }

    #[test]
    fn fleet_health_http_reachable_from_store_is_propagated() {
        let mut store = FleetHealthStore::default();
        store.http_reachable = true;
        assert!(store.http_reachable);
    }

    // ── ws_connected logic ────────────────────────────────────────────────────

    #[test]
    fn fleet_health_ws_connected_false_when_no_sender() {
        // No sender in map means ws_connected = false
        use std::collections::HashMap;
        use tokio::sync::mpsc;

        let senders: HashMap<String, mpsc::Sender<rc_common::protocol::CoreToAgentMessage>> =
            HashMap::new();

        let ws_connected = senders
            .get("pod_1")
            .map(|s| !s.is_closed())
            .unwrap_or(false);

        assert!(!ws_connected);
    }

    #[test]
    fn fleet_health_ws_connected_true_when_sender_exists_and_open() {
        use std::collections::HashMap;
        use tokio::sync::mpsc;

        let (tx, _rx) = mpsc::channel::<rc_common::protocol::CoreToAgentMessage>(8);
        let mut senders = HashMap::new();
        senders.insert("pod_1".to_string(), tx);

        let ws_connected = senders
            .get("pod_1")
            .map(|s| !s.is_closed())
            .unwrap_or(false);

        assert!(ws_connected, "open sender should give ws_connected=true");
    }

    #[test]
    fn fleet_health_ws_connected_false_when_receiver_dropped() {
        use std::collections::HashMap;
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::channel::<rc_common::protocol::CoreToAgentMessage>(8);
        let mut senders = HashMap::new();
        senders.insert("pod_1".to_string(), tx);

        // Drop the receiver — sender should now be closed
        drop(rx);

        let ws_connected = senders
            .get("pod_1")
            .map(|s| !s.is_closed())
            .unwrap_or(false);

        assert!(!ws_connected, "dropped receiver should give ws_connected=false");
    }

    // ── Phase 46: boot verification fields ───────────────────────────────────

    #[test]
    fn fleet_health_store_startup_report_stores_boot_verification() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.6.0", 10, false, true, true, true, &[9996, 20777]);
        assert_eq!(store.lock_screen_port_bound, Some(true));
        assert_eq!(store.remote_ops_port_bound, Some(true));
        assert_eq!(store.hid_detected, Some(true));
        assert_eq!(store.udp_ports_bound, Some(vec![9996, 20777]));

        clear_on_disconnect(&mut store);
        assert_eq!(store.lock_screen_port_bound, None);
        assert_eq!(store.remote_ops_port_bound, None);
        assert_eq!(store.hid_detected, None);
        assert_eq!(store.udp_ports_bound, None);
    }
}

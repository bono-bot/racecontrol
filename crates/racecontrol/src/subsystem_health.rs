//! Subsystem Health Probes — per-subsystem ground truth health monitoring.
//!
//! Phase 352: Replaces the flat "always ok" health endpoint with real per-subsystem
//! probes. Background task runs every 10 seconds, caching results in LazyLock<RwLock<>>.
//! On state transitions (ok->degraded, degraded->ok), fires WhatsApp alerts with
//! 10-minute dedup per (subsystem, error_code) pair.
//!
//! Probes: db_writable, rc_backend, disk_free, cloud_sync, whatsapp_api,
//! fleet_connectivity, admin_db, db_sync_lag.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::state::AppState;

#[path = "subsystem_health_probes.rs"]
mod probes;

#[path = "subsystem_health_alerts.rs"]
mod alerts;

#[cfg(test)]
#[path = "subsystem_health_tests.rs"]
mod tests;

const LOG_TARGET: &str = "subsystem_health";
const PROBE_INTERVAL_SECS: u64 = 10;
const DEDUP_WINDOW_SECS: u64 = 600; // 10 minutes per OPS-04/D3

// ─── Types ──────────────────────────────────────────────────────────────────

/// Per-subsystem health status, returned in the /api/v1/health response.
#[derive(Debug, Clone, Serialize)]
pub struct SubsystemStatus {
    pub ok: bool,
    pub latency_ms: u64,
    pub error_code: Option<String>,
    pub detail: Option<String>,
}

/// Previous state for a subsystem (for transition detection).
struct PrevState {
    ok: bool,
    degraded_since: Option<Instant>,
}

// ─── Static State ───────────────────────────────────────────────────────────

/// Cached subsystem probe results. Updated every 10 seconds by the background task.
/// Read by the health endpoint handler (zero latency — reads cached, never blocks).
static SUBSYSTEM_STATE: LazyLock<RwLock<HashMap<String, SubsystemStatus>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Central dedup map per D3: (subsystem, error_code) -> last_alert_time.
/// Same (subsystem, error_code) within 10 minutes = suppressed.
static DEDUP_MAP: LazyLock<Mutex<HashMap<(String, String), Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ─── Public API ─────────────────────────────────────────────────────────────

/// Get current subsystem health status (read from cache, zero latency).
/// Returns empty HashMap before first probe cycle completes.
pub fn get_current_status() -> HashMap<String, SubsystemStatus> {
    SUBSYSTEM_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Spawn the subsystem health probe background task.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(subsystem_health_task(state));
    tracing::info!(target: "startup", "subsystem_health probe task spawned ({}s interval)", PROBE_INTERVAL_SECS);
}

// ─── Background Task ────────────────────────────────────────────────────────

async fn subsystem_health_task(state: Arc<AppState>) {
    tracing::info!(target: "state", task = "subsystem_health", event = "lifecycle", "lifecycle: started");

    // Startup grace — let server fully initialize
    tokio::time::sleep(Duration::from_secs(15)).await;

    let mut ticker = tokio::time::interval(Duration::from_secs(PROBE_INTERVAL_SECS));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Track previous state for transition detection
    let mut prev_states: HashMap<String, PrevState> = HashMap::new();

    loop {
        ticker.tick().await;
        run_probes(&state, &mut prev_states).await;
    }
}

async fn run_probes(state: &AppState, prev_states: &mut HashMap<String, PrevState>) {
    // Run all probes concurrently where independent
    let (db_writable, rc_backend, disk_free, cloud_sync, whatsapp_api, fleet_conn, admin_db, db_sync_lag) =
        tokio::join!(
            probes::probe_db_writable(&state.db),
            probes::probe_rc_backend(),
            probes::probe_disk_free(),
            probes::probe_cloud_sync(&state.db),
            probes::probe_whatsapp_api(state),
            probes::probe_fleet_connectivity(state),
            probes::probe_admin_db(),
            probes::probe_db_sync_lag(&state.config),
        );

    let mut results: HashMap<String, SubsystemStatus> = HashMap::new();
    results.insert("db_writable".to_string(), db_writable);
    results.insert("rc_backend".to_string(), rc_backend);
    results.insert("disk_free".to_string(), disk_free);
    results.insert("cloud_sync".to_string(), cloud_sync);
    results.insert("whatsapp_api".to_string(), whatsapp_api);
    results.insert("fleet_connectivity".to_string(), fleet_conn);
    results.insert("admin_db".to_string(), admin_db);
    results.insert("db_sync_lag".to_string(), db_sync_lag);

    // Check transitions and dispatch alerts
    for (name, status) in &results {
        let prev = prev_states.get(name);
        let was_ok = prev.map(|p| p.ok).unwrap_or(true);

        if was_ok && !status.ok {
            // ok -> degraded transition
            let error_code = status.error_code.as_deref().unwrap_or("UNKNOWN");
            let detail = status.detail.as_deref().unwrap_or("");

            if alerts::should_alert(name, error_code) {
                alerts::dispatch_subsystem_alert(
                    &state.config,
                    &state.db,
                    name,
                    error_code,
                    detail,
                )
                .await;
            }

            prev_states.insert(
                name.clone(),
                PrevState {
                    ok: false,
                    degraded_since: Some(Instant::now()),
                },
            );
        } else if !was_ok && status.ok {
            // degraded -> ok transition (recovery)
            let duration_secs = prev
                .and_then(|p| p.degraded_since)
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);

            alerts::dispatch_recovery_alert(&state.config, &state.db, name, duration_secs).await;

            prev_states.insert(
                name.clone(),
                PrevState {
                    ok: true,
                    degraded_since: None,
                },
            );
        } else {
            // No transition — update ok state but keep degraded_since
            prev_states
                .entry(name.clone())
                .and_modify(|p| p.ok = status.ok)
                .or_insert(PrevState {
                    ok: status.ok,
                    degraded_since: if status.ok { None } else { Some(Instant::now()) },
                });
        }
    }

    // Update cached state — clone results, drop lock before any async work
    {
        let mut guard = SUBSYSTEM_STATE
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *guard = results;
    }
}

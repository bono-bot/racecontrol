//! Fleet deployment status computation and cloud health probing.
//!
//! Extracted from deploy_awareness.rs — contains the heavy `compute_fleet_deploy_status`
//! function and the cloud health probe.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::state::AppState;

use super::{
    FleetDeployStatus, PodBuildInfo,
    SERVER_BUILD_ID,
    load_manifest, is_manifest_stale, count_server_restarts_1h,
    truncate_build_id, sanitize_build_id, RestartCount,
};

/// Compute the current fleet deployment status from in-memory state.
pub(super) async fn compute_fleet_deploy_status(state: &Arc<AppState>) -> FleetDeployStatus {
    let manifest = load_manifest().await;
    let manifest_missing = manifest.is_none();
    let manifest_stale = manifest.as_ref().map(is_manifest_stale).unwrap_or(false);
    let expected_build = manifest.as_ref().map(|m| m.expected_build_id.clone());

    // Read fleet health snapshot (clone quickly, drop lock)
    let fleet_health = {
        let guard = state.pod_fleet_health.read().await;
        guard.clone()
    };

    // Read pod info for numbers
    let pods_info = {
        let guard = state.pods.read().await;
        guard.clone()
    };

    // Read WS connection state (from agent_senders — pod has open WS if sender exists)
    let ws_states: HashMap<String, bool> = {
        let senders = state.agent_senders.read().await;
        pods_info.keys().map(|id| {
            let connected = senders.get(id).map(|s| !s.is_closed()).unwrap_or(false);
            (id.clone(), connected)
        }).collect()
    };

    // Count server restarts from watchdog log (blind spot #2)
    let (server_restarts_1h, watchdog_error) = match count_server_restarts_1h().await {
        RestartCount::Count(n) => (n, None),
        RestartCount::Unknown(reason) => (0, Some(reason)),
    };

    let server_current = match &expected_build {
        Some(expected) => SERVER_BUILD_ID == expected.as_str(),
        None => false, // MMA fix: no manifest = NOT confirmed current
    };

    let mut pod_builds: Vec<PodBuildInfo> = Vec::new();
    let mut distinct_builds: HashMap<String, u32> = HashMap::new();
    let mut pods_unknown_build = Vec::new();
    let mut pods_stale = Vec::new();

    for (pod_id, pod_info) in &pods_info {
        let health = fleet_health.get(pod_id);
        let raw_build_id = health.and_then(|h| h.build_id.clone());
        // MMA fix: sanitize pod-reported build_id
        let build_id = raw_build_id.and_then(|b| sanitize_build_id(&b));
        let ws_connected = ws_states.get(pod_id).copied().unwrap_or(false);
        let http_reachable = health.map(|h| h.http_reachable).unwrap_or(false);

        let matches_expected = match (&build_id, &expected_build) {
            (Some(b), Some(e)) => b == e,
            (None, _) => false, // Unknown = not matching
            (_, None) => true,  // No manifest = can't check
        };

        let matches_server = match &build_id {
            Some(b) => b.as_str() == SERVER_BUILD_ID,
            None => false,
        };

        if let Some(ref b) = build_id {
            *distinct_builds.entry(b.clone()).or_insert(0) += 1;
        }

        if build_id.is_none() && (ws_connected || http_reachable) {
            // Pod is reachable but has no build_id — anomaly
            pods_unknown_build.push(pod_id.clone());
        }

        if !matches_expected && build_id.is_some() {
            pods_stale.push(pod_id.clone());
        }

        pod_builds.push(PodBuildInfo {
            pod_id: pod_id.clone(),
            pod_number: pod_info.number,
            build_id,
            ws_connected,
            http_reachable,
            matches_expected,
            matches_server,
        });
    }

    // Sort by pod number for consistent output
    pod_builds.sort_by_key(|p| p.pod_number);

    // Count server build too
    *distinct_builds.entry(SERVER_BUILD_ID.to_string()).or_insert(0) += 1;

    // Build issues list
    let mut issues = Vec::new();

    // MMA fix: missing manifest is an issue, not silent
    if manifest_missing {
        issues.push("Deploy manifest missing — fleet version cannot be verified".to_string());
    }

    if manifest_stale {
        issues.push("Deploy manifest is stale (>24h old) — may not reflect current expected state".to_string());
    }

    if !server_current && expected_build.is_some()
        && let Some(ref expected) = expected_build {
            issues.push(format!(
                "Server build mismatch: running {} but manifest expects {}",
                truncate_build_id(SERVER_BUILD_ID), truncate_build_id(expected)
            ));
        }

    if !pods_unknown_build.is_empty() {
        issues.push(format!(
            "Pods with unknown build_id (online but unreported): {}",
            pods_unknown_build.join(", ")
        ));
    }

    if !pods_stale.is_empty() {
        issues.push(format!(
            "Pods running stale build (not matching expected): {}",
            pods_stale.join(", ")
        ));
    }

    if distinct_builds.len() > 1 {
        // MMA fix: use truncate_build_id for UTF-8-safe display
        let builds_summary: Vec<String> = distinct_builds
            .iter()
            .map(|(b, count)| format!("{}(x{})", truncate_build_id(b), count))
            .collect();
        issues.push(format!(
            "Fleet version fragmentation: {} distinct builds — {}",
            distinct_builds.len(),
            builds_summary.join(", ")
        ));
    }

    if server_restarts_1h >= 3 {
        issues.push(format!(
            "Server crash pattern: {} restarts in last hour (build {})",
            server_restarts_1h, truncate_build_id(SERVER_BUILD_ID)
        ));
    }

    // MMA Round 2 fix: surface watchdog log read failures as degraded
    if let Some(ref reason) = watchdog_error {
        issues.push(format!("Server restart detection degraded: {}", reason));
    }

    // --- Cloud health probe (MI Gap 1-2) ---
    let (cloud_build_id, cloud_reachable, cloud_in_sync) = probe_cloud_health(state).await;
    if cloud_reachable {
        if !cloud_in_sync
            && let Some(ref cb) = cloud_build_id {
                issues.push(format!(
                    "Cloud build diverged: cloud={} venue={} — rebuild cloud racecontrol binary",
                    truncate_build_id(cb), truncate_build_id(SERVER_BUILD_ID)
                ));
            }
    } else if state.config.cloud.enabled && state.config.cloud.api_url.is_some() {
        issues.push("Cloud racecontrol unreachable — cannot verify build sync".to_string());
    }

    // Determine overall status
    let status = if issues.is_empty() {
        "healthy".to_string()
    } else if !server_current || !pods_stale.is_empty() || server_restarts_1h >= 3 || (cloud_reachable && !cloud_in_sync) {
        "critical".to_string()
    } else {
        "degraded".to_string()
    };

    FleetDeployStatus {
        server_build_id: SERVER_BUILD_ID,
        expected_build_id: expected_build,
        server_current,
        pod_builds,
        distinct_builds: distinct_builds.len(),
        pods_unknown_build,
        pods_stale,
        server_restarts_1h,
        manifest_missing,
        manifest_stale,
        cloud_build_id,
        cloud_in_sync,
        cloud_reachable,
        status,
        issues,
    }
}

/// Probe cloud racecontrol health and compare build_id (MI Gap 1-2).
/// Returns (cloud_build_id, reachable, in_sync_with_venue).
async fn probe_cloud_health(state: &Arc<AppState>) -> (Option<String>, bool, bool) {
    let api_url = match &state.config.cloud.api_url {
        Some(url) if state.config.cloud.enabled => url.clone(),
        _ => return (None, false, true), // No cloud configured — consider "in sync"
    };

    // Strip trailing /api/v1 if present, then append /api/v1/health
    let base = api_url.trim_end_matches('/').trim_end_matches("/api/v1").trim_end_matches('/');
    let health_url = format!("{}/api/v1/health", base);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return (None, false, true),
    };

    match client.get(&health_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            #[derive(Deserialize)]
            struct HealthResp {
                build_id: Option<String>,
            }
            match resp.json::<HealthResp>().await {
                Ok(h) => {
                    let cloud_bid = h.build_id.and_then(|b| sanitize_build_id(&b));
                    let in_sync = match &cloud_bid {
                        Some(cb) => cb == SERVER_BUILD_ID,
                        None => true, // Can't determine — assume ok
                    };
                    (cloud_bid, true, in_sync)
                }
                Err(_) => (None, true, true), // Reachable but bad JSON — degrade gracefully
            }
        }
        Ok(_) => (None, true, true), // Non-success status — reachable but unhealthy
        Err(_) => (None, false, true), // Unreachable
    }
}

/// Send a WhatsApp alert for a deployment issue.
pub(super) async fn send_deploy_alert(state: &Arc<AppState>, message: &str) {
    let alert_msg = format!("[Deploy Awareness] {}", message);
    crate::whatsapp_alerter::send_admin_alert(&state.config, "deploy_awareness", &alert_msg).await;
}

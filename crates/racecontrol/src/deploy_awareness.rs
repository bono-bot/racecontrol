//! Deployment Awareness for Meshed Intelligence (DEPLOY-AWARE-01).
//!
//! Closes 4 blind spots in the mesh's situational awareness:
//!   1. Fleet version consistency — detects build_id mismatches across pods
//!   2. Server crash pattern detection — reads watchdog log for restart frequency
//!   3. Deployment completeness — compares fleet state against expected build manifest
//!   4. Stale build detection — flags pods/server running old builds
//!
//! Runs every 60 seconds alongside fleet_health probe loop.
//! Logs deployment anomalies to recovery-log.jsonl (on state transitions only)
//! and emits WhatsApp alerts for critical issues.
//!
//! MMA-audited: GPT-5.4 + Gemini 3.1 Pro (2026-03-30). Fixes applied:
//!   - tokio::fs for non-blocking manifest read
//!   - UTF-8-safe build_id truncation (char boundary guard)
//!   - State-transition logging (no log flooding)
//!   - Missing manifest = degraded (not healthy)
//!   - Server restart detection from watchdog log
//!   - Stale manifest detection (>24h = warning)

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::time::interval;

use crate::state::AppState;
use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_SERVER};

const LOG_TARGET: &str = "deploy-awareness";
const SCAN_INTERVAL_SECS: u64 = 60;
/// Path to deployment manifest (written by deploy scripts after staging)
const DEPLOY_MANIFEST_PATH: &str = r"C:\RacingPoint\deploy-manifest.json";
/// Path to watchdog log (parsed for server restart detection)
const WATCHDOG_LOG_PATH: &str = r"C:\RacingPoint\racecontrol-watchdog.log";
/// Server's own build_id (compile-time)
const SERVER_BUILD_ID: &str = env!("GIT_HASH");
/// Max age for a manifest before it's flagged as stale (24 hours)
const MANIFEST_STALE_SECS: i64 = 86400;
/// Max build_id length to accept from pods (sanitization)
const MAX_BUILD_ID_LEN: usize = 64;

/// Deployment manifest — written by deploy-server.sh / deploy-pod.sh after staging.
/// Records the expected build_id for the fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployManifest {
    /// Expected build_id for all binaries in this deploy wave
    pub expected_build_id: String,
    /// When this manifest was created (RFC3339)
    pub created_at: String,
    /// Which targets are included: "server", "pods", "all"
    pub targets: Vec<String>,
    /// SHA256 of the staged binary (for integrity verification)
    pub binary_sha256: Option<String>,
    /// Human-readable deploy reason
    pub reason: Option<String>,
}

/// Fleet deployment status snapshot — computed each scan cycle.
#[derive(Debug, Clone, Serialize)]
pub struct FleetDeployStatus {
    /// Server's own build_id
    pub server_build_id: &'static str,
    /// Expected build from manifest (None if no manifest)
    pub expected_build_id: Option<String>,
    /// Whether server matches expected build
    pub server_current: bool,
    /// Per-pod build info
    pub pod_builds: Vec<PodBuildInfo>,
    /// Number of distinct build_ids across the fleet (1 = uniform)
    pub distinct_builds: usize,
    /// Pods with unknown (None) build_id
    pub pods_unknown_build: Vec<String>,
    /// Pods mismatched from expected build
    pub pods_stale: Vec<String>,
    /// Server restart count in the last hour (from watchdog log)
    pub server_restarts_1h: u32,
    /// Whether manifest is missing
    pub manifest_missing: bool,
    /// Whether manifest is stale (>24h old)
    pub manifest_stale: bool,
    /// Cloud racecontrol build_id (None if cloud unreachable or not configured)
    pub cloud_build_id: Option<String>,
    /// Whether cloud build matches venue server build
    pub cloud_in_sync: bool,
    /// Cloud API reachable
    pub cloud_reachable: bool,
    /// Overall deployment health: "healthy", "degraded", "critical"
    pub status: String,
    /// Human-readable issues
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PodBuildInfo {
    pub pod_id: String,
    pub pod_number: u32,
    pub build_id: Option<String>,
    pub ws_connected: bool,
    pub http_reachable: bool,
    pub matches_expected: bool,
    pub matches_server: bool,
}

/// Internal state for tracking deployment issues across scan cycles.
/// Only logs on state transitions to prevent recovery-log flooding.
struct DeployAwarenessState {
    /// Last known fleet deploy status (for change detection)
    last_status: Option<String>,
    /// Issue keys that were active in the PREVIOUS scan (for transition detection)
    previous_issues: HashSet<String>,
    /// Whether we've already alerted for the current set of issues (key -> last alert time)
    alerted_issues: HashMap<String, Instant>,
    /// Cooldown between alerts for the same issue (15 min)
    alert_cooldown: Duration,
}

impl DeployAwarenessState {
    fn new() -> Self {
        Self {
            last_status: None,
            previous_issues: HashSet::new(),
            alerted_issues: HashMap::new(),
            alert_cooldown: Duration::from_secs(900), // 15 min
        }
    }

    /// Check if we should alert for a given issue key (respects cooldown).
    fn should_alert(&mut self, issue_key: &str) -> bool {
        let now = Instant::now();
        if let Some(last) = self.alerted_issues.get(issue_key) {
            if now.duration_since(*last) < self.alert_cooldown {
                return false;
            }
        }
        self.alerted_issues.insert(issue_key.to_string(), now);
        true
    }
}

/// Load the deployment manifest from disk (async, non-blocking).
/// Returns None if not found or invalid.
async fn load_manifest() -> Option<DeployManifest> {
    let content = tokio::fs::read_to_string(DEPLOY_MANIFEST_PATH).await.ok()?;
    serde_json::from_str(&content).ok()
}

/// Check if a manifest is stale (created_at > 24h ago).
fn is_manifest_stale(manifest: &DeployManifest) -> bool {
    if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&manifest.created_at) {
        let age = chrono::Utc::now().signed_duration_since(created);
        age.num_seconds() > MANIFEST_STALE_SECS
    } else {
        true // Unparseable timestamp = treat as stale
    }
}

/// Result of watchdog log parsing. Distinguishes "0 restarts" from "can't read log".
enum RestartCount {
    /// Successfully counted restarts
    Count(u32),
    /// Watchdog log unreadable — deployment awareness is degraded
    Unknown(String),
}

/// Count server restarts in the last hour by parsing the watchdog log.
/// Looks for lines containing "RESTART SUCCESS" with timestamps in the last hour.
/// Watchdog log timestamps are in server local time (IST = UTC+5:30).
async fn count_server_restarts_1h() -> RestartCount {
    let content = match tokio::fs::read_to_string(WATCHDOG_LOG_PATH).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Cannot read watchdog log: {e}");
            return RestartCount::Unknown(format!("Watchdog log unreadable: {e}"));
        }
    };

    // Watchdog log is written in server local time (IST = UTC+5:30).
    // Compute IST "now" and "1h ago" to compare correctly.
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .expect("IST offset is valid");
    let now_ist = chrono::Utc::now().with_timezone(&ist_offset);
    let one_hour_ago_ist = now_ist - chrono::Duration::hours(1);

    let mut count = 0u32;

    for line in content.lines().rev() {
        // Watchdog log format: "2026-03-30 13:40:23 | RESTART SUCCESS: ..."
        // or "Mon 03/30/2026 17:19:01.45 WATCHDOG: restart SUCCESS"
        if !line.contains("RESTART SUCCESS") && !line.contains("restart SUCCESS") {
            continue;
        }

        // Try to parse timestamp from the start of the line
        // Format 1: "2026-03-30 13:40:23 | ..." (IST local time)
        if line.len() >= 19 {
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&line[..19], "%Y-%m-%d %H:%M:%S") {
                // Interpret as IST (server local time)
                let ist_dt = naive.and_local_timezone(ist_offset);
                if let chrono::LocalResult::Single(dt) = ist_dt {
                    if dt >= one_hour_ago_ist {
                        count += 1;
                    } else {
                        break; // Log is chronological — once we pass 1h, stop
                    }
                }
            }
        }
    }
    RestartCount::Count(count)
}

/// Safely truncate a build_id to 8 chars for display, respecting UTF-8 boundaries.
fn truncate_build_id(s: &str) -> &str {
    if s.len() <= 8 {
        return s;
    }
    // Find the last char boundary at or before index 8
    let mut end = 8;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Sanitize a build_id from a pod: limit length, reject control chars.
fn sanitize_build_id(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > MAX_BUILD_ID_LEN {
        return None;
    }
    if raw.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(raw.to_string())
}

/// Spawn the deployment awareness background task.
/// Scans every 60s, logs anomalies on state transitions, alerts on critical issues.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "Deployment awareness task started (interval: {}s, server build: {})", SCAN_INTERVAL_SECS, SERVER_BUILD_ID);

        // Grace period — let fleet_health populate first
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut tick = interval(Duration::from_secs(SCAN_INTERVAL_SECS));
        let mut awareness_state = DeployAwarenessState::new();
        let logger = RecoveryLogger::new(RECOVERY_LOG_SERVER);

        loop {
            tick.tick().await;

            let status = compute_fleet_deploy_status(&state).await;

            // Log status changes
            let status_str = status.status.clone();
            if awareness_state.last_status.as_deref() != Some(&status_str) {
                tracing::info!(
                    target: LOG_TARGET,
                    status = %status.status,
                    distinct_builds = status.distinct_builds,
                    pods_stale = status.pods_stale.len(),
                    pods_unknown = status.pods_unknown_build.len(),
                    server_current = status.server_current,
                    server_restarts_1h = status.server_restarts_1h,
                    manifest_missing = status.manifest_missing,
                    "Fleet deployment status changed"
                );
                awareness_state.last_status = Some(status_str.clone());
            }

            // Build current issue set for transition detection
            let current_issues: HashSet<String> = status.issues.iter().map(|i| issue_to_key(i)).collect();

            // Only log NEW issues (not present in previous scan) — prevents log flooding
            for issue in &status.issues {
                let issue_key = issue_to_key(issue);

                if !awareness_state.previous_issues.contains(&issue_key) {
                    // New issue — log to recovery journal
                    let decision = RecoveryDecision::new(
                        "server",
                        "fleet",
                        RecoveryAuthority::PodHealer,
                        RecoveryAction::AlertStaff,
                        &format!("deploy_awareness: {}", issue),
                    );
                    let _ = logger.log(&decision);
                    tracing::warn!(target: LOG_TARGET, issue = %issue, "New deployment issue detected");
                }

                // Alert on critical issues (with cooldown)
                if status.status == "critical" && awareness_state.should_alert(&issue_key) {
                    tracing::error!(target: LOG_TARGET, issue = %issue, "CRITICAL deployment issue");
                    send_deploy_alert(&state, issue).await;
                }
            }

            // Log resolved issues
            for prev_key in &awareness_state.previous_issues {
                if !current_issues.contains(prev_key) {
                    let decision = RecoveryDecision::new(
                        "server",
                        "fleet",
                        RecoveryAuthority::PodHealer,
                        RecoveryAction::AlertStaff,
                        &format!("deploy_awareness RESOLVED: {}", prev_key),
                    );
                    let _ = logger.log(&decision);
                    tracing::info!(target: LOG_TARGET, issue = %prev_key, "Deployment issue resolved");
                }
            }

            // Update previous issues for next cycle
            awareness_state.previous_issues = current_issues;
        }
    });
}

/// Compute the current fleet deployment status from in-memory state.
async fn compute_fleet_deploy_status(state: &Arc<AppState>) -> FleetDeployStatus {
    let manifest = load_manifest().await;
    let manifest_missing = manifest.is_none();
    let manifest_stale = manifest.as_ref().map(|m| is_manifest_stale(m)).unwrap_or(false);
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

    if !server_current && expected_build.is_some() {
        if let Some(ref expected) = expected_build {
            issues.push(format!(
                "Server build mismatch: running {} but manifest expects {}",
                truncate_build_id(SERVER_BUILD_ID), truncate_build_id(expected)
            ));
        }
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
        if !cloud_in_sync {
            if let Some(ref cb) = cloud_build_id {
                issues.push(format!(
                    "Cloud build diverged: cloud={} venue={} — rebuild cloud racecontrol binary",
                    truncate_build_id(cb), truncate_build_id(SERVER_BUILD_ID)
                ));
            }
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
async fn send_deploy_alert(state: &Arc<AppState>, message: &str) {
    let alert_msg = format!("[Deploy Awareness] {}", message);
    crate::whatsapp_alerter::send_admin_alert(&state.config, "deploy_awareness", &alert_msg).await;
}

/// Convert an issue string to a stable key for dedup/cooldown.
fn issue_to_key(issue: &str) -> String {
    if issue.contains("Server build mismatch") {
        "server_build_mismatch".to_string()
    } else if issue.contains("unknown build_id") {
        "pods_unknown_build".to_string()
    } else if issue.contains("stale build") {
        "pods_stale_build".to_string()
    } else if issue.contains("version fragmentation") {
        "fleet_fragmentation".to_string()
    } else if issue.contains("manifest missing") {
        "manifest_missing".to_string()
    } else if issue.contains("manifest is stale") {
        "manifest_stale".to_string()
    } else if issue.contains("crash pattern") {
        "server_crash_pattern".to_string()
    } else if issue.contains("restart detection degraded") {
        "watchdog_unreadable".to_string()
    } else {
        // MMA fix: safe truncation instead of byte slicing
        let safe_prefix: String = issue.chars().take(32).collect();
        format!("other:{}", safe_prefix)
    }
}

// ─── Public API for mesh/stats endpoint ──────────────────────────────────────

/// Get current fleet deployment status (called by /api/v1/mesh/deploy-status).
pub async fn get_fleet_deploy_status(state: &Arc<AppState>) -> FleetDeployStatus {
    compute_fleet_deploy_status(state).await
}

/// Get the server's own build_id.
pub fn server_build_id() -> &'static str {
    SERVER_BUILD_ID
}

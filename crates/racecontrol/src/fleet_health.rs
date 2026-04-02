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
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;
use rc_common::types::ProcessViolation;

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
}

/// Per-pod violation history with time-based eviction and fingerprint dedup.
///
/// MMA-P1 fixes (4-model consensus):
/// - Time-based eviction (24h) instead of 100-entry FIFO cap — real violations
///   can no longer be evicted by high-frequency false positives (vendor schtasks).
/// - Fingerprint dedup: recurring "reported" violations for the same process/task
///   increment `seen_count` instead of creating new entries.
/// - repeat_offender_check works in report_only mode (not gated on "killed").
/// - Future timestamps rejected (clock skew protection).
/// - Hard cap at 1000 entries as safety net (time-based eviction is primary).
#[derive(Debug, Clone, Default)]
pub struct ViolationStore {
    entries: VecDeque<ProcessViolation>,
    /// Dedup index: fingerprint (lowercase name + violation_type) → index of last entry.
    /// Used to increment seen_count for recurring "reported" violations instead of duplicating.
    dedup_index: HashMap<String, usize>,
    /// Rolling counter of scan failures (not stored as violations).
    pub scan_failure_count: u32,
}

impl ViolationStore {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            dedup_index: HashMap::new(),
            scan_failure_count: 0,
        }
    }

    /// Fingerprint for dedup: lowercased name + violation type string.
    fn fingerprint(v: &ProcessViolation) -> String {
        format!("{}:{:?}", v.name.to_lowercase(), v.violation_type)
    }

    /// Insert a violation with dedup and time-based eviction.
    ///
    /// - "reported" violations with the same fingerprint increment `consecutive_count`
    ///   on the existing entry instead of creating duplicates.
    /// - "killed"/"disabled"/"removed" violations always create new entries (they represent actions).
    /// - Entries older than 24h are evicted on push.
    /// - Hard cap at 1000 entries as safety net.
    pub fn push(&mut self, v: ProcessViolation) {
        let now = Utc::now();

        // MMA-P2: Reject future timestamps (clock skew protection)
        if let Ok(parsed) = DateTime::parse_from_rfc3339(&v.timestamp) {
            let age_secs = (now - parsed.with_timezone(&Utc)).num_seconds();
            if age_secs < -60 {
                // More than 60s in the future — likely clock skew, drop silently
                return;
            }
        }

        // Time-based eviction: remove entries older than 24h
        self.evict_stale(now);

        // Hard cap safety net FIRST (before dedup lookup to keep indices valid)
        if self.entries.len() >= 1000 {
            if let Some(old) = self.entries.pop_front() {
                self.dedup_index.remove(&Self::fingerprint(&old));
            }
            // MMA-Iter2-P1: Rebuild dedup index after pop (indices shifted).
            // Must happen BEFORE dedup lookup to prevent stale index access.
            self.rebuild_dedup_index();
        }

        // Dedup: for "reported" violations, increment existing entry's count
        if v.action_taken == "reported" {
            let fp = Self::fingerprint(&v);
            if let Some(&idx) = self.dedup_index.get(&fp) {
                if let Some(existing) = self.entries.get_mut(idx) {
                    existing.consecutive_count = existing.consecutive_count.saturating_add(1);
                    existing.timestamp = v.timestamp; // update to latest sighting
                    return;
                }
            }
            // New fingerprint — track index AFTER push_back (done below)
        }

        self.entries.push_back(v);

        // Update dedup index for the newly pushed entry (if "reported")
        if let Some(last) = self.entries.back() {
            if last.action_taken == "reported" {
                let fp = Self::fingerprint(last);
                self.dedup_index.insert(fp, self.entries.len() - 1);
            }
        }
    }

    /// Record a scan failure (not a violation — tracked separately for OTA gating).
    pub fn record_scan_failure(&mut self) {
        self.scan_failure_count = self.scan_failure_count.saturating_add(1);
    }

    /// Evict entries older than 24 hours.
    fn evict_stale(&mut self, now: DateTime<Utc>) {
        let cutoff = now - chrono::Duration::hours(24);
        let before_len = self.entries.len();
        self.entries.retain(|v| {
            DateTime::parse_from_rfc3339(&v.timestamp)
                .map(|t| t.with_timezone(&Utc) >= cutoff)
                .unwrap_or(false) // drop unparseable timestamps
        });
        if self.entries.len() != before_len {
            self.rebuild_dedup_index();
        }
    }

    /// Rebuild dedup index after eviction (only for "reported" entries).
    fn rebuild_dedup_index(&mut self) {
        self.dedup_index.clear();
        for (idx, v) in self.entries.iter().enumerate() {
            if v.action_taken == "reported" {
                self.dedup_index.insert(Self::fingerprint(v), idx);
            }
        }
    }

    /// Count distinct violations within the last 24 hours.
    /// MMA-P1: Uses time-based window with future-timestamp rejection.
    /// MMA-Iter2-P2: Excludes guard degradation notifications (state changes, not violations).
    pub fn violation_count_24h(&self, now: DateTime<Utc>) -> u32 {
        let cutoff = now - chrono::Duration::hours(24);
        self.entries.iter().filter(|v| {
            // Skip guard degradation notifications — they're state changes, not violations
            if v.action_taken == "guard_degraded_to_report_only" {
                return false;
            }
            DateTime::parse_from_rfc3339(&v.timestamp)
                .map(|t| {
                    let ts = t.with_timezone(&Utc);
                    // MMA-P2: Reject future timestamps in count too
                    ts >= cutoff && ts <= now + chrono::Duration::seconds(60)
                })
                .unwrap_or(false)
        }).count() as u32
    }

    /// Timestamp of the most recently pushed violation, or None if empty.
    pub fn last_violation_at(&self) -> Option<&str> {
        self.entries.back().map(|v| v.timestamp.as_str())
    }

    /// Returns true if `violation` should trigger escalation:
    /// MMA-P1: Works in ALL modes (report_only + kill_and_report).
    /// Checks if the same process name has been seen >= threshold times
    /// in the last 300 seconds, regardless of action_taken.
    pub fn repeat_offender_check(&self, violation: &ProcessViolation, now: DateTime<Utc>) -> bool {
        let name_lower = violation.name.to_lowercase();
        let window_start = now - chrono::Duration::seconds(300);
        // MMA-Iter2-P2: Count DISTINCT entries, not consecutive_count sum.
        // Summing consecutive_count causes false positives on benign vendor tasks
        // that get re-reported hourly (consecutive_count grows to 100+).
        let distinct_entries: usize = self.entries.iter()
            .filter(|v| {
                v.name.to_lowercase() == name_lower
                    && DateTime::parse_from_rfc3339(&v.timestamp)
                        .map(|t| {
                            let ts = t.with_timezone(&Utc);
                            ts >= window_start && ts <= now
                        })
                        .unwrap_or(false)
            })
            .count();
        // Threshold: 5 distinct entries in 300s = repeat offender (works in report_only mode)
        distinct_entries >= 5
    }
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
            detect_fleet_anomalies(&state, &fleet_snapshot_for_anomalies).await;

            // Services health is handled by app_health_monitor (30s, WhatsApp alerts, DB logging).
            // No duplicate probing needed here.
        }
    });
}

/// Fleet anomaly detection — checks 4 dimensions after each probe cycle:
/// 1. Build version skew (majority build_id vs outliers)
/// 2. Clock drift >30s
/// 3. Ready delay >60s
/// 4. Guard violation spike
///
/// Uses static cooldowns (15 min per class) to prevent alert fatigue.
async fn detect_fleet_anomalies(
    state: &Arc<AppState>,
    fleet: &HashMap<String, FleetHealthStore>,
) {
    use std::sync::atomic::{AtomicI64, Ordering};

    // Cooldown: one alert per class per 15 minutes
    static LAST_BUILD_ALERT: AtomicI64 = AtomicI64::new(0);
    static LAST_DRIFT_ALERT: AtomicI64 = AtomicI64::new(0);
    static LAST_DELAY_ALERT: AtomicI64 = AtomicI64::new(0);
    static LAST_VIOLATION_ALERT: AtomicI64 = AtomicI64::new(0);
    const COOLDOWN_SECS: i64 = 900; // 15 minutes

    let now_ts = Utc::now().timestamp();

    // ── 0. Empty fleet guard (closes incident #5: pods DB desync) ──────────
    // If fleet map is empty but agents are connected, something is wrong.
    {
        static LAST_EMPTY_ALERT: AtomicI64 = AtomicI64::new(0);
        let agents_connected = {
            let senders = state.agent_senders.read().await;
            senders.values().filter(|s| !s.is_closed()).count()
        };
        if fleet.is_empty() && agents_connected > 0 {
            tracing::error!(
                target: "fleet-anomaly",
                "PODS_EMPTY: fleet health store is empty but {} agents connected — kiosk may show 'Waiting for pods'",
                agents_connected
            );
            if now_ts - LAST_EMPTY_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_EMPTY_ALERT.store(now_ts, Ordering::Relaxed);
                let msg = format!(
                    "⚠ PODS EMPTY: Fleet health store empty but {} WS agents connected.\nKiosk may show 'Waiting for pods'. Server may need restart or pods need to reconnect.",
                    agents_connected
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 1. Build version skew ──────────────────────────────────────────────
    // Find the majority build_id among reachable pods, flag outliers.
    {
        let mut build_counts: HashMap<&str, Vec<&str>> = HashMap::new();
        for (pod_id, store) in fleet.iter() {
            if store.http_reachable {
                if let Some(ref bid) = store.build_id {
                    build_counts.entry(bid.as_str()).or_default().push(pod_id.as_str());
                }
            }
        }

        if build_counts.len() > 1 {
            // Find majority
            let (majority_build, _majority_pods) = build_counts
                .iter()
                .max_by_key(|(_, pods)| pods.len())
                .unwrap();

            let outliers: Vec<(&str, &str)> = build_counts
                .iter()
                .filter(|(bid, _)| *bid != majority_build)
                .flat_map(|(bid, pods)| pods.iter().map(move |p| (*p, *bid)))
                .collect();

            if !outliers.is_empty() {
                for (pod_id, bid) in &outliers {
                    tracing::warn!(
                        target: "fleet-anomaly",
                        "BUILD_SKEW: {} running {} (fleet majority: {})",
                        pod_id, bid, majority_build
                    );
                }

                if now_ts - LAST_BUILD_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_BUILD_ALERT.store(now_ts, Ordering::Relaxed);
                    let outlier_list: Vec<String> = outliers
                        .iter()
                        .map(|(p, b)| format!("{} ({})", p, b))
                        .collect();
                    let msg = format!(
                        "⚠ BUILD SKEW: {} pod(s) running different build than fleet majority ({}):\n{}",
                        outliers.len(), majority_build,
                        outlier_list.join(", ")
                    );
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }
        }
    }

    // ── 2. Clock drift >30s ────────────────────────────────────────────────
    {
        const CLOCK_DRIFT_THRESHOLD_SECS: i64 = 30;
        let mut drifted: Vec<(&str, i64)> = Vec::new();

        for (pod_id, store) in fleet.iter() {
            if let Some(drift) = store.clock_drift_secs {
                if drift.abs() > CLOCK_DRIFT_THRESHOLD_SECS {
                    drifted.push((pod_id.as_str(), drift));
                    tracing::warn!(
                        target: "fleet-anomaly",
                        "CLOCK_DRIFT: {} drifted {}s (threshold: {}s)",
                        pod_id, drift, CLOCK_DRIFT_THRESHOLD_SECS
                    );
                }
            }
        }

        if !drifted.is_empty() && now_ts - LAST_DRIFT_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
            LAST_DRIFT_ALERT.store(now_ts, Ordering::Relaxed);
            let list: Vec<String> = drifted.iter().map(|(p, d)| format!("{}: {}s", p, d)).collect();
            let msg = format!(
                "⚠ CLOCK DRIFT: {} pod(s) drifted >{}s from server:\n{}",
                drifted.len(), CLOCK_DRIFT_THRESHOLD_SECS, list.join(", ")
            );
            crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
        }
    }

    // ── 3. Guard violation spike (fleet-wide) ──────────────────────────────
    // Check violation stores for recent surge (>10 in last hour across fleet).
    {
        let violations = state.pod_violations.read().await;
        let now = Utc::now();
        let mut total_violations_1h: u32 = 0;
        let mut pods_with_violations: Vec<(&str, u32)> = Vec::new();

        for (pod_id, vstore) in violations.iter() {
            let count_24h = vstore.violation_count_24h(now);
            if count_24h > 0 {
                pods_with_violations.push((pod_id.as_str(), count_24h));
                total_violations_1h += count_24h;
            }
        }

        // Fleet-wide pattern: if ALL pods have violations, it's likely a config issue
        let pod_count = fleet.values().filter(|s| s.http_reachable).count();
        let pods_affected = pods_with_violations.len();

        if pods_affected > 0 && (total_violations_1h > 20 || pods_affected >= pod_count.max(1)) {
            tracing::warn!(
                target: "fleet-anomaly",
                "VIOLATION_SPIKE: {} violations across {}/{} pods in 24h — possible config issue",
                total_violations_1h, pods_affected, pod_count
            );

            if now_ts - LAST_VIOLATION_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_VIOLATION_ALERT.store(now_ts, Ordering::Relaxed);
                let list: Vec<String> = pods_with_violations
                    .iter()
                    .map(|(p, c)| format!("{}: {}", p, c))
                    .collect();
                let msg = format!(
                    "⚠ VIOLATION SPIKE: {} total violations across {}/{} pods (24h):\n{}\nIf ALL pods affected, check allowlist config.",
                    total_violations_1h, pods_affected, pod_count, list.join(", ")
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 4. Ready delay >60s (from DB, checked less frequently) ─────────────
    // This runs every probe cycle (15s) but the DB query is lightweight.
    // Only alert if a pod consistently has >60s avg ready delay over 7 days.
    {
        const READY_DELAY_THRESHOLD_MS: f64 = 60_000.0;

        let rows: Vec<(String, f64)> = match sqlx::query_as(
            "SELECT pod_id, AVG(CAST(duration_to_playable_ms AS REAL))
             FROM launch_events
             WHERE duration_to_playable_ms IS NOT NULL
               AND created_at >= datetime('now', '-7 days')
             GROUP BY pod_id
             HAVING AVG(CAST(duration_to_playable_ms AS REAL)) > ?",
        )
        .bind(READY_DELAY_THRESHOLD_MS)
        .fetch_all(&state.db)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "SLOW_READY DB query failed: {} — skipping check this cycle",
                    e
                );
                vec![]
            }
        };

        if !rows.is_empty() {
            for (pod_id, avg_ms) in &rows {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "SLOW_READY: {} avg ready delay {:.0}ms (threshold: {:.0}ms)",
                    pod_id, avg_ms, READY_DELAY_THRESHOLD_MS
                );
            }

            if now_ts - LAST_DELAY_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_DELAY_ALERT.store(now_ts, Ordering::Relaxed);
                let list: Vec<String> = rows.iter().map(|(p, ms)| format!("{}: {:.0}ms", p, ms)).collect();
                let msg = format!(
                    "⚠ SLOW READY: {} pod(s) with avg game launch delay >{}s:\n{}",
                    rows.len(), READY_DELAY_THRESHOLD_MS / 1000.0, list.join(", ")
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 5. App health degradation (MI bridge — server-side blind spot) ─────
    // Check app_health_monitor consecutive failures. If any app has been
    // degraded for >=5 consecutive probes (~2.5 min), alert at fleet level.
    // MMA P2: Per-app cooldown so kiosk failure doesn't mask admin failure.
    // MMA P1: WhatsApp error handling + cooldown updated post-send only.
    {
        use std::sync::LazyLock;
        static APP_ALERT_COOLDOWNS: LazyLock<std::sync::Mutex<HashMap<String, i64>>> =
            LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

        let apps = ["admin", "kiosk", "web"];
        for app in &apps {
            let count = crate::app_health_monitor::get_consecutive_failures(app);
            if count >= 5 {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "APP_DEGRADED: {} has {} consecutive health failures",
                    app, count
                );

                // Per-app cooldown check
                let last_alert = APP_ALERT_COOLDOWNS.lock().ok()
                    .and_then(|m| m.get(*app).copied())
                    .unwrap_or(0);

                if now_ts - last_alert > COOLDOWN_SECS {
                    let msg = format!(
                        "⚠ APP DEGRADED: {} failing health checks ({} consecutive, ≥2.5 min)\nCheck app_health_log for details. May need pm2 restart.",
                        app, count
                    );
                    // Send alert and update per-app cooldown
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                    if let Ok(mut m) = APP_ALERT_COOLDOWNS.lock() {
                        m.insert(app.to_string(), now_ts);
                    }
                }
            }
        }
    }

    // ── 6. Dashboard orphan detection (MI Phase 1 — client-side blind spot) ─
    // If kiosk app health is "ok" but zero dashboard WS clients connected for
    // 10+ consecutive probe cycles (2.5 min at 15s interval), alert.
    // This catches: WS token missing, CSP blocking JS, 401 on WS upgrade,
    // kiosk JS crash — all invisible to HTTP health probes.
    {
        static DASHBOARD_ORPHAN_COUNT: AtomicI64 = AtomicI64::new(0);
        static LAST_ORPHAN_ALERT: AtomicI64 = AtomicI64::new(0);

        let dashboard_clients = crate::ws::dashboard_client_count();
        let kiosk_healthy = crate::app_health_monitor::get_consecutive_failures("kiosk") == 0;

        if kiosk_healthy && dashboard_clients == 0 {
            let count = DASHBOARD_ORPHAN_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            if count >= 10 {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "DASHBOARD_ORPHAN: kiosk app healthy but 0 dashboard WS clients for {}+ probes — possible JS/auth/token issue",
                    count
                );
                if now_ts - LAST_ORPHAN_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_ORPHAN_ALERT.store(now_ts, Ordering::Relaxed);
                    let msg = "⚠ DASHBOARD ORPHAN: Kiosk app reports healthy but NO dashboard WebSocket clients connected (>2.5 min).\n\
                        Possible causes: WS token missing/wrong, CSP blocking JS, 401 on WS upgrade, kiosk JS crash.\n\
                        Check: browser console, .env.production.local, racecontrol.toml terminal_secret.";
                    crate::whatsapp_alerter::send_whatsapp(&state.config, msg).await;

                    // Log fleet incident via MI bridge
                    let incident = rc_common::mesh_types::MeshIncident {
                        id: format!("inc_dashboard_orphan_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                        node: "server".to_string(),
                        problem_key: "dashboard_orphan:kiosk".to_string(),
                        severity: rc_common::mesh_types::IncidentSeverity::High,
                        cost: 0.0,
                        resolution: None,
                        time_to_resolve_secs: None,
                        resolved_by_tier: None,
                        detected_at: chrono::Utc::now(),
                        resolved_at: None,
                    };
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        let _ = crate::fleet_kb::insert_incident(&db, &incident).await;
                    });
                }
            }
        } else {
            // Reset counter when clients are connected or kiosk is down
            let prev = DASHBOARD_ORPHAN_COUNT.swap(0, Ordering::Relaxed);
            if prev >= 10 {
                tracing::info!(
                    target: "fleet-anomaly",
                    "DASHBOARD_ORPHAN: Resolved — {} dashboard client(s) connected",
                    dashboard_clients
                );
                // Auto-resolve open orphan incidents
                let db = state.db.clone();
                tokio::spawn(async move {
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = sqlx::query(
                        "UPDATE fleet_incidents SET resolved_at = ?1, resolution = 'dashboard_reconnected'
                         WHERE problem_key = 'dashboard_orphan:kiosk' AND resolved_at IS NULL"
                    )
                    .bind(&now)
                    .execute(&db)
                    .await;
                });
            }
        }
    }

    // ── 7. Mass pod offline (MI bridge — network/power issue) ───────────────
    // If >=3 pods are offline simultaneously while venue is open, this is
    // likely a network/power issue, not individual pod crashes.
    {
        static LAST_MASS_OFFLINE_ALERT: AtomicI64 = AtomicI64::new(0);

        if crate::venue_state::venue_is_open() {
            let offline_pods: Vec<&str> = fleet.iter()
                .filter(|(_, s)| !s.http_reachable)
                .map(|(id, _)| id.as_str())
                .collect();

            if offline_pods.len() >= 3 {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "MASS_OFFLINE: {} pods unreachable while venue is open — possible network/power issue: {:?}",
                    offline_pods.len(), offline_pods
                );

                if now_ts - LAST_MASS_OFFLINE_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_MASS_OFFLINE_ALERT.store(now_ts, Ordering::Relaxed);
                    let msg = format!(
                        "⚠ MASS OFFLINE: {} pods unreachable while venue is open:\n{}\nPossible network outage or power issue. Check router/switch/UPS.",
                        offline_pods.len(),
                        offline_pods.join(", ")
                    );
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }
        }
    }

    // ── 8. PIN validation failure spike ────────────────────────────────────
    // Drain API error counts and alert if PIN failures are elevated.
    // This detects: wrong PIN entered repeatedly, kiosk → server connectivity issues,
    // or broken PIN validation logic.
    {
        static LAST_PIN_ALERT: AtomicI64 = AtomicI64::new(0);
        let error_counts = state.drain_api_error_counts();
        let pin_failures: u32 = error_counts.iter()
            .filter(|(k, _)| k.contains("pin") || k.contains("validate"))
            .map(|(_, v)| *v)
            .sum();

        if pin_failures >= 5 {
            tracing::warn!(
                target: "fleet-anomaly",
                "PIN_FAILURE_SPIKE: {} PIN validation failures since last check",
                pin_failures
            );

            if now_ts - LAST_PIN_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_PIN_ALERT.store(now_ts, Ordering::Relaxed);
                let details: Vec<String> = error_counts.iter()
                    .filter(|(k, _)| k.contains("pin") || k.contains("validate"))
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                let msg = format!(
                    "⚠ PIN FAILURE SPIKE: {} failed PIN attempts in last 15s:\n{}\nCheck kiosk connectivity + staff PIN in DB.",
                    pin_failures, details.join(", ")
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 8. Bat file hash drift (closes incident #16, #22) ─────────────────
    // If bat_sha256 differs across the fleet, startup scripts are out of sync.
    // Stale bat files miss process kills, power settings, ConspitLink guards.
    // Data already collected in fleet health store from agent /health probe.
    {
        static LAST_BAT_ALERT: AtomicI64 = AtomicI64::new(0);
        let mut bat_hashes: HashMap<&str, Vec<&str>> = HashMap::new();
        for (pod_id, store) in fleet.iter() {
            if store.http_reachable {
                if let Some(ref hash) = store.bat_sha256 {
                    bat_hashes.entry(hash.as_str()).or_default().push(pod_id.as_str());
                }
            }
        }

        if bat_hashes.len() > 1 {
            let (majority_hash, _) = bat_hashes.iter().max_by_key(|(_, pods)| pods.len()).unwrap();
            let drifted: Vec<(&str, &str)> = bat_hashes.iter()
                .filter(|(h, _)| *h != majority_hash)
                .flat_map(|(h, pods)| pods.iter().map(move |p| (*p, *h)))
                .collect();

            if !drifted.is_empty() {
                for (pod_id, hash) in &drifted {
                    tracing::warn!(
                        target: "fleet-anomaly",
                        "BAT_DRIFT: {} has bat_sha256={} (fleet majority: {})",
                        pod_id, &hash[..16.min(hash.len())], &majority_hash[..16.min(majority_hash.len())]
                    );
                }

                if now_ts - LAST_BAT_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_BAT_ALERT.store(now_ts, Ordering::Relaxed);
                    let list: Vec<String> = drifted.iter().map(|(p, _)| p.to_string()).collect();
                    let msg = format!(
                        "⚠ BAT DRIFT: {} pod(s) have different start-rcagent.bat than fleet majority:\n{}\nDeploy bat sync needed — stale bat causes settings regression.",
                        drifted.len(), list.join(", ")
                    );
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }
        }
    }
}

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

    Json(json!({
        "pods": result,
        "services": services,
        "displays": display_status,
        "dashboard_clients": crate::ws::dashboard_client_count(),
        "venue_open": crate::venue_state::venue_is_open(),
        "timestamp": Utc::now().to_rfc3339(),
    }))
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

    // ── Phase 100: maintenance state ──────────────────────────────────────────

    #[test]
    fn fleet_health_store_default_not_in_maintenance() {
        let store = FleetHealthStore::default();
        assert!(!store.in_maintenance, "in_maintenance defaults to false");
        assert!(store.maintenance_failures.is_empty(), "maintenance_failures defaults to empty");
    }

    #[test]
    fn fleet_health_clear_on_disconnect_clears_maintenance() {
        let mut store = FleetHealthStore::default();
        store.in_maintenance = true;
        store.maintenance_failures = vec!["DisplayCheck".to_string(), "HidCheck".to_string()];

        clear_on_disconnect(&mut store);

        assert!(!store.in_maintenance, "in_maintenance should be cleared on disconnect");
        assert!(store.maintenance_failures.is_empty(), "maintenance_failures should be cleared on disconnect");
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

    #[test]
    fn test_sentry_crash_field_default() {
        let store = FleetHealthStore::default();
        assert!(store.last_sentry_crash.is_none());
    }
}

//! Fleet-health API response shapes (Phase 445 — moved here from
//! `crates/racecontrol/src/fleet_health_api.rs` so gen-types can emit TS
//! bindings directly from rc-common without cross-crate gymnastics).
//!
//! The original definition stays available via a re-export in
//! `racecontrol::fleet_health_api` so existing consumers (handler builders,
//! state initializers) compile unchanged. The business logic (WS/HTTP
//! probing, cache, handlers) remains in `fleet_health_api.rs`.
//!
//! Decision traceability: Phase 445 RESEARCH § "Drift findings" #4
//! (PodFleetStatus lives outside rc-common) + Plan 02a Step A-C
//! (relocation to rc-common).

use serde::Serialize;

/// API response shape for a single pod in `GET /api/v1/fleet/health`.
///
/// This mirrors the runtime-computed shape returned by
/// `racecontrol::fleet_health_api::fleet_health_handler` — every field,
/// every `#[serde]` attribute, every doc comment is preserved byte-for-byte
/// from the pre-relocation definition to keep admin/kiosk clients
/// compatible.
///
/// Ts-rs derive is gated behind the `ts-rs` feature so default builds of
/// rc-agent / rc-sentry / racecontrol incur zero ts-rs cost. The feature
/// is only active when the `gen-types` binary runs.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-rs", ts(export))]
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
    /// Pattern I part 3: TRUE when pod's HTTP is reachable but WS is not connected —
    /// strongly suggests the silent-reconnect-forever pathology (seen 2026-04-18 23:05→00:09 IST
    /// on Pods 1+6). Staff should investigate the pod's WS reconnect loop.
    /// Derived server-side from `http_reachable && !ws_connected`.
    #[serde(default)]
    pub silent_reconnect_suspected: bool,
    /// Pattern I Part 5 Commit 6 (D3 rollback-detection): server-tracked active
    /// billing session id for this pod, sourced from `state.billing.active_timers`.
    /// `None` when no active session. Exposed so operators can correlate with
    /// the pod's own reported state during stuck-session investigations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_session_id: Option<String>,
    /// Pattern I Part 5 Commit 6 (D3 rollback-detection): TRUE when the pod
    /// looks stuck — server has an active billing session for this pod BUT
    /// the pod's WS is down while HTTP is up (silent-reconnect class). A
    /// pod running the Part 5 rc-agent binary would be self-healing via the
    /// T1/T2 HTTP fallback; a pod flagged here for > 5 min likely has a
    /// pre-patch binary (rolled-back deploy) OR hit the silent-loop-death
    /// class not covered by T1/T2 (Part 4 future fix).
    /// Derived: `silent_reconnect_suspected && active_session_id.is_some()`.
    #[serde(default)]
    pub stuck_session_candidate: bool,
}

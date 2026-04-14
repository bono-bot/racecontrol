//! Pod Healer: Self-healing daemon with AI diagnostics.
//!
//! Runs every 2 minutes (configurable). For each connected pod, collects deep
//! diagnostics via pod-agent `/exec`, applies safe rule-based fixes (kill zombie
//! sockets, clear temp files), and escalates complex/unfamiliar issues to AI
//! (Claude CLI -> Ollama -> Anthropic).
//!
//! rc-agent restarts are deferred to pod_monitor (which owns the shared backoff).
//! The healer reads the shared EscalatingBackoff from AppState.pod_backoffs for cooldown
//! gating but does NOT advance the backoff (advancing is pod_monitor's exclusive responsibility).
//!
//! ## Module structure (Phase 385, v49.0)
//!
//! - `pod_healer_diagnostics` — remote health data collection + verification chain
//! - `pod_healer_rules` — proactive 5-rule diagnostics for online pods
//! - `pod_healer_recovery` — graduated 5-step recovery for offline pods
//! - `pod_healer_ai` — AI escalation + WARN log surge scanner

use std::sync::Arc;
use std::time::Duration;

use crate::state::{AppState, WatchdogState};
use rc_common::types::{PodInfo, PodStatus};

// Re-exports used by tests (via `use super::*`)
#[cfg(test)]
pub(crate) use crate::pod_healer_ai::parse_ai_action_server;

// ─── Constants ───────────────────────────────────────────────────────────────

pub(crate) const POD_AGENT_PORT: u16 = 8090;
pub(crate) const POD_AGENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Processes that must NEVER be killed by the healer.
pub(crate) const PROTECTED_PROCESSES: &[&str] = &[
    "rc-agent.exe",
    "pod-agent.exe",
    "acs.exe",
    "conspitlink2.0.exe",
    "msedge.exe",
    "explorer.exe",
    "system",
    "svchost.exe",
    "csrss.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "dwm.exe",
    "taskhostw.exe",
    "conhost.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "vmsdesktop.exe",
    // James's machine runs as Pod 1 -- these are infrastructure, not suspicious
    "claude.exe",
    "ollama.exe",
    "ollama_llama_server.exe",
    "deskin.exe",
];

/// Ports we monitor for stale sockets.
pub(crate) const MONITORED_PORTS: &[&str] = &["18923", "18924"];

/// Disk usage threshold (percent used) to trigger temp cleanup.
pub(crate) const DISK_THRESHOLD_PCT: f64 = 90.0;

/// Memory threshold (MB free) to flag as low memory.
pub(crate) const MEMORY_LOW_MB: u64 = 2048;

// ─── Types ───────────────────────────────────────────────────────────────────

pub(crate) struct PodDiagnostics {
    pub(crate) stale_sockets: Vec<(u32, String)>, // (PID, state like CLOSE_WAIT)
    pub(crate) disk_free_pct: f64,
    pub(crate) memory_free_mb: u64,
    pub(crate) memory_total_mb: u64,
    pub(crate) rc_agent_healthy: bool,
    pub(crate) suspicious_processes: Vec<(String, u32, u64)>, // (name, PID, mem_kb)
}

pub(crate) struct HealAction {
    pub(crate) pod_id: String,
    pub(crate) action: String,
    pub(crate) target: String,
    pub(crate) reason: String,
}

// ─── Graduated Recovery Types ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PodRecoveryStep {
    /// First offline detection — waiting 30s before acting.
    Waiting,
    /// Second cycle — attempt Tier 1 rc-agent restart.
    TierOneRestart,
    /// Third cycle — context-aware WoL after Tier 1 fails.
    WakeOnLan,
    /// Fourth cycle — escalate to AI.
    AiEscalation,
    /// Fifth+ cycle — alert staff.
    AlertStaff,
}

/// Per-pod graduated recovery state. Held in a HashMap inside heal_all_pods.
/// Not shared with AppState — local to the healer loop.
#[derive(Debug)]
pub(crate) struct PodRecoveryTracker {
    pub(crate) step: PodRecoveryStep,
    pub(crate) first_detected_at: Option<std::time::Instant>,
    /// CONN-RESIL: Timestamp of last staff alert sent. Used to throttle re-alerts
    /// to every 15 minutes instead of every 2-minute healer cycle.
    pub(crate) last_staff_alert_at: Option<std::time::Instant>,
}

impl PodRecoveryTracker {
    pub(crate) fn new() -> Self {
        Self {
            step: PodRecoveryStep::Waiting,
            first_detected_at: None,
            last_staff_alert_at: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.step = PodRecoveryStep::Waiting;
        self.first_detected_at = None;
        self.last_staff_alert_at = None;
    }
}

impl Default for PodRecoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Spawn ───────────────────────────────────────────────────────────────────

/// Spawn the pod healer background task.
pub fn spawn(state: Arc<AppState>) {
    if !state.config.pods.healer_enabled {
        tracing::info!("Pod healer disabled");
        return;
    }

    let interval_secs = state.config.pods.healer_interval_secs as u64;

    tracing::info!(
        "Pod healer starting (interval: {}s, shared backoff via AppState)",
        interval_secs,
    );

    tokio::spawn(async move {
        // Wait for pods to connect before first scan
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        let mut recovery_trackers: std::collections::HashMap<String, PodRecoveryTracker> =
            std::collections::HashMap::new();

        loop {
            interval.tick().await;
            heal_all_pods(&state, &mut recovery_trackers).await;
        }
    });
}

// ─── Main Loop ───────────────────────────────────────────────────────────────

async fn heal_all_pods(
    state: &Arc<AppState>,
    trackers: &mut std::collections::HashMap<String, PodRecoveryTracker>,
) {
    // Check cascade guard before any recovery action
    {
        let guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_paused() {
            tracing::warn!(
                target: "pod_healer",
                "Recovery paused by cascade guard (remaining: {:?}), skipping heal cycle",
                guard.pause_remaining()
            );
            return;
        }
    }

    // Snapshot connected pods
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();

    let active_pods: Vec<&PodInfo> = pods
        .iter()
        .filter(|p| p.status != PodStatus::Disabled && p.last_seen.is_some())
        .collect();

    if active_pods.is_empty() {
        return;
    }

    tracing::info!("Pod healer: checking {} pods", active_pods.len());

    for pod in active_pods {
        if pod.status == PodStatus::Offline {
            // Offline pod: run graduated recovery instead of proactive diagnostics.
            crate::pod_healer_recovery::run_graduated_recovery(state, pod, trackers).await;
        } else {
            // Online pod: reset any graduated recovery tracker, then run proactive diagnostics.
            trackers.entry(pod.id.clone()).or_default().reset();
            if let Err(e) = crate::pod_healer_rules::heal_pod(state, pod).await {
                tracing::warn!("Pod healer: error checking pod {}: {}", pod.id, e);
            }
        }
    }

    // Phase 141: Scan server-side WARN log for surge detection
    crate::pod_healer_ai::scan_warn_logs(state).await;
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Check if a pod has an active billing session.
pub(crate) async fn has_active_billing(state: &Arc<AppState>, pod_id: &str) -> bool {
    let timers = state.billing.active_timers.read().await;
    timers.contains_key(pod_id)
}

/// Returns true if the pod is currently in a watchdog recovery cycle (Restarting or Verifying).
/// A second bot task must not act on this pod while recovery is in progress.
///
/// Note: RecoveryFailed means the watchdog has given up — bots may still attempt fixes.
pub fn is_pod_in_recovery(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}

/// Pure helper: given a WatchdogState, return true if the healer should skip diagnostics.
/// This is extracted for testability — heal_pod() calls this to decide whether to return early.
pub(crate) fn should_skip_for_watchdog_state(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}

#[cfg(test)]
#[path = "pod_healer_tests.rs"]
mod tests;

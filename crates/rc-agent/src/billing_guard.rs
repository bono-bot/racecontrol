//! Billing Guard — polls FailureMonitorState every 5s, detects billing anomalies.
//!
//! Detection rules:
//!   BILL-02: billing_active=true + game_pid=None for >= 60s → BillingAnomaly(BillingStuckSession)
//!   BILL-03: billing_active=true + driving_state not Active for >= 300s → BillingAnomaly(IdleDriftDetected)
//!   SESSION-01: billing_active=true + game_pid=None + !billing_paused for >= orphan_end_threshold_secs → orphan auto-end via HTTP
//!
//! Sends AgentMessage::BillingAnomaly. For orphan auto-end, calls HTTP POST to server directly.
//! recovery_in_progress=true suppresses all anomaly sends.
//! billing_paused=true suppresses anomaly sends and orphan auto-end (crash recovery in progress).

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::sync::{mpsc, watch, RwLock};
use rc_common::protocol::AgentMessage;
use rc_common::types::{DrivingState, PodFailureReason};
use crate::failure_monitor::FailureMonitorState;
use crate::feature_flags::FeatureFlags;

const LOG_TARGET: &str = "billing";
const POLL_INTERVAL_SECS: u64 = 5;
const STUCK_SESSION_THRESHOLD_SECS: u64 = 60;
const IDLE_DRIFT_THRESHOLD_SECS: u64 = 300;

#[cfg(feature = "http-client")]
static ORPHAN_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[cfg(feature = "http-client")]
fn orphan_client() -> &'static reqwest::Client {
    ORPHAN_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("orphan HTTP client build failed")
    })
}

async fn attempt_orphan_end(core_base_url: &str, session_id: &str, end_reason: &str) -> bool {
    #[cfg(feature = "http-client")]
    {
    let client = orphan_client();
    let url = format!("{}/billing/session/{}/end?reason={}", core_base_url, session_id, end_reason);
    match client.post(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "orphan end HTTP failed: {}", e);
            false
        }
    }
    }
    #[cfg(not(feature = "http-client"))]
    {
        let _ = (core_base_url, session_id, end_reason);
        false
    }
}

pub fn spawn(
    state_rx: watch::Receiver<FailureMonitorState>,
    agent_msg_tx: mpsc::Sender<AgentMessage>,
    pod_id: String,
    core_base_url: String,           // HTTP base URL e.g. "http://192.168.31.23:8080/api/v1"
    orphan_end_threshold_secs: u64,  // From config.auto_end_orphan_session_secs (default 300)
    flags: Arc<RwLock<FeatureFlags>>, // v22.0 Phase 178: feature flag access
) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "Billing guard task started (poll interval: {}s, orphan_threshold: {}s)", POLL_INTERVAL_SECS, orphan_end_threshold_secs);
        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
        // Task-local debounce state (same pattern as launch_timeout_fired in failure_monitor)
        let mut stuck_fired = false;
        let mut game_gone_since: Option<tokio::time::Instant> = None;
        let mut idle_fired = false;
        let mut idle_since: Option<tokio::time::Instant> = None;
        let mut orphan_fired = false;

        loop {
            interval.tick().await;

            // v22.0 Phase 178: Feature flag gate — skip billing guard if disabled
            {
                let ff = flags.read().await;
                if !ff.flag_enabled("billing_guard") {
                    continue; // Skip this tick entirely — billing guard feature flag is off
                }
            }

            let state = state_rx.borrow().clone();

            // Global suppression: server-initiated recovery in progress
            if state.recovery_in_progress {
                // Reset timers so they don't fire the moment recovery clears
                game_gone_since = None;
                stuck_fired = false;
                orphan_fired = false;
                idle_since = None;
                idle_fired = false;
                continue;
            }

            // SESSION-01: billing_paused suppresses anomalies during crash recovery
            if state.billing_paused {
                // Billing legitimately paused during crash recovery — suppress anomalies
                game_gone_since = None;
                stuck_fired = false;
                orphan_fired = false;
                idle_since = None;
                idle_fired = false;
                continue;
            }

            // BILL-02: Stuck session detection
            if state.billing_active && state.game_pid.is_none() {
                let since = game_gone_since.get_or_insert_with(tokio::time::Instant::now);
                if since.elapsed() >= Duration::from_secs(STUCK_SESSION_THRESHOLD_SECS) && !stuck_fired {
                    stuck_fired = true;
                    let msg = AgentMessage::BillingAnomaly {
                        pod_id: pod_id.clone(),
                        billing_session_id: state.active_billing_session_id.clone().unwrap_or_else(|| "unknown".to_string()),
                        reason: PodFailureReason::SessionStuckWaitingForGame,
                        detail: format!(
                            "game_pid=None for {}s while billing active",
                            since.elapsed().as_secs()
                        ),
                    };
                    let _ = agent_msg_tx.try_send(msg);
                    tracing::warn!(target: LOG_TARGET, "pod={} BillingStuckSession anomaly sent", pod_id);
                }

                // SESSION-01: Orphan auto-end escalation at configurable threshold (default 300s)
                // Uses same game_gone_since timer as BILL-02 (60s), fires second tier
                if since.elapsed() >= Duration::from_secs(orphan_end_threshold_secs) && !orphan_fired {
                    orphan_fired = true;
                    if let Some(ref session_id) = state.active_billing_session_id {
                        tracing::warn!(
                            target: LOG_TARGET,
                            "pod={} ORPHAN auto-end: billing active {}s with no game (threshold={}s) — ending session {}",
                            pod_id, since.elapsed().as_secs(), orphan_end_threshold_secs, session_id
                        );
                        let session_id_clone = session_id.clone();
                        let base_url = core_base_url.clone();
                        let pod_id_clone = pod_id.clone();
                        let tx = agent_msg_tx.clone();

                        // Retry loop: 3 attempts with backoff [5s, 15s, 30s]
                        tokio::spawn(async move {
                            let delays = [5u64, 15, 30];
                            let mut succeeded = false;
                            for (i, delay) in delays.iter().enumerate() {
                                if attempt_orphan_end(&base_url, &session_id_clone, "orphan_timeout").await {
                                    succeeded = true;
                                    tracing::info!(target: LOG_TARGET, "Orphan session {} ended successfully on attempt {}", session_id_clone, i + 1);
                                    break;
                                }
                                tracing::warn!(target: LOG_TARGET, "Orphan end attempt {} failed, retrying in {}s", i + 1, delay);
                                tokio::time::sleep(Duration::from_secs(*delay)).await;
                            }
                            // Send WS notification regardless of HTTP outcome
                            let msg = AgentMessage::SessionAutoEnded {
                                pod_id: pod_id_clone,
                                billing_session_id: session_id_clone.clone(),
                                reason: "orphan_timeout".to_string(),
                            };
                            let _ = tx.try_send(msg);
                            if !succeeded {
                                tracing::error!(target: LOG_TARGET, "All 3 orphan end attempts failed — SessionAutoEnded sent but billing may be stale on server");
                            }
                        });
                    } else {
                        tracing::warn!(target: LOG_TARGET, "pod={} Orphan detected but no session_id in FailureMonitorState — cannot auto-end", pod_id);
                    }
                }
            } else {
                // Condition cleared — reset
                game_gone_since = None;
                stuck_fired = false;
                orphan_fired = false;
            }

            // BILL-03: Idle drift detection
            let is_driving_active = matches!(state.driving_state, Some(DrivingState::Active));
            if state.billing_active && !is_driving_active {
                let since = idle_since.get_or_insert_with(tokio::time::Instant::now);
                if since.elapsed() >= Duration::from_secs(IDLE_DRIFT_THRESHOLD_SECS) && !idle_fired {
                    idle_fired = true;
                    let msg = AgentMessage::BillingAnomaly {
                        pod_id: pod_id.clone(),
                        billing_session_id: state.active_billing_session_id.clone().unwrap_or_else(|| "unknown".to_string()),
                        reason: PodFailureReason::IdleBillingDrift,
                        detail: format!(
                            "DrivingState not Active for {}s while billing active (state={:?})",
                            since.elapsed().as_secs(),
                            state.driving_state
                        ),
                    };
                    let _ = agent_msg_tx.try_send(msg);
                    tracing::warn!(target: LOG_TARGET, "pod={} IdleDriftDetected anomaly sent", pod_id);
                }
            } else {
                idle_since = None;
                idle_fired = false;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::{mpsc, watch};
    use rc_common::protocol::AgentMessage;
    use rc_common::types::{DrivingState, PodFailureReason};
    use crate::failure_monitor::FailureMonitorState;

    fn make_state(
        billing_active: bool,
        game_pid: Option<u32>,
        driving_state: Option<DrivingState>,
        recovery: bool,
    ) -> FailureMonitorState {
        FailureMonitorState {
            billing_active,
            game_pid,
            driving_state,
            recovery_in_progress: recovery,
            hid_connected: false,
            last_udp_secs_ago: None,
            launch_started_at: None,
            billing_paused: false,
            active_billing_session_id: None,
            sim_type: None,
        }
    }

    #[test]
    fn stuck_session_condition_requires_billing_and_no_pid() {
        let state = make_state(true, None, None, false);
        assert!(state.billing_active && state.game_pid.is_none());
    }

    #[test]
    fn no_stuck_session_when_billing_inactive() {
        let state = make_state(false, None, None, false);
        assert!(!(state.billing_active && state.game_pid.is_none()));
    }

    #[test]
    fn no_stuck_session_when_game_running() {
        let state = make_state(true, Some(1234), None, false);
        assert!(!(state.billing_active && state.game_pid.is_none()));
    }

    #[test]
    fn idle_drift_condition_driving_inactive() {
        let state = make_state(true, Some(1234), Some(DrivingState::Idle), false);
        let is_active = matches!(state.driving_state, Some(DrivingState::Active));
        assert!(state.billing_active && !is_active);
    }

    #[test]
    fn idle_drift_suppressed_when_recovery_in_progress() {
        let state = make_state(true, None, Some(DrivingState::Idle), true);
        assert!(state.recovery_in_progress, "recovery_in_progress must suppress detection");
    }

    #[test]
    fn stuck_threshold_is_60s() {
        assert_eq!(STUCK_SESSION_THRESHOLD_SECS, 60);
    }

    #[test]
    fn idle_threshold_is_300s() {
        assert_eq!(IDLE_DRIFT_THRESHOLD_SECS, 300);
    }

    // ── Phase 49 Plan 01: Orphan auto-end tests ───────────────────────────

    #[test]
    fn default_orphan_threshold_is_300s() {
        assert_eq!(crate::config::default_auto_end_orphan_session_secs(), 300);
    }

    #[test]
    fn orphan_condition_requires_billing_and_no_pid_and_not_paused() {
        let state = make_state(true, None, None, false);
        // Orphan fires when: billing_active && game_pid.is_none() && !billing_paused
        assert!(state.billing_active && state.game_pid.is_none() && !state.billing_paused,
            "Orphan condition must require billing_active + no pid + not paused");
    }

    #[test]
    fn orphan_suppressed_when_billing_paused() {
        let mut state = make_state(true, None, None, false);
        state.billing_paused = true;
        // Orphan must NOT fire when billing_paused=true (crash recovery in progress)
        let should_fire = state.billing_active && state.game_pid.is_none() && !state.billing_paused;
        assert!(!should_fire, "Orphan must be suppressed when billing_paused=true");
    }

    #[test]
    fn orphan_suppressed_when_recovery_in_progress() {
        let state = make_state(true, None, None, true); // recovery=true
        assert!(state.recovery_in_progress,
            "recovery_in_progress must suppress all anomaly detection including orphan");
    }

    // ── Timer + channel tests (TEST-01) — verify AgentMessage sends via tokio::time ──
    //
    // Pattern: tokio::time::pause() freezes the mock clock. The spawned task must be
    // initially polled before any advance() so it can start and register its interval.
    // We yield several times first (to let the task start), then advance(5) to fire the
    // first interval tick (recording game_gone_since/idle_since), then advance past the
    // threshold to trigger the anomaly send.

    #[tokio::test]
    async fn bill02_anomaly_fires_after_60s() {
        tokio::time::pause();

        let initial_state = FailureMonitorState {
            billing_active: true,
            game_pid: None,
            ..FailureMonitorState::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (msg_tx, mut msg_rx) = mpsc::channel::<AgentMessage>(16);
        let _ = state_tx; // keep sender alive

        spawn(state_rx, msg_tx, "pod_test".to_string(), "http://unused".to_string(), 9999,
              Arc::new(RwLock::new(FeatureFlags::new())));

        // Yield to let the spawned task start and block on interval.tick()
        for _ in 0..5 { tokio::task::yield_now().await; }
        // Advance one poll interval: first tick fires, task records game_gone_since
        tokio::time::advance(Duration::from_secs(5)).await;
        for _ in 0..5 { tokio::task::yield_now().await; }
        // Advance past 60s threshold: elapsed = 65s >= 60s → anomaly fires
        tokio::time::advance(Duration::from_secs(65)).await;
        for _ in 0..15 { tokio::task::yield_now().await; }

        match msg_rx.try_recv() {
            Ok(AgentMessage::BillingAnomaly { reason, .. }) => {
                assert_eq!(reason, PodFailureReason::SessionStuckWaitingForGame,
                    "BILL-02: expected SessionStuckWaitingForGame");
            }
            other => panic!("Expected BillingAnomaly(SessionStuckWaitingForGame), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn bill02_does_not_fire_before_threshold() {
        tokio::time::pause();

        let initial_state = FailureMonitorState {
            billing_active: true,
            game_pid: None,
            ..FailureMonitorState::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (msg_tx, mut msg_rx) = mpsc::channel::<AgentMessage>(16);
        let _ = state_tx;

        spawn(state_rx, msg_tx, "pod_test".to_string(), "http://unused".to_string(), 9999,
              Arc::new(RwLock::new(FeatureFlags::new())));

        // Let task start, record game_gone_since
        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(5)).await;
        for _ in 0..5 { tokio::task::yield_now().await; }
        // Advance only 50s more — total elapsed since game_gone_since = 50s < 60s threshold
        tokio::time::advance(Duration::from_secs(50)).await;
        for _ in 0..10 { tokio::task::yield_now().await; }

        assert!(msg_rx.try_recv().is_err(),
            "BILL-02 must NOT fire before 60s threshold");
    }

    #[tokio::test]
    async fn bill02_suppressed_when_recovery_in_progress() {
        tokio::time::pause();

        let initial_state = FailureMonitorState {
            billing_active: true,
            game_pid: None,
            recovery_in_progress: true,
            ..FailureMonitorState::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (msg_tx, mut msg_rx) = mpsc::channel::<AgentMessage>(16);
        let _ = state_tx;

        spawn(state_rx, msg_tx, "pod_test".to_string(), "http://unused".to_string(), 9999,
              Arc::new(RwLock::new(FeatureFlags::new())));

        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(5)).await;
        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(65)).await;
        for _ in 0..15 { tokio::task::yield_now().await; }

        assert!(msg_rx.try_recv().is_err(),
            "BILL-02 must be suppressed when recovery_in_progress=true");
    }

    #[tokio::test]
    async fn bill02_suppressed_when_billing_paused() {
        tokio::time::pause();

        let initial_state = FailureMonitorState {
            billing_active: true,
            game_pid: None,
            billing_paused: true,
            ..FailureMonitorState::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (msg_tx, mut msg_rx) = mpsc::channel::<AgentMessage>(16);
        let _ = state_tx;

        spawn(state_rx, msg_tx, "pod_test".to_string(), "http://unused".to_string(), 9999,
              Arc::new(RwLock::new(FeatureFlags::new())));

        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(5)).await;
        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(65)).await;
        for _ in 0..15 { tokio::task::yield_now().await; }

        assert!(msg_rx.try_recv().is_err(),
            "BILL-02 must be suppressed when billing_paused=true (SESSION-01)");
    }

    #[tokio::test]
    async fn bill03_idle_drift_fires_after_300s() {
        tokio::time::pause();

        let initial_state = FailureMonitorState {
            billing_active: true,
            game_pid: Some(1234),
            driving_state: Some(DrivingState::Idle),
            ..FailureMonitorState::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (msg_tx, mut msg_rx) = mpsc::channel::<AgentMessage>(16);
        let _ = state_tx;

        spawn(state_rx, msg_tx, "pod_test".to_string(), "http://unused".to_string(), 9999,
              Arc::new(RwLock::new(FeatureFlags::new())));

        // Let task start, record idle_since
        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(5)).await;
        for _ in 0..5 { tokio::task::yield_now().await; }
        // Advance past 300s threshold: elapsed = 300s >= 300s → anomaly fires
        tokio::time::advance(Duration::from_secs(300)).await;
        for _ in 0..15 { tokio::task::yield_now().await; }

        match msg_rx.try_recv() {
            Ok(AgentMessage::BillingAnomaly { reason, .. }) => {
                assert_eq!(reason, PodFailureReason::IdleBillingDrift,
                    "BILL-03: expected IdleBillingDrift");
            }
            other => panic!("Expected BillingAnomaly(IdleBillingDrift), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn bill03_no_drift_when_driving_active() {
        tokio::time::pause();

        let initial_state = FailureMonitorState {
            billing_active: true,
            game_pid: Some(1234),
            driving_state: Some(DrivingState::Active),
            ..FailureMonitorState::default()
        };
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (msg_tx, mut msg_rx) = mpsc::channel::<AgentMessage>(16);
        let _ = state_tx;

        spawn(state_rx, msg_tx, "pod_test".to_string(), "http://unused".to_string(), 9999,
              Arc::new(RwLock::new(FeatureFlags::new())));

        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(5)).await;
        for _ in 0..5 { tokio::task::yield_now().await; }
        tokio::time::advance(Duration::from_secs(300)).await;
        for _ in 0..15 { tokio::task::yield_now().await; }

        assert!(msg_rx.try_recv().is_err(),
            "BILL-03 must NOT fire when DrivingState is Active");
    }
}

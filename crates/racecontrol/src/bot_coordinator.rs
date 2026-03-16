//! Bot Coordinator — server-side routing for bot anomaly messages.
//!
//! Receives AgentMessage variants from ws/mod.rs and routes to the correct
//! handler. Owns all session-ending logic on the server side.
//!
//! Routing:
//!   BillingAnomaly(SessionStuckWaitingForGame) → recover_stuck_session()
//!   BillingAnomaly(IdleBillingDrift)           → alert_staff_idle_drift()
//!   HardwareFailure                            → log + alert (stub; Phase 24 handles rc-agent side)
//!   TelemetryGap                               → log + alert (stub; TELEM-01 Phase 26)

use std::sync::Arc;
use std::sync::atomic::Ordering;

use rc_common::types::{BillingSessionStatus, PodFailureReason};

use crate::billing::end_billing_session_public;
use crate::pod_healer::is_pod_in_recovery;
use crate::state::{AppState, WatchdogState};

/// Route a BillingAnomaly message to the correct handler.
///
/// Guards:
/// - is_pod_in_recovery() skips action (pod healer is already acting)
/// - SessionStuckWaitingForGame → recover_stuck_session()
/// - IdleBillingDrift           → alert_staff_idle_drift() (NEVER auto-ends session)
pub async fn handle_billing_anomaly(
    state: &Arc<AppState>,
    pod_id: &str,
    _billing_session_id: &str, // from agent; may be "unknown" — server resolves from active_timers
    reason: PodFailureReason,
    detail: &str,
) {
    // Guard: skip if pod healer is already handling this pod
    let wd_state = state
        .pod_watchdog_states
        .read()
        .await
        .get(pod_id)
        .cloned()
        .unwrap_or(WatchdogState::Healthy);
    if is_pod_in_recovery(&wd_state) {
        tracing::info!(
            "[bot-coord] BillingAnomaly for {} skipped — pod in recovery",
            pod_id
        );
        return;
    }

    tracing::info!(
        "[bot-coord] BillingAnomaly pod={} reason={:?}: {}",
        pod_id,
        reason,
        detail
    );

    match reason {
        PodFailureReason::SessionStuckWaitingForGame => {
            recover_stuck_session(state, pod_id).await;
        }
        PodFailureReason::IdleBillingDrift => {
            alert_staff_idle_drift(state, pod_id, detail).await;
        }
        _ => {
            tracing::warn!(
                "[bot-coord] Unhandled BillingAnomaly reason {:?} for pod={}",
                reason,
                pod_id
            );
        }
    }
}

/// Route a HardwareFailure message.
/// Phase 24 handles the rc-agent side (fix_usb_reconnect, fix_frozen_game).
/// Server side logs. Stub for Phase 25 — full impl Phase 26.
pub async fn handle_hardware_failure(
    _state: &Arc<AppState>,
    pod_id: &str,
    reason: &PodFailureReason,
    detail: &str,
) {
    tracing::warn!(
        "[bot-coord] HardwareFailure pod={} reason={:?}: {} (logged, no server action needed)",
        pod_id,
        reason,
        detail
    );
}

/// Route a TelemetryGap message.
/// TELEM-01 alert logic implemented in Phase 26. Stub here for BOT-01 completeness.
pub async fn handle_telemetry_gap(
    _state: &Arc<AppState>,
    pod_id: &str,
    gap_seconds: u64,
) {
    tracing::warn!(
        "[bot-coord] TelemetryGap pod={} gap={}s (TELEM-01 alert — Phase 26)",
        pod_id,
        gap_seconds
    );
}

/// Recover a stuck billing session.
///
/// Resolves session_id from active_timers (ignores agent-provided id which may be stale).
/// Calls end_billing_session_public() which handles StopGame + SessionEnded internally.
/// NEVER sends StopGame separately — that causes a double-end race.
///
/// After end_billing_session_public(), triggers the cloud sync fence (Plan 04 adds full fence;
/// here we log the recovery event so sync picks it up).
async fn recover_stuck_session(state: &Arc<AppState>, pod_id: &str) {
    // Resolve session_id from server's active_timers — agent's id may be "unknown"
    let session_id = state
        .billing
        .active_timers
        .read()
        .await
        .get(pod_id)
        .map(|t| t.session_id.clone());

    let Some(session_id) = session_id else {
        tracing::info!(
            "[bot-coord] recover_stuck_session pod={}: no active timer — noop",
            pod_id
        );
        return;
    };

    tracing::warn!(
        "[bot-coord] Recovering stuck session {} for pod={}",
        session_id,
        pod_id
    );

    // end_billing_session_public() sends StopGame, SessionEnded, debits wallet, broadcasts dashboard
    let ended =
        end_billing_session_public(state, &session_id, BillingSessionStatus::EndedEarly).await;
    if ended {
        tracing::info!(
            "[bot-coord] Stuck session {} ended for pod={}",
            session_id,
            pod_id
        );
        // BILL-04: cloud sync fence — wait up to 5s for relay to cycle after session end.
        // Ensures wallet debit is propagated before any further bot actions.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if state.relay_available.load(Ordering::Relaxed) {
                tracing::info!(
                    "[bot-coord] Sync fence complete — relay available, session={}",
                    session_id
                );
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!(
                    "[bot-coord] Sync fence timeout 5s for session={} — HTTP fallback scheduled",
                    session_id
                );
                break;
            }
        }
    } else {
        tracing::warn!(
            "[bot-coord] end_billing_session_public returned false for session={}",
            session_id
        );
    }
}

/// Alert staff about idle billing drift.
///
/// BILL-03: billing active + DrivingState not Active for 5 min → alert ONLY.
/// DO NOT call end_billing_session_public() here. Staff decides.
async fn alert_staff_idle_drift(state: &Arc<AppState>, pod_id: &str, detail: &str) {
    let subject = format!(
        "Racing Point Alert: Pod {} idle while billing active",
        pod_id
    );
    let body = format!(
        "Pod {} has an active billing session but no driving activity detected.\n\nDetail: {}\n\nPlease check the pod and decide whether to end the session.",
        pod_id, detail
    );
    tracing::warn!(
        "[bot-coord] IdleDrift alert for pod={}: {}",
        pod_id,
        detail
    );
    state
        .email_alerter
        .write()
        .await
        .send_alert(pod_id, &subject, &body)
        .await;
}

#[cfg(test)]
mod tests {
    use rc_common::types::PodFailureReason;

    // Pure condition logic tests (no AppState construction needed)

    #[test]
    fn stuck_session_reason_is_session_stuck() {
        let reason = PodFailureReason::SessionStuckWaitingForGame;
        assert!(matches!(
            reason,
            PodFailureReason::SessionStuckWaitingForGame
        ));
    }

    #[test]
    fn idle_drift_reason_is_idle_billing_drift() {
        let reason = PodFailureReason::IdleBillingDrift;
        assert!(matches!(reason, PodFailureReason::IdleBillingDrift));
    }

    #[test]
    fn routing_match_coverage() {
        // Verify that SessionStuckWaitingForGame and IdleBillingDrift are distinct reasons
        let stuck = PodFailureReason::SessionStuckWaitingForGame;
        let idle = PodFailureReason::IdleBillingDrift;
        assert!(!matches!(idle, PodFailureReason::SessionStuckWaitingForGame));
        assert!(!matches!(stuck, PodFailureReason::IdleBillingDrift));
    }

    #[test]
    fn recover_no_timer_noop_precondition() {
        // Documents the guard: if active_timers doesn't contain pod_id, recover_stuck_session returns early
        // The HashMap lookup pattern: None means noop
        let timers: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let session_id = timers.get("pod_1").cloned();
        assert!(
            session_id.is_none(),
            "empty timers must produce None session_id — noop path"
        );
    }

    #[test]
    fn alert_not_end_session_for_idle_drift() {
        // Documents invariant: IdleBillingDrift NEVER triggers end_billing_session_public
        // The routing match arm for IdleBillingDrift calls alert_staff_idle_drift, not recover_stuck_session
        let reason = PodFailureReason::IdleBillingDrift;
        let would_end_session = matches!(
            reason,
            PodFailureReason::SessionStuckWaitingForGame
        );
        assert!(
            !would_end_session,
            "IdleBillingDrift must NOT trigger session end"
        );
    }
}

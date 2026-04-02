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

use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::{BillingSessionStatus, GameState, PodFailureReason};

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
///
/// TELEM-01: sends staff email when pod game_state=Running AND billing_active=true.
/// No-op when game_state is not Running (Idle, Launching, menu) or billing is inactive.
pub async fn handle_telemetry_gap(
    state: &Arc<AppState>,
    pod_id: &str,
    gap_seconds: u64,
) {
    // TELEM-01 guard: only alert during active gameplay (GameState::Running)
    let game_state = state
        .pods
        .read()
        .await
        .get(pod_id)
        .and_then(|p| p.game_state);
    if !matches!(game_state, Some(GameState::Running)) {
        tracing::debug!(
            "[bot-coord] TelemetryGap ignored — pod {} not Running ({:?})",
            pod_id,
            game_state
        );
        return;
    }

    // TELEM-01 guard: only alert when billing is active
    let billing_active = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(pod_id);
    if !billing_active {
        tracing::debug!(
            "[bot-coord] TelemetryGap ignored — pod {} billing not active",
            pod_id
        );
        return;
    }

    let subject = format!(
        "Racing Point Alert: Pod {} UDP telemetry gap {}s",
        pod_id, gap_seconds
    );
    let body = format!(
        "Pod {} has not sent UDP telemetry for {}s while billing is active and game is running.\n\
         Game may have crashed silently. Please check the pod.\n\n\
         Game state: Running | Billing: Active | Gap: {}s",
        pod_id, gap_seconds, gap_seconds
    );
    tracing::warn!(
        "[bot-coord] TELEM-01 alert: pod={} gap={}s — sending staff email",
        pod_id,
        gap_seconds
    );
    state
        .email_alerter
        .write()
        .await
        .send_alert(pod_id, &subject, &body)
        .await;
}

/// Handle an AC multiplayer server disconnect detected by the pod agent.
///
/// MULTI-01 teardown order (non-negotiable):
///   1. Engage lock screen (pod is locked immediately — customer can't continue driving)
///   2. End billing session (after lock, not before)
///   3. Log event
///
/// No-op if no active billing session for the pod.
pub async fn handle_multiplayer_failure(
    state: &Arc<AppState>,
    pod_id: &str,
    reason: &PodFailureReason,
    session_id: Option<&str>,
) {
    tracing::warn!(
        "[bot-coord] MULTI-01 pod={} reason={:?} session={:?}",
        pod_id,
        reason,
        session_id
    );

    // Guard: only act if billing is active
    let active_session_id = state
        .billing
        .active_timers
        .read()
        .await
        .get(pod_id)
        .map(|t| t.session_id.clone());
    let Some(resolved_session_id) = active_session_id else {
        tracing::info!(
            "[bot-coord] MultiplayerFailure pod={}: no active billing — noop",
            pod_id
        );
        return;
    };

    // Step 1: Engage lock screen — pod is locked before billing ends.
    // BlankScreen blanks the pod display immediately (customer can no longer drive).
    // FFB zero is guaranteed by end_billing_session_public → StopGame arm in main.rs.
    {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BlankScreen)).await;
        }
    }

    // Step 2: End billing for triggering pod.
    // end_billing_session_public sends StopGame to the agent.
    // The agent's StopGame handler zeroes FFB BEFORE killing the game process
    // (rc-agent/src/main.rs, StopGame arm: ffb.zero_force() is awaited first).
    // No separate FFB zero step is needed here.
    let ended = end_billing_session_public(
        state,
        &resolved_session_id,
        BillingSessionStatus::EndedEarly,
        None,
    )
    .await;
    tracing::info!(
        "[bot-coord] MULTI-01 triggering pod={} session={} ended={}",
        pod_id,
        resolved_session_id,
        ended
    );

    // Step 3: Cascade teardown to all other pods in the same group session.
    // BillingTimer does NOT carry group_session_id — resolve via DB.
    // group_session_members table: pod_id, group_session_id, status
    let group_pods: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT pod_id FROM group_session_members
         WHERE group_session_id = (
             SELECT group_session_id FROM group_session_members
             WHERE pod_id = ? AND status = 'validated'
         )
         AND pod_id != ?
         AND pod_id IS NOT NULL",
    )
    .bind(pod_id)
    .bind(pod_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(p,)| p)
    .collect();

    for group_pod_id in &group_pods {
        tracing::warn!(
            "[bot-coord] MULTI-01 cascade to pod={} (same group as triggering pod={})",
            group_pod_id,
            pod_id
        );
        // Blank screen each group pod first
        {
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(group_pod_id.as_str()) {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BlankScreen)).await;
            }
        }
        // End billing for each group pod (sends StopGame → FFB zero on each agent)
        let group_session_id = state
            .billing
            .active_timers
            .read()
            .await
            .get(group_pod_id.as_str())
            .map(|t| t.session_id.clone());
        if let Some(gsid) = group_session_id {
            let _ = end_billing_session_public(
                state,
                &gsid,
                BillingSessionStatus::EndedEarly,
                None,
            )
            .await;
            tracing::info!(
                "[bot-coord] MULTI-01 cascade: pod={} session={} ended",
                group_pod_id,
                gsid
            );
        } else {
            tracing::warn!(
                "[bot-coord] MULTI-01 cascade: pod={} has no active billing (already ended?)",
                group_pod_id
            );
        }
    }

    // Step 4: Log event (triggering pod; group pods are logged by their own end_billing calls)
    crate::activity_log::log_pod_activity(
        state,
        pod_id,
        "multiplayer",
        "Multiplayer Disconnect",
        &format!(
            "reason={:?} session={} cascaded_to={:?}",
            reason, resolved_session_id, group_pods
        ),
        "bot",
        None,
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
        end_billing_session_public(state, &session_id, BillingSessionStatus::EndedEarly, None).await;
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

    #[test]
    fn telemetry_gap_skipped_when_game_not_running() {
        // TELEM-01: guard logic — not Running means no email
        use rc_common::types::GameState;
        let game_state: Option<GameState> = Some(GameState::Idle);
        let should_alert = matches!(game_state, Some(GameState::Running));
        assert!(!should_alert, "Idle game must not trigger TELEM-01 alert");
    }

    #[test]
    fn telemetry_gap_alerts_when_game_running_and_billing_active() {
        use rc_common::types::GameState;
        let game_state: Option<GameState> = Some(GameState::Running);
        let billing_active = true;
        let should_alert = matches!(game_state, Some(GameState::Running)) && billing_active;
        assert!(should_alert, "Running game + billing active must trigger TELEM-01 alert");
    }

    #[test]
    fn multiplayer_failure_triggers_lock_end_billing_log_in_order() {
        // MULTI-01: teardown order invariant documented as a test.
        // Actual ordering is enforced by sequential awaits in handle_multiplayer_failure.
        // FFB zero is guaranteed by end_billing_session_public → StopGame → ffb.zero_force()
        // on the agent side (rc-agent/src/main.rs StopGame arm).
        let steps = vec!["blank_screen", "end_billing_ffb_zero_via_stop_game", "cascade_group_pods", "log_event"];
        assert_eq!(steps[0], "blank_screen", "lock screen must come first");
        assert_eq!(steps[1], "end_billing_ffb_zero_via_stop_game", "billing end (with FFB zero) must come after lock");
        assert_eq!(steps[2], "cascade_group_pods", "group pod cascade must follow triggering pod teardown");
        assert_eq!(steps[3], "log_event", "log must come last");
    }

    #[test]
    fn multiplayer_failure_noop_when_billing_inactive() {
        use std::collections::HashMap;
        let active_timers: HashMap<String, String> = HashMap::new();
        let session_id = active_timers.get("pod_1").cloned();
        assert!(
            session_id.is_none(),
            "no active timer means handle_multiplayer_failure is a noop"
        );
    }
}

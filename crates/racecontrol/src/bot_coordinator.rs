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

/// Phase 364: Atomically append a reason string to billing_sessions.suspect_reasons.
/// Uses SQLite json_insert('$[#]') to avoid read-modify-write race with
/// run_session_audit() (Phase 363). Also sets suspect=1.
/// No-op if session_id is empty or DB write fails (advisory only).
pub async fn append_suspect_reason(
    db: &sqlx::SqlitePool,
    session_id: &str,
    reason: &str,
) {
    if session_id.is_empty() {
        return;
    }
    let result = sqlx::query(
        "UPDATE billing_sessions
         SET suspect = 1,
             suspect_reasons = CASE
                 WHEN suspect_reasons IS NULL THEN json_array(?1)
                 ELSE json_insert(suspect_reasons, '$[#]', ?1)
             END
         WHERE id = ?2"
    )
    .bind(reason)
    .bind(session_id)
    .execute(db)
    .await;
    if let Err(e) = result {
        tracing::warn!("[bot-coord] append_suspect_reason failed session={} reason={}: {}", session_id, reason, e);
    }
}

/// Phase 364 QUALITY-01: Handle TelemetryQualityGap (>500ms UDP silence).
/// Advisory quality signal -- logs + appends to suspect_reasons.
/// Does NOT send staff email (quality gap is not a crash alert).
pub async fn handle_telemetry_quality_gap(
    state: &Arc<AppState>,
    pod_id: &str,
    gap_ms: u32,
) {
    // Feature flag guard — snapshot + drop guard before any .await
    let flag_enabled = {
        let guard = state.feature_flags.read().await;
        guard
            .get("phase364_quality_monitor")
            .map(|r| r.enabled)
            .unwrap_or(true) // Intentional default: true. Flag missing = treat as enabled.
    }; // guard dropped here — CLAUDE.md never-hold-lock-across-await
    if !flag_enabled {
        return;
    }

    // Guard: only during active gameplay (GameState::Running)
    let game_state = {
        let pods = state.pods.read().await;
        pods.get(pod_id).and_then(|p| p.game_state)
    };
    if !matches!(game_state, Some(GameState::Running)) {
        return;
    }

    // Guard: billing must be active
    let session_id = {
        let timers = state.billing.active_timers.read().await;
        timers.get(pod_id).map(|t| t.session_id.clone())
    };
    let Some(session_id) = session_id else { return; };

    tracing::warn!(
        "[bot-coord] QUALITY-01: pod={} UDP silent {}ms -- appending suspect reason",
        pod_id, gap_ms
    );

    // Bucket gap_ms to nearest 500ms for reason string
    let bucket = (gap_ms / 500) * 500;
    let reason = format!("telemetry_gap_ms_{}", bucket);
    append_suspect_reason(&state.db, &session_id, &reason).await;
}

/// Phase 364 STALL-01: Handle SessionStalled (15s in-race telemetry silence).
/// Logs + appends to suspect_reasons. Does NOT auto-end the session.
pub async fn handle_session_stalled(
    state: &Arc<AppState>,
    pod_id: &str,
    silence_seconds: u32,
) {
    // Feature flag guard — snapshot + drop guard before any .await
    let flag_enabled = {
        let guard = state.feature_flags.read().await;
        guard
            .get("phase364_quality_monitor")
            .map(|r| r.enabled)
            .unwrap_or(true) // Intentional default: true. Flag missing = treat as enabled.
    }; // guard dropped here
    if !flag_enabled {
        return;
    }

    // Guard: GameState::Running + billing active (same as quality gap)
    let game_state = {
        let pods = state.pods.read().await;
        pods.get(pod_id).and_then(|p| p.game_state)
    };
    if !matches!(game_state, Some(GameState::Running)) {
        return;
    }
    let session_id = {
        let timers = state.billing.active_timers.read().await;
        timers.get(pod_id).map(|t| t.session_id.clone())
    };
    let Some(session_id) = session_id else { return; };

    tracing::warn!(
        "[bot-coord] STALL-01: pod={} telemetry silent {}s -- appending suspect reason",
        pod_id, silence_seconds
    );

    let reason = format!("session_stalled_{}s", silence_seconds);
    append_suspect_reason(&state.db, &session_id, &reason).await;
}

// Recovery, multiplayer teardown, drift alerting, and tests moved to bot_coordinator_recovery.rs

#[path = "bot_coordinator_recovery.rs"]
mod recovery;

// Re-export everything so callers using `crate::bot_coordinator::X` still work
pub use recovery::handle_multiplayer_failure;
pub(crate) use recovery::recover_stuck_session;
pub(crate) use recovery::alert_staff_idle_drift;

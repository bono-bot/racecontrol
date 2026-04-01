//! Billing FSM — Transition Table, Validation, and Authoritative End Session
//!
//! This module is the SINGLE SOURCE OF TRUTH for all billing status changes.
//! Every status mutation in billing.rs must go through `validate_transition()`.
//! All session-end paths converge on `authoritative_end_session()`.

use rc_common::types::BillingSessionStatus;
use std::sync::Arc;

// ─── FSM-07: Split Session Status ─────────────────────────────────────────────

/// Status of a single split entitlement within a parent billing session.
///
/// FSM-07: Each child split has its own immutable allocated_seconds and status.
/// Transitions: Pending → Active → Completed | Cancelled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitStatus {
    /// Split has been created but not yet started.
    Pending,
    /// Split is currently running (timer ticking).
    Active,
    /// Split has ended normally (time elapsed or session ended).
    Completed,
    /// Split was cancelled (parent session cancelled before this split started).
    Cancelled,
}

impl SplitStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SplitStatus::Pending => "pending",
            SplitStatus::Active => "active",
            SplitStatus::Completed => "completed",
            SplitStatus::Cancelled => "cancelled",
        }
    }
}

/// All events that can drive a billing session state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingEvent {
    /// Billing session created — Pending → WaitingForGame
    StartWaiting,
    /// Game process reached playable state — WaitingForGame → Active
    GameLive,
    /// AC Pause / ESC menu — Active → PausedGamePause
    Pause,
    /// Pod network disconnect — Active → PausedDisconnect
    Disconnect,
    /// Staff manually pauses a session — Active → PausedManual
    PauseManual,
    /// Resume from any paused state → Active
    Resume,
    /// Session timer expired naturally — any → Completed
    End,
    /// Session ended early by staff/driver — any → EndedEarly
    EndEarly,
    /// Session cancelled (full refund) — any → Cancelled
    Cancel,
    /// Session cancelled because game never became playable — WaitingForGame → CancelledNoPlayable
    CancelNoPlayable,
    /// Game crash while Active — Active → PausedCrashRecovery
    CrashPause,
}

/// Static transition table — the ONLY source of allowed billing state transitions.
/// Format: (current_status, event) → new_status
const TRANSITION_TABLE: &[(BillingSessionStatus, BillingEvent, BillingSessionStatus)] = &[
    // ─── Startup path ──────────────────────────────────────────────────────────
    (BillingSessionStatus::Pending, BillingEvent::StartWaiting, BillingSessionStatus::WaitingForGame),
    (BillingSessionStatus::WaitingForGame, BillingEvent::GameLive, BillingSessionStatus::Active),
    (BillingSessionStatus::WaitingForGame, BillingEvent::Cancel, BillingSessionStatus::CancelledNoPlayable),
    (BillingSessionStatus::WaitingForGame, BillingEvent::CancelNoPlayable, BillingSessionStatus::CancelledNoPlayable),
    // ─── Active → pause paths ─────────────────────────────────────────────────
    (BillingSessionStatus::Active, BillingEvent::Pause, BillingSessionStatus::PausedGamePause),
    (BillingSessionStatus::Active, BillingEvent::CrashPause, BillingSessionStatus::PausedCrashRecovery),
    (BillingSessionStatus::Active, BillingEvent::Disconnect, BillingSessionStatus::PausedDisconnect),
    (BillingSessionStatus::Active, BillingEvent::PauseManual, BillingSessionStatus::PausedManual),
    // ─── Active → end paths ───────────────────────────────────────────────────
    (BillingSessionStatus::Active, BillingEvent::End, BillingSessionStatus::Completed),
    (BillingSessionStatus::Active, BillingEvent::EndEarly, BillingSessionStatus::EndedEarly),
    (BillingSessionStatus::Active, BillingEvent::Cancel, BillingSessionStatus::Cancelled),
    // ─── PausedGamePause paths ────────────────────────────────────────────────
    (BillingSessionStatus::PausedGamePause, BillingEvent::Resume, BillingSessionStatus::Active),
    (BillingSessionStatus::PausedGamePause, BillingEvent::End, BillingSessionStatus::Completed),
    (BillingSessionStatus::PausedGamePause, BillingEvent::EndEarly, BillingSessionStatus::EndedEarly),
    (BillingSessionStatus::PausedGamePause, BillingEvent::Cancel, BillingSessionStatus::Cancelled),
    // ─── PausedDisconnect paths ───────────────────────────────────────────────
    (BillingSessionStatus::PausedDisconnect, BillingEvent::Resume, BillingSessionStatus::Active),
    (BillingSessionStatus::PausedDisconnect, BillingEvent::End, BillingSessionStatus::Completed),
    (BillingSessionStatus::PausedDisconnect, BillingEvent::Cancel, BillingSessionStatus::Cancelled),
    // ─── PausedManual paths ───────────────────────────────────────────────────
    (BillingSessionStatus::PausedManual, BillingEvent::Resume, BillingSessionStatus::Active),
    (BillingSessionStatus::PausedManual, BillingEvent::End, BillingSessionStatus::Completed),
    (BillingSessionStatus::PausedManual, BillingEvent::Cancel, BillingSessionStatus::Cancelled),
    // ─── PausedCrashRecovery paths ───────────────────────────────────
    (BillingSessionStatus::PausedCrashRecovery, BillingEvent::Resume, BillingSessionStatus::Active),
    (BillingSessionStatus::PausedCrashRecovery, BillingEvent::End, BillingSessionStatus::Completed),
    (BillingSessionStatus::PausedCrashRecovery, BillingEvent::EndEarly, BillingSessionStatus::EndedEarly),
    (BillingSessionStatus::PausedCrashRecovery, BillingEvent::Cancel, BillingSessionStatus::Cancelled),
];

/// Validate a billing state transition.
///
/// Returns `Ok(new_status)` if the transition is allowed, or `Err` with a description
/// if the transition is invalid (e.g. active→active, cancelled→ended).
///
/// Invalid transitions are logged at WARN level.
pub fn validate_transition(
    current: BillingSessionStatus,
    event: BillingEvent,
) -> Result<BillingSessionStatus, String> {
    for &(from, ref ev, to) in TRANSITION_TABLE {
        if from == current && *ev == event {
            return Ok(to);
        }
    }

    let msg = format!(
        "BILLING FSM: rejected transition {:?} + {:?} (current={:?})",
        event, current, current
    );
    tracing::warn!("{}", msg);
    Err(msg)
}

/// The SINGLE authoritative function for ending a billing session.
///
/// Uses CAS (Compare-And-Swap) — only finalizes sessions still in an
/// in-progress state. Returns `true` if the session was ended, `false`
/// if the CAS failed (session already finalized by a concurrent request).
///
/// This replaces the end logic in `end_billing_session()` and is called
/// for all end paths: natural expiry, early end, pause timeout, crash recovery.
pub async fn authoritative_end_session(
    db: &sqlx::SqlitePool,
    state: &Arc<crate::state::AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
    end_reason: Option<&str>,
) -> bool {
    let status_str = match end_status {
        BillingSessionStatus::Completed => "completed",
        BillingSessionStatus::EndedEarly => "ended_early",
        BillingSessionStatus::Cancelled => "cancelled",
        BillingSessionStatus::CancelledNoPlayable => "cancelled_no_playable",
        other => {
            tracing::error!(
                "BILLING FSM: authoritative_end_session called with non-terminal status {:?} for session {}",
                other, session_id
            );
            return false;
        }
    };

    // Snapshot driving_seconds from active timer (if present) before CAS
    let driving_seconds: u32 = {
        let timers = state.billing.active_timers.read().await;
        timers
            .values()
            .find(|t| t.session_id == session_id)
            .map(|t| t.driving_seconds)
            .unwrap_or(0)
    };

    // CAS: only update if session is in an in-progress state
    let cas_result = sqlx::query(
        "UPDATE billing_sessions \
         SET status = ?, driving_seconds = ?, ended_at = datetime('now'), end_reason = ? \
         WHERE id = ? \
         AND status IN ('active','paused_manual','paused_disconnect','paused_game_pause','paused_crash_recovery','waiting_for_game')",
    )
    .bind(status_str)
    .bind(driving_seconds as i64)
    .bind(end_reason.unwrap_or(""))
    .bind(session_id)
    .execute(db)
    .await;

    match cas_result {
        Err(e) => {
            tracing::error!(
                "BILLING FSM: DB error in authoritative_end_session for session {}: {}",
                session_id, e
            );
            return false;
        }
        Ok(result) if result.rows_affected() == 0 => {
            tracing::warn!(
                "BILLING FSM: CAS failed — session {} already ended (double-end prevented)",
                session_id
            );
            return false;
        }
        _ => {}
    }

    // Remove timer from active_timers
    let pod_id = {
        let mut timers = state.billing.active_timers.write().await;
        let pod_id = timers
            .iter()
            .find(|(_, t)| t.session_id == session_id)
            .map(|(k, _)| k.clone());
        if let Some(ref pid) = pod_id {
            timers.remove(pid);
        }
        pod_id
    };

    if let Some(ref pod_id) = pod_id {
        // Trigger pending rolling deploy for this pod
        crate::deploy::check_and_trigger_pending_deploy(state, pod_id).await;
    }

    true
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::types::BillingSessionStatus;

    // ─── Valid transitions ────────────────────────────────────────────────────

    #[test]
    fn test_pending_to_waiting() {
        let result = validate_transition(BillingSessionStatus::Pending, BillingEvent::StartWaiting);
        assert_eq!(result, Ok(BillingSessionStatus::WaitingForGame));
    }

    #[test]
    fn test_waiting_to_active() {
        let result = validate_transition(BillingSessionStatus::WaitingForGame, BillingEvent::GameLive);
        assert_eq!(result, Ok(BillingSessionStatus::Active));
    }

    #[test]
    fn test_active_pause() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::Pause);
        assert_eq!(result, Ok(BillingSessionStatus::PausedGamePause));
    }

    #[test]
    fn test_active_crash_pause() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::CrashPause);
        assert_eq!(result, Ok(BillingSessionStatus::PausedCrashRecovery));
    }

    #[test]
    fn test_active_disconnect() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::Disconnect);
        assert_eq!(result, Ok(BillingSessionStatus::PausedDisconnect));
    }

    #[test]
    fn test_active_pause_manual() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::PauseManual);
        assert_eq!(result, Ok(BillingSessionStatus::PausedManual));
    }

    #[test]
    fn test_active_end() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::End);
        assert_eq!(result, Ok(BillingSessionStatus::Completed));
    }

    #[test]
    fn test_active_end_early() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::EndEarly);
        assert_eq!(result, Ok(BillingSessionStatus::EndedEarly));
    }

    #[test]
    fn test_active_cancel() {
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::Cancel);
        assert_eq!(result, Ok(BillingSessionStatus::Cancelled));
    }

    #[test]
    fn test_paused_game_pause_resume() {
        let result = validate_transition(BillingSessionStatus::PausedGamePause, BillingEvent::Resume);
        assert_eq!(result, Ok(BillingSessionStatus::Active));
    }

    #[test]
    fn test_paused_disconnect_resume() {
        let result = validate_transition(BillingSessionStatus::PausedDisconnect, BillingEvent::Resume);
        assert_eq!(result, Ok(BillingSessionStatus::Active));
    }

    #[test]
    fn test_paused_manual_resume() {
        let result = validate_transition(BillingSessionStatus::PausedManual, BillingEvent::Resume);
        assert_eq!(result, Ok(BillingSessionStatus::Active));
    }

    #[test]
    fn test_waiting_cancel_no_playable() {
        let result = validate_transition(BillingSessionStatus::WaitingForGame, BillingEvent::Cancel);
        assert_eq!(result, Ok(BillingSessionStatus::CancelledNoPlayable));
    }

    #[test]
    fn test_paused_game_pause_end() {
        // pause timeout → end
        let result = validate_transition(BillingSessionStatus::PausedGamePause, BillingEvent::End);
        assert_eq!(result, Ok(BillingSessionStatus::Completed));
    }

    #[test]
    fn test_paused_game_pause_end_early() {
        let result = validate_transition(BillingSessionStatus::PausedGamePause, BillingEvent::EndEarly);
        assert_eq!(result, Ok(BillingSessionStatus::EndedEarly));
    }

    #[test]
    fn test_paused_crash_recovery_resume() {
        let result = validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::Resume);
        assert_eq!(result, Ok(BillingSessionStatus::Active));
    }

    #[test]
    fn test_paused_crash_recovery_end() {
        let result = validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::End);
        assert_eq!(result, Ok(BillingSessionStatus::Completed));
    }

    #[test]
    fn test_paused_crash_recovery_end_early() {
        let result = validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::EndEarly);
        assert_eq!(result, Ok(BillingSessionStatus::EndedEarly));
    }

    #[test]
    fn test_paused_crash_recovery_cancel() {
        let result = validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::Cancel);
        assert_eq!(result, Ok(BillingSessionStatus::Cancelled));
    }

    #[test]
    fn test_paused_disconnect_end() {
        let result = validate_transition(BillingSessionStatus::PausedDisconnect, BillingEvent::End);
        assert_eq!(result, Ok(BillingSessionStatus::Completed));
    }

    #[test]
    fn test_paused_manual_end() {
        let result = validate_transition(BillingSessionStatus::PausedManual, BillingEvent::End);
        assert_eq!(result, Ok(BillingSessionStatus::Completed));
    }

    #[test]
    fn test_waiting_cancel_explicit() {
        let result = validate_transition(
            BillingSessionStatus::WaitingForGame,
            BillingEvent::CancelNoPlayable,
        );
        assert_eq!(result, Ok(BillingSessionStatus::CancelledNoPlayable));
    }

    // ─── Invalid transitions (rejected with Err) ──────────────────────────────

    #[test]
    fn test_active_start_billing_rejected() {
        // active→active is invalid
        let result = validate_transition(BillingSessionStatus::Active, BillingEvent::StartWaiting);
        assert!(result.is_err(), "active+StartWaiting should be rejected");
    }

    #[test]
    fn test_cancelled_end_rejected() {
        // cancelled→ended is invalid
        let result = validate_transition(BillingSessionStatus::Cancelled, BillingEvent::End);
        assert!(result.is_err(), "cancelled+End should be rejected");
    }

    #[test]
    fn test_completed_end_rejected() {
        // completed→anything is a terminal state
        let result = validate_transition(BillingSessionStatus::Completed, BillingEvent::End);
        assert!(result.is_err(), "completed+End should be rejected");
    }

    #[test]
    fn test_completed_cancel_rejected() {
        let result = validate_transition(BillingSessionStatus::Completed, BillingEvent::Cancel);
        assert!(result.is_err(), "completed+Cancel should be rejected");
    }

    #[test]
    fn test_ended_early_cancel_rejected() {
        let result = validate_transition(BillingSessionStatus::EndedEarly, BillingEvent::Cancel);
        assert!(result.is_err(), "ended_early+Cancel should be rejected");
    }

    #[test]
    fn test_cancelled_no_playable_resume_rejected() {
        let result = validate_transition(BillingSessionStatus::CancelledNoPlayable, BillingEvent::Resume);
        assert!(result.is_err(), "cancelled_no_playable+Resume should be rejected");
    }

    #[test]
    fn test_pending_game_live_rejected() {
        // Cannot go Pending → Active directly (must go through WaitingForGame)
        let result = validate_transition(BillingSessionStatus::Pending, BillingEvent::GameLive);
        assert!(result.is_err(), "pending+GameLive should be rejected");
    }

    #[test]
    fn test_pending_cancel_rejected() {
        // Pending sessions should not be cancellable via Cancel event (use CancelNoPlayable path from Waiting)
        let result = validate_transition(BillingSessionStatus::Pending, BillingEvent::Cancel);
        assert!(result.is_err(), "pending+Cancel should be rejected");
    }
}

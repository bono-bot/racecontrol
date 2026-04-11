//! Game Launch Retry Orchestrator — GAME-01 through GAME-05.
//!
//! When a game launch fails, this module orchestrates:
//!   1. Immediate diagnosis via Game Doctor (GAME-01)
//!   2. Up to 2 retries with clean state reset and 5s backoff (GAME-02)
//!   3. Escalation to Tier 3/4 MMA if deterministic fixes fail (GAME-03)
//!   4. KB recording of successful fixes for future instant replay (GAME-04)
//!   5. Fleet cascade via mesh gossip for pre-immunization (GAME-05)
//!
//! The entire retry sequence is bounded to 60 seconds so the customer
//! sees recovery within one minute.

use std::time::{Duration, Instant};

use crate::game_doctor::{self, GameDiagnosis, RetryHint};
use tokio::sync::mpsc;

/// Send a LaunchStatusUpdate message on the WS channel (non-blocking, sync-safe via try_send).
///
/// Errors are logged at WARN level — a full or closed channel does not block the retry loop.
fn emit_status(
    ws_msg_tx: &mpsc::Sender<rc_common::protocol::AgentMessage>,
    launch_id: &str,
    state: rc_common::protocol::LaunchState,
    detail: Option<String>,
    ai_tier: Option<u8>,
    fix_action: Option<String>,
) {
    let msg = rc_common::protocol::AgentMessage::LaunchStatusUpdate {
        launch_id: launch_id.to_string(),
        state,
        detail,
        ai_tier,
        fix_action,
        origin: rc_common::protocol::LaunchOrigin::Retry,
    };
    if let Err(e) = ws_msg_tx.try_send(msg) {
        tracing::warn!(
            launch_id = %launch_id,
            error = %e,
            "failed to send LaunchStatusUpdate — channel full or closed"
        );
    }
}

const LOG_TARGET: &str = "game-launch-retry";
const MAX_RETRY_ATTEMPTS: u32 = 2;
const BACKOFF_SECS: u64 = 5;
const TOTAL_TIMEOUT_SECS: u64 = 60;

/// Result of the retry orchestrator.
#[derive(Debug, Clone)]
pub enum RetryResult {
    /// Deterministic fix succeeded on attempt N.
    Fixed {
        attempt: u32,
        cause: String,
        fix: String,
    },
    /// All retry attempts exhausted — escalate to MMA tiers.
    EscalateToMma {
        attempts: u32,
        causes: Vec<String>,
    },
}

/// Run the game launch retry sequence with an injectable diagnoser (for testability).
///
/// Production callers use `retry_game_launch` which passes `game_doctor::diagnose_and_fix`.
/// Tests pass a closure that returns a pre-computed `GameDiagnosis`.
///
/// `backoff_override_secs` — if Some, overrides BACKOFF_SECS (tests pass 0 to skip sleep).
pub(crate) fn retry_game_launch_with_diagnoser<F>(
    launch_id: &str,
    ws_msg_tx: &mpsc::Sender<rc_common::protocol::AgentMessage>,
    mut diagnose_fn: F,
    backoff_override_secs: Option<u64>,
) -> RetryResult
where
    F: FnMut() -> GameDiagnosis,
{
    let start = Instant::now();
    // Phase 368: emit AiAnalysisRequested at retry entry so kiosk shows "Diagnosing…"
    emit_status(ws_msg_tx, launch_id, rc_common::protocol::LaunchState::AiAnalysisRequested, None, None, None);
    let deadline = Duration::from_secs(TOTAL_TIMEOUT_SECS);
    let backoff = backoff_override_secs.unwrap_or(BACKOFF_SECS);
    let mut causes: Vec<String> = Vec::new();

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        // Check timeout
        if start.elapsed() >= deadline {
            tracing::warn!(
                target: LOG_TARGET,
                attempt = attempt,
                elapsed_ms = start.elapsed().as_millis() as u64,
                "Retry sequence timed out ({}s limit) — escalating",
                TOTAL_TIMEOUT_SECS
            );
            return RetryResult::EscalateToMma {
                attempts: attempt - 1,
                causes,
            };
        }

        tracing::info!(
            target: LOG_TARGET,
            attempt = attempt,
            max = MAX_RETRY_ATTEMPTS,
            "Game launch retry attempt {}/{}",
            attempt,
            MAX_RETRY_ATTEMPTS
        );

        let diagnosis: GameDiagnosis = diagnose_fn();
        let hint = &diagnosis.retry_hint;

        if diagnosis.fixed {
            let cause_str = format!("{:?}", diagnosis.cause);
            let fix_str = diagnosis.fix_applied.unwrap_or_else(|| diagnosis.detail.clone());
            tracing::info!(
                target: LOG_TARGET,
                attempt = attempt,
                cause = %cause_str,
                fix = %fix_str,
                elapsed_ms = start.elapsed().as_millis() as u64,
                "Game launch fix succeeded on attempt {}",
                attempt
            );
            // Phase 368: emit IssueBeingFixed + IssueFixed at Tier 1 success boundary.
            // NOTE: The server also emits IssueFixed at playable_at transition (Plan 01 Task 3).
            // LaunchStateMachine monotonic check accepts the first and silently drops the second — safe.
            emit_status(
                ws_msg_tx,
                launch_id,
                rc_common::protocol::LaunchState::IssueBeingFixed,
                Some(format!("Tier 1 fix: {}", fix_str)),
                Some(1),
                Some(fix_str.clone()),
            );
            emit_status(
                ws_msg_tx,
                launch_id,
                rc_common::protocol::LaunchState::IssueFixed,
                None,
                Some(1),
                Some(fix_str.clone()),
            );
            return RetryResult::Fixed {
                attempt,
                cause: cause_str,
                fix: fix_str,
            };
        }

        // Record the cause for this attempt
        causes.push(format!("attempt {}: {:?} — {}", attempt, diagnosis.cause, diagnosis.detail));

        // If hint says NoRetry, don't waste time on more attempts
        if *hint == RetryHint::NoRetry {
            tracing::info!(
                target: LOG_TARGET,
                cause = ?diagnosis.cause,
                "No retry possible for this cause — escalating immediately"
            );
            // Phase 368: emit NeedsManualIntervention on NoRetry exit
            emit_status(
                ws_msg_tx,
                launch_id,
                rc_common::protocol::LaunchState::NeedsManualIntervention,
                Some(format!("No retry possible for {:?}", diagnosis.cause)),
                None,
                None,
            );
            return RetryResult::EscalateToMma {
                attempts: attempt,
                causes,
            };
        }

        // Backoff before next attempt (if not last)
        if attempt < MAX_RETRY_ATTEMPTS && backoff > 0 {
            let remaining = deadline.saturating_sub(start.elapsed());
            let sleep_dur = Duration::from_secs(backoff).min(remaining);
            if sleep_dur.is_zero() {
                tracing::warn!(target: LOG_TARGET, "No time left for backoff — escalating");
                return RetryResult::EscalateToMma {
                    attempts: attempt,
                    causes,
                };
            }
            tracing::info!(
                target: LOG_TARGET,
                backoff_ms = sleep_dur.as_millis() as u64,
                "Backing off {}ms before retry attempt {}",
                sleep_dur.as_millis(),
                attempt + 1
            );
            std::thread::sleep(sleep_dur);
        }
    }

    tracing::warn!(
        target: LOG_TARGET,
        attempts = MAX_RETRY_ATTEMPTS,
        "All {} retry attempts exhausted — escalating to MMA",
        MAX_RETRY_ATTEMPTS
    );
    // Phase 368: emit NeedsManualIntervention on exhaustion exit
    emit_status(
        ws_msg_tx,
        launch_id,
        rc_common::protocol::LaunchState::NeedsManualIntervention,
        Some(format!("Tier 1 exhausted after {} attempts", MAX_RETRY_ATTEMPTS)),
        None,
        None,
    );
    RetryResult::EscalateToMma {
        attempts: MAX_RETRY_ATTEMPTS,
        causes,
    }
}

/// Run the game launch retry sequence (synchronous, called from tier engine via spawn_blocking).
///
/// Calls `game_doctor::diagnose_and_fix()` up to `MAX_RETRY_ATTEMPTS` times
/// with `BACKOFF_SECS` between attempts. Entire sequence bounded by `TOTAL_TIMEOUT_SECS`.
///
/// `launch_id` — server-minted UUID (or rcagent-local-<uuid> fallback) for status correlation.
/// `ws_msg_tx` — bounded sender to the WS writer; emit_status uses try_send (never blocks).
///
/// Returns `RetryResult::Fixed` on success or `RetryResult::EscalateToMma` on exhaustion.
pub fn retry_game_launch(
    launch_id: &str,
    ws_msg_tx: &mpsc::Sender<rc_common::protocol::AgentMessage>,
) -> RetryResult {
    retry_game_launch_with_diagnoser(launch_id, ws_msg_tx, game_doctor::diagnose_and_fix, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_doctor::{GameDiagnosis, GameFailureCause, RetryHint, hint_for_cause};
    use rc_common::protocol::{AgentMessage, LaunchState};

    // ─── Helper constructors ───────────────────────────────────────────────────

    fn fixed_diagnosis() -> GameDiagnosis {
        GameDiagnosis {
            cause: GameFailureCause::OrphanAcsProcess { pid: 1234 },
            detail: "orphan acs.exe killed".to_string(),
            fix_applied: Some("killed pid 1234".to_string()),
            fixed: true,
            retry_hint: RetryHint::RetryAfterKill,
        }
    }

    fn no_retry_diagnosis() -> GameDiagnosis {
        GameDiagnosis {
            cause: GameFailureCause::AcNotInstalled,
            detail: "AC not installed".to_string(),
            fix_applied: None,
            fixed: false,
            retry_hint: RetryHint::NoRetry,
        }
    }

    fn retryable_fail_diagnosis() -> GameDiagnosis {
        GameDiagnosis {
            cause: GameFailureCause::RaceIniCorrupt,
            detail: "race.ini corrupt".to_string(),
            fix_applied: None,
            fixed: false,
            retry_hint: RetryHint::RetryAfterConfigReset,
        }
    }

    fn drain_channel(rx: &mut tokio::sync::mpsc::Receiver<AgentMessage>) -> Vec<AgentMessage> {
        let mut msgs = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            msgs.push(msg);
        }
        msgs
    }

    fn states_from_messages(msgs: &[AgentMessage]) -> Vec<LaunchState> {
        msgs.iter()
            .filter_map(|m| {
                if let AgentMessage::LaunchStatusUpdate { state, .. } = m {
                    Some(*state)
                } else {
                    None
                }
            })
            .collect()
    }

    fn launch_ids_from_messages(msgs: &[AgentMessage]) -> Vec<String> {
        msgs.iter()
            .filter_map(|m| {
                if let AgentMessage::LaunchStatusUpdate { launch_id, .. } = m {
                    Some(launch_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    // ─── Phase 368 Plan 02 emission tests ─────────────────────────────────────

    #[test]
    fn game_launch_retry_emits_ai_analysis_on_entry() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentMessage>(16);
        let launch_id = "test-launch-entry-123";

        let result = retry_game_launch_with_diagnoser(launch_id, &tx, fixed_diagnosis, Some(0));

        let msgs = drain_channel(&mut rx);
        let states = states_from_messages(&msgs);
        let ids = launch_ids_from_messages(&msgs);

        assert!(!states.is_empty(), "expected at least one LaunchStatusUpdate message");
        assert_eq!(
            states[0],
            LaunchState::AiAnalysisRequested,
            "first emission must be AiAnalysisRequested, got {:?}",
            states[0]
        );
        for id in &ids {
            assert_eq!(id, launch_id, "launch_id mismatch: got {}", id);
        }
        assert!(
            matches!(result, RetryResult::Fixed { .. }),
            "expected RetryResult::Fixed"
        );
    }

    #[test]
    fn game_launch_retry_emits_fix_boundaries_on_success() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentMessage>(16);
        let launch_id = "test-launch-success-456";

        let _ = retry_game_launch_with_diagnoser(launch_id, &tx, fixed_diagnosis, Some(0));

        let msgs = drain_channel(&mut rx);
        let states = states_from_messages(&msgs);

        assert!(
            states.contains(&LaunchState::IssueBeingFixed),
            "expected IssueBeingFixed in {:?}", states
        );
        assert!(
            states.contains(&LaunchState::IssueFixed),
            "expected IssueFixed in {:?}", states
        );

        // IssueBeingFixed must come before IssueFixed
        let idx_being_fixed = states.iter().position(|s| *s == LaunchState::IssueBeingFixed).unwrap();
        let idx_fixed = states.iter().position(|s| *s == LaunchState::IssueFixed).unwrap();
        assert!(
            idx_being_fixed < idx_fixed,
            "IssueBeingFixed (idx {}) must precede IssueFixed (idx {})",
            idx_being_fixed, idx_fixed
        );

        // Verify ai_tier = Some(1) on IssueBeingFixed
        let being_fixed_msg = msgs.iter().find(|m| {
            matches!(m, AgentMessage::LaunchStatusUpdate { state: LaunchState::IssueBeingFixed, .. })
        });
        if let Some(AgentMessage::LaunchStatusUpdate { ai_tier, fix_action, .. }) = being_fixed_msg {
            assert_eq!(*ai_tier, Some(1), "ai_tier must be Some(1) for IssueBeingFixed");
            assert!(fix_action.is_some(), "fix_action must be Some for IssueBeingFixed");
        } else {
            panic!("IssueBeingFixed message not found");
        }
    }

    #[test]
    fn game_launch_retry_emits_needs_manual_on_no_retry() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentMessage>(16);
        let launch_id = "test-launch-no-retry-789";

        let result = retry_game_launch_with_diagnoser(launch_id, &tx, no_retry_diagnosis, Some(0));

        let msgs = drain_channel(&mut rx);
        let states = states_from_messages(&msgs);

        assert!(
            states.contains(&LaunchState::AiAnalysisRequested),
            "expected AiAnalysisRequested in {:?}", states
        );
        assert!(
            states.contains(&LaunchState::NeedsManualIntervention),
            "expected NeedsManualIntervention in {:?}", states
        );
        assert!(
            !states.contains(&LaunchState::IssueFixed),
            "should not emit IssueFixed on NoRetry path"
        );

        let needs_manual = msgs.iter().find(|m| {
            matches!(m, AgentMessage::LaunchStatusUpdate { state: LaunchState::NeedsManualIntervention, .. })
        });
        if let Some(AgentMessage::LaunchStatusUpdate { detail, .. }) = needs_manual {
            assert!(
                detail.as_deref().map(|d| d.contains("No retry possible")).unwrap_or(false),
                "NeedsManualIntervention detail should mention 'No retry possible', got {:?}", detail
            );
        } else {
            panic!("NeedsManualIntervention message not found");
        }

        assert!(
            matches!(result, RetryResult::EscalateToMma { .. }),
            "expected RetryResult::EscalateToMma"
        );
    }

    #[test]
    fn game_launch_retry_emits_needs_manual_on_exhaustion() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentMessage>(16);
        let launch_id = "test-launch-exhausted-abc";

        let result = retry_game_launch_with_diagnoser(
            launch_id,
            &tx,
            retryable_fail_diagnosis,
            Some(0), // no backoff in tests
        );

        let msgs = drain_channel(&mut rx);
        let states = states_from_messages(&msgs);

        assert!(
            states.contains(&LaunchState::AiAnalysisRequested),
            "expected AiAnalysisRequested in {:?}", states
        );
        assert!(
            states.contains(&LaunchState::NeedsManualIntervention),
            "expected NeedsManualIntervention in {:?}", states
        );

        let needs_manual = msgs.iter().find(|m| {
            matches!(m, AgentMessage::LaunchStatusUpdate { state: LaunchState::NeedsManualIntervention, .. })
        });
        if let Some(AgentMessage::LaunchStatusUpdate { detail, .. }) = needs_manual {
            assert!(
                detail.as_deref().map(|d| d.contains("exhausted")).unwrap_or(false),
                "NeedsManualIntervention detail should mention 'exhausted', got {:?}", detail
            );
        } else {
            panic!("NeedsManualIntervention message not found");
        }

        assert!(
            matches!(result, RetryResult::EscalateToMma { .. }),
            "expected RetryResult::EscalateToMma"
        );
    }

    // ─── Pre-existing game_doctor hint tests ──────────────────────────────────

    #[test]
    fn test_hint_for_orphan_acs() {
        assert_eq!(
            hint_for_cause(&GameFailureCause::OrphanAcsProcess { pid: 1234 }),
            RetryHint::RetryAfterKill
        );
    }

    #[test]
    fn test_hint_for_ac_not_installed() {
        assert_eq!(
            hint_for_cause(&GameFailureCause::AcNotInstalled),
            RetryHint::NoRetry
        );
    }

    #[test]
    fn test_hint_for_disk_space() {
        assert_eq!(
            hint_for_cause(&GameFailureCause::DiskSpaceLow { available_mb: 100 }),
            RetryHint::RetryAfterDiskCleanup
        );
    }

    #[test]
    fn test_hint_for_config_issues() {
        assert_eq!(
            hint_for_cause(&GameFailureCause::RaceIniCorrupt),
            RetryHint::RetryAfterConfigReset
        );
        assert_eq!(
            hint_for_cause(&GameFailureCause::GuiIniNotPatched),
            RetryHint::RetryAfterConfigReset
        );
    }
}

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

/// Run the game launch retry sequence (synchronous, called from tier engine via spawn_blocking).
///
/// Calls `game_doctor::diagnose_and_fix()` up to `MAX_RETRY_ATTEMPTS` times
/// with `BACKOFF_SECS` between attempts. Entire sequence bounded by `TOTAL_TIMEOUT_SECS`.
///
/// Returns `RetryResult::Fixed` on success or `RetryResult::EscalateToMma` on exhaustion.
pub fn retry_game_launch() -> RetryResult {
    let start = Instant::now();
    let deadline = Duration::from_secs(TOTAL_TIMEOUT_SECS);
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

        let diagnosis: GameDiagnosis = game_doctor::diagnose_and_fix();
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
            return RetryResult::EscalateToMma {
                attempts: attempt,
                causes,
            };
        }

        // Backoff before next attempt (if not last)
        if attempt < MAX_RETRY_ATTEMPTS {
            let remaining = deadline.saturating_sub(start.elapsed());
            let sleep_dur = Duration::from_secs(BACKOFF_SECS).min(remaining);
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
    RetryResult::EscalateToMma {
        attempts: MAX_RETRY_ATTEMPTS,
        causes,
    }
}

#[cfg(test)]
mod tests {
    use crate::game_doctor::{GameFailureCause, RetryHint, hint_for_cause};

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

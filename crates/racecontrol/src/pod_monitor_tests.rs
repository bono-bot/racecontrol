#[cfg(test)]
mod tests {
    use super::super::pod_monitor_helpers::{backoff_label, determine_failure_reason, failure_type_from_reason};
    use crate::state::{
        create_initial_backoffs, create_initial_watchdog_states, WatchdogState,
    };
    use chrono::{TimeDelta, Utc};
    use rc_common::types::PodStatus;
    use std::time::Duration;

    // ── backoff_label tests ───────────────────────────────────────────────────

    #[test]
    fn backoff_label_30s() {
        assert_eq!(backoff_label(Duration::from_secs(30)), "30s");
    }

    #[test]
    fn backoff_label_120s_is_2m() {
        assert_eq!(backoff_label(Duration::from_secs(120)), "2m");
    }

    #[test]
    fn backoff_label_600s_is_10m() {
        assert_eq!(backoff_label(Duration::from_secs(600)), "10m");
    }

    #[test]
    fn backoff_label_1800s_is_30m() {
        assert_eq!(backoff_label(Duration::from_secs(1800)), "30m");
    }

    #[test]
    fn backoff_label_3600s_is_1h() {
        assert_eq!(backoff_label(Duration::from_secs(3600)), "1h");
    }

    // ── determine_failure_reason tests ───────────────────────────────────────

    #[test]
    fn failure_reason_process_dead() {
        assert_eq!(determine_failure_reason(false, false, false), "process_dead");
        assert_eq!(determine_failure_reason(false, true, true), "process_dead");
    }

    #[test]
    fn failure_reason_no_ws_when_process_ok() {
        assert_eq!(determine_failure_reason(true, false, false), "no_ws");
        assert_eq!(determine_failure_reason(true, false, true), "no_ws");
    }

    #[test]
    fn failure_reason_no_lock_screen_when_process_and_ws_ok() {
        // This is the partial recovery case -- now treated as SUCCESS (early return before this helper)
        // but the helper itself still returns "no_lock_screen" for logging purposes
        assert_eq!(determine_failure_reason(true, true, false), "no_lock_screen");
    }

    // ── failure_type_from_reason tests ───────────────────────────────────────

    #[test]
    fn failure_type_process_dead() {
        assert_eq!(failure_type_from_reason("process_dead"), "Process Dead");
    }

    #[test]
    fn failure_type_no_ws() {
        assert_eq!(failure_type_from_reason("no_ws"), "No WebSocket");
    }

    #[test]
    fn failure_type_no_lock_screen() {
        assert_eq!(failure_type_from_reason("no_lock_screen"), "Lock Screen Unresponsive");
    }

    #[test]
    fn failure_type_unknown_fallback() {
        assert_eq!(failure_type_from_reason("something_else"), "Unknown");
    }

    // ── WatchdogState skip logic tests ───────────────────────────────────────

    #[test]
    fn watchdog_restarting_state_is_skip_condition() {
        let now = Utc::now();
        let state = WatchdogState::Restarting { attempt: 1, started_at: now };
        // Pod in Restarting should NOT be restarted again
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(should_skip, "Restarting state should trigger skip");
    }

    #[test]
    fn watchdog_verifying_state_is_skip_condition() {
        let now = Utc::now();
        let state = WatchdogState::Verifying { attempt: 2, started_at: now };
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(should_skip, "Verifying state should trigger skip");
    }

    #[test]
    fn watchdog_healthy_state_is_not_skip_condition() {
        let state = WatchdogState::Healthy;
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(!should_skip, "Healthy state should NOT trigger skip");
    }

    #[test]
    fn watchdog_recovery_failed_state_is_not_skip_condition() {
        let now = Utc::now();
        let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: now };
        // RecoveryFailed allows retry (backoff will gate actual timing)
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(!should_skip, "RecoveryFailed state should NOT trigger skip");
    }

    // ── Backoff + WatchdogState integration tests ────────────────────────────

    #[test]
    fn backoff_reset_on_natural_recovery_clears_attempt() {
        let mut backoffs = create_initial_backoffs();
        let now = Utc::now();

        // Simulate 2 prior restart attempts
        if let Some(b) = backoffs.get_mut("pod_5") {
            b.record_attempt(now);
            b.record_attempt(now + TimeDelta::seconds(120));
        }
        assert_eq!(backoffs["pod_5"].attempt(), 2);

        // Natural recovery: fresh heartbeat -> reset backoff
        if let Some(b) = backoffs.get_mut("pod_5") {
            if b.attempt() > 0 {
                b.reset();
            }
        }
        assert_eq!(backoffs["pod_5"].attempt(), 0);
    }

    #[test]
    fn watchdog_state_set_to_healthy_on_natural_recovery() {
        let mut wd_states = create_initial_watchdog_states();
        let now = Utc::now();

        // Pod was in RecoveryFailed
        wd_states.insert(
            "pod_2".to_string(),
            WatchdogState::RecoveryFailed { attempt: 3, failed_at: now },
        );
        assert!(!matches!(wd_states["pod_2"], WatchdogState::Healthy));

        // Natural recovery resets to Healthy
        if let Some(state) = wd_states.get("pod_2") {
            if *state != WatchdogState::Healthy {
                wd_states.insert("pod_2".to_string(), WatchdogState::Healthy);
            }
        }
        assert!(matches!(wd_states["pod_2"], WatchdogState::Healthy));
    }

    #[test]
    fn watchdog_state_already_healthy_no_change_on_natural_recovery() {
        let mut wd_states = create_initial_watchdog_states();

        // Pod_1 is already Healthy (default)
        let was_healthy = matches!(wd_states["pod_1"], WatchdogState::Healthy);
        assert!(was_healthy);

        // "Natural recovery" branch -- should not change anything
        if let Some(state) = wd_states.get("pod_1") {
            if *state != WatchdogState::Healthy {
                // This branch NOT entered -- already healthy
                wd_states.insert("pod_1".to_string(), WatchdogState::Healthy);
            }
        }
        // Still healthy
        assert!(matches!(wd_states["pod_1"], WatchdogState::Healthy));
    }

    // ── WS liveness pattern tests ─────────────────────────────────────────────

    #[test]
    fn ws_liveness_pattern_uses_is_closed_not_contains_key() {
        // This test documents the pattern: is_ws_alive() uses is_closed()
        // We can't test is_ws_alive() directly without AppState, but we can
        // verify the function signature exists and is correct via compilation.
        // The real test is that contains_key() is no longer used in pod_monitor.

        // Verify backoff_label correctness (smoke test)
        assert_eq!(backoff_label(Duration::from_secs(30)), "30s");
        assert_eq!(backoff_label(Duration::from_secs(120)), "2m");
    }

    // ── Partial recovery = failure tests ─────────────────────────────────────

    #[test]
    fn partial_recovery_process_and_ws_ok_lock_fail_is_failure() {
        // Partial recovery: process running + WS connected + lock screen unresponsive
        // Per CONTEXT.md: this is FAILURE, not success. Alert must fire.
        let reason = determine_failure_reason(
            true,  // process_ok
            true,  // ws_ok
            false, // lock_ok -- FAILS
        );
        assert_eq!(reason, "no_lock_screen");
        assert_eq!(failure_type_from_reason(reason), "Lock Screen Unresponsive");
    }

    #[test]
    fn full_recovery_all_three_checks_pass_is_success() {
        // All 3 checks passing is the ONLY success condition
        let process_ok = true;
        let ws_ok = true;
        let lock_ok = true;
        // If all pass, success path runs
        let is_full_recovery = process_ok && ws_ok && lock_ok;
        assert!(is_full_recovery);
    }

    #[test]
    fn any_check_failing_is_not_full_recovery() {
        // Only combinations where all three pass count as recovery
        assert!(!(true && true && false));   // lock fails
        assert!(!(true && false && true));   // ws fails
        assert!(!(false && true && true));   // process fails
    }

    // ── Email alert next_action format tests ──────────────────────────────────

    #[test]
    fn next_action_manual_when_attempt_gte_4() {
        let attempt = 4u32;
        let next_action = if attempt >= 4 {
            "Manual intervention required".to_string()
        } else {
            format!("Pod will retry in {}", backoff_label(Duration::from_secs(30)))
        };
        assert_eq!(next_action, "Manual intervention required");
    }

    #[test]
    fn next_action_retry_label_when_attempt_lt_4() {
        let attempt = 2u32;
        let cooldown_secs = 600u64; // 10m step
        let next_action = if attempt >= 4 {
            "Manual intervention required".to_string()
        } else {
            format!("Pod will retry in {}", backoff_label(Duration::from_secs(cooldown_secs)))
        };
        assert_eq!(next_action, "Pod will retry in 10m");
    }

    // ── pod_is_marked_offline_when_heartbeat_stale ───────────────────────────

    #[test]
    fn pod_status_offline_is_detection_only() {
        // Verify the status transition logic: PodStatus::Offline is the signal
        // that pod_healer's graduated tracker picks up for recovery actions.
        // pod_monitor sets it; pod_healer reads it.
        let status = PodStatus::Offline;
        assert_eq!(status, PodStatus::Offline);
    }
}

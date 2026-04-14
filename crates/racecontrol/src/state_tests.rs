//! Tests for state types and initializer functions.
//!
//! Extracted from state.rs for ARCH-03 (<500 line modules).

use super::*;

// ── WatchdogState tests ──────────────────────────────────────────────────

#[test]
fn watchdog_state_healthy_is_default_for_all_8_pods() {
    let states = create_initial_watchdog_states();
    assert_eq!(states.len(), 8);
    for i in 1u32..=8 {
        let key = format!("pod_{}", i);
        assert!(
            matches!(states.get(&key), Some(WatchdogState::Healthy)),
            "pod_{} should default to WatchdogState::Healthy",
            i
        );
    }
}

#[test]
fn watchdog_state_restarting_has_attempt_and_started_at() {
    let now = chrono::Utc::now();
    let s = WatchdogState::Restarting { attempt: 2, started_at: now };
    match s {
        WatchdogState::Restarting { attempt, started_at } => {
            assert_eq!(attempt, 2);
            assert_eq!(started_at, now);
        }
        _ => panic!("Expected Restarting variant"),
    }
}

#[test]
fn watchdog_state_verifying_has_attempt_and_started_at() {
    let now = chrono::Utc::now();
    let s = WatchdogState::Verifying { attempt: 1, started_at: now };
    match s {
        WatchdogState::Verifying { attempt, started_at } => {
            assert_eq!(attempt, 1);
            assert_eq!(started_at, now);
        }
        _ => panic!("Expected Verifying variant"),
    }
}

#[test]
fn watchdog_state_recovery_failed_has_attempt_and_failed_at() {
    let now = chrono::Utc::now();
    let s = WatchdogState::RecoveryFailed { attempt: 4, failed_at: now };
    match s {
        WatchdogState::RecoveryFailed { attempt, failed_at } => {
            assert_eq!(attempt, 4);
            assert_eq!(failed_at, now);
        }
        _ => panic!("Expected RecoveryFailed variant"),
    }
}

#[test]
fn pod_needs_restart_pre_populated_false_for_8_pods() {
    let needs = create_initial_needs_restart();
    assert_eq!(needs.len(), 8);
    for i in 1u32..=8 {
        let key = format!("pod_{}", i);
        assert_eq!(
            needs.get(&key),
            Some(&false),
            "pod_{} should default to false",
            i
        );
    }
}

// ── Deploy state tests ───────────────────────────────────────────────────

#[test]
fn create_initial_deploy_states_has_8_entries_all_idle() {
    let states = create_initial_deploy_states();
    assert_eq!(states.len(), 8);
    for i in 1u32..=8 {
        let key = format!("pod_{}", i);
        assert_eq!(
            states.get(&key),
            Some(&DeployState::Idle),
            "pod_{} should default to DeployState::Idle",
            i
        );
    }
}

// ── ChainFailureState tests (Phase 317 LAUNCH-04) ───────────────────────

#[test]
fn test_chain_failure_state_window_expired_when_no_start() {
    let state = ChainFailureState::default();
    assert!(
        state.is_window_expired(),
        "ChainFailureState with no window_start should report expired"
    );
}

#[test]
fn test_chain_failure_state_window_not_expired_recently() {
    let mut state = ChainFailureState::default();
    state.window_start = Some(std::time::Instant::now());
    assert!(
        !state.is_window_expired(),
        "ChainFailureState with freshly-set window_start should NOT be expired"
    );
}

#[test]
fn test_chain_failure_state_reset_clears_all() {
    let mut state = ChainFailureState {
        consecutive_failures: 5,
        window_start: Some(std::time::Instant::now()),
        alerted: true,
    };
    state.reset();
    assert_eq!(state.consecutive_failures, 0, "reset() should zero consecutive_failures");
    assert!(state.window_start.is_none(), "reset() should clear window_start");
    assert!(!state.alerted, "reset() should clear alerted flag");
}

#[test]
fn test_chain_failure_state_three_failures_triggers_alert() {
    let mut state = ChainFailureState::default();
    // Simulate first failure
    if state.is_window_expired() { state.reset(); }
    if state.window_start.is_none() { state.window_start = Some(std::time::Instant::now()); }
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    let should1 = state.consecutive_failures >= 3 && !state.alerted;
    assert!(!should1, "Should not escalate on 1st failure");
    // Second failure
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    let should2 = state.consecutive_failures >= 3 && !state.alerted;
    assert!(!should2, "Should not escalate on 2nd failure");
    // Third failure
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    let should3 = state.consecutive_failures >= 3 && !state.alerted;
    assert!(should3, "Should escalate on 3rd failure");
    if should3 { state.alerted = true; }
    assert!(state.alerted, "alerted should be true after 3rd failure escalation");
    assert_eq!(state.consecutive_failures, 3, "consecutive_failures should be 3");
}

#[test]
fn test_chain_failure_state_running_resets() {
    let mut state = ChainFailureState {
        consecutive_failures: 2,
        window_start: Some(std::time::Instant::now()),
        alerted: false,
    };
    state.reset();
    assert_eq!(state.consecutive_failures, 0, "Running (reset) should zero failures");
}

#[test]
fn test_chain_failure_state_no_double_alert() {
    let mut state = ChainFailureState {
        consecutive_failures: 3,
        window_start: Some(std::time::Instant::now()),
        alerted: true,
    };
    // 4th failure
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    let should4 = state.consecutive_failures >= 3 && !state.alerted;
    assert!(!should4, "4th failure should NOT trigger escalation again (alerted=true)");
}

// ── Backoff tests (existing) ─────────────────────────────────────────────

#[test]
fn create_initial_backoffs_has_exactly_8_entries() {
    let backoffs = create_initial_backoffs();
    assert_eq!(backoffs.len(), 8, "Expected exactly 8 pod backoff entries");
}

#[test]
fn create_initial_backoffs_keyed_pod_1_through_pod_8() {
    let backoffs = create_initial_backoffs();
    for i in 1u32..=8 {
        let key = format!("pod_{}", i);
        assert!(backoffs.contains_key(&key), "Missing key: {}", key);
    }
}

#[test]
fn create_initial_backoffs_each_entry_starts_at_attempt_zero() {
    let backoffs = create_initial_backoffs();
    for i in 1u32..=8 {
        let key = format!("pod_{}", i);
        let backoff = backoffs.get(&key).unwrap();
        // Use public API: attempt() method and ready() — a fresh backoff is always ready
        assert_eq!(backoff.attempt(), 0, "pod_{} should start at attempt 0", i);
        assert!(backoff.ready(chrono::Utc::now()), "pod_{} should have no prior attempt (always ready)", i);
    }
}

#[test]
fn create_initial_backoffs_pod_5_is_some() {
    let backoffs = create_initial_backoffs();
    assert!(backoffs.get("pod_5").is_some(), "pod_5 should be pre-populated");
}

#[test]
fn or_insert_with_returns_existing_entry_not_duplicate() {
    let mut backoffs = create_initial_backoffs();
    // Simulate what pod_monitor does: entry().or_insert_with()
    // Record an attempt on the pre-existing entry
    {
        let entry = backoffs.entry("pod_3".to_string()).or_insert_with(EscalatingBackoff::new);
        entry.record_attempt(chrono::Utc::now());
    }
    // Re-access — should still be at attempt 1 (existing entry was mutated, not replaced)
    let val = backoffs.get("pod_3").unwrap();
    assert_eq!(val.attempt(), 1, "or_insert_with should return pre-existing entry, not a new one");
}

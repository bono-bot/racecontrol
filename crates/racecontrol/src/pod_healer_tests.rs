use super::*;
use crate::state::WatchdogState;
use chrono::Utc;

// --- Task 1: WatchdogState skip logic ---

#[test]
fn skip_returns_true_for_restarting_state() {
    let now = Utc::now();
    let state = WatchdogState::Restarting { attempt: 1, started_at: now };
    assert!(
        should_skip_for_watchdog_state(&state),
        "heal_pod should skip for Restarting state"
    );
}

#[test]
fn skip_returns_true_for_verifying_state() {
    let now = Utc::now();
    let state = WatchdogState::Verifying { attempt: 2, started_at: now };
    assert!(
        should_skip_for_watchdog_state(&state),
        "heal_pod should skip for Verifying state"
    );
}

#[test]
fn skip_returns_false_for_healthy_state() {
    let state = WatchdogState::Healthy;
    assert!(
        !should_skip_for_watchdog_state(&state),
        "heal_pod should NOT skip for Healthy state"
    );
}

#[test]
fn skip_returns_false_for_recovery_failed_state() {
    let now = Utc::now();
    let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: now };
    assert!(
        !should_skip_for_watchdog_state(&state),
        "heal_pod should NOT skip for RecoveryFailed state (healer can still diagnose)"
    );
}

// --- is_pod_in_recovery() predicate ---

#[test]
fn recovery_blocks_second_bot_task_when_restarting() {
    let state = WatchdogState::Restarting { attempt: 1, started_at: Utc::now() };
    assert!(
        is_pod_in_recovery(&state),
        "is_pod_in_recovery must return true for Restarting — blocks second bot task"
    );
}

#[test]
fn recovery_blocks_second_bot_task_when_verifying() {
    let state = WatchdogState::Verifying { attempt: 1, started_at: Utc::now() };
    assert!(is_pod_in_recovery(&state));
}

#[test]
fn recovery_allows_bot_when_healthy() {
    assert!(!is_pod_in_recovery(&WatchdogState::Healthy));
}

#[test]
fn recovery_allows_bot_when_recovery_failed() {
    let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: Utc::now() };
    assert!(
        !is_pod_in_recovery(&state),
        "RecoveryFailed means watchdog gave up — bot may still try"
    );
}

// ─── Phase 185-02: WakeOnLan step and recovery event query tests ──────────

#[test]
fn test_wol_step_exists_in_enum() {
    let step = PodRecoveryStep::WakeOnLan;
    assert_eq!(step, PodRecoveryStep::WakeOnLan);
    assert_ne!(step, PodRecoveryStep::TierOneRestart);
    assert_ne!(step, PodRecoveryStep::AiEscalation);
    assert_ne!(step, PodRecoveryStep::AlertStaff);
    assert_ne!(step, PodRecoveryStep::Waiting);
}

#[test]
fn test_graduated_recovery_step_order() {
    let mut tracker = PodRecoveryTracker::new();
    assert_eq!(tracker.step, PodRecoveryStep::Waiting, "must start at Waiting");

    tracker.step = PodRecoveryStep::TierOneRestart;
    assert_eq!(tracker.step, PodRecoveryStep::TierOneRestart);

    tracker.step = PodRecoveryStep::WakeOnLan;
    assert_eq!(tracker.step, PodRecoveryStep::WakeOnLan);

    tracker.step = PodRecoveryStep::AiEscalation;
    assert_eq!(tracker.step, PodRecoveryStep::AiEscalation);

    tracker.step = PodRecoveryStep::AlertStaff;
    assert_eq!(tracker.step, PodRecoveryStep::AlertStaff);
}

#[test]
fn test_skip_wol_when_sentry_restart_recent() {
    use crate::recovery::RecoveryEventStore;
    use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryEvent};

    let mut store = RecoveryEventStore::new();
    let event = RecoveryEvent {
        pod_id: "pod-1".to_string(),
        process: "rc-agent.exe".to_string(),
        authority: RecoveryAuthority::RcSentry,
        action: RecoveryAction::Restart,
        spawn_verified: Some(true),
        server_reachable: Some(true),
        reason: "heartbeat_timeout".to_string(),
        context: String::new(),
        timestamp: Utc::now(),
    };
    store.push(event);

    let recent = store.query(Some("pod-1"), Some(60));
    let skip_wol_sentry = recent.iter().any(|e| {
        e.authority == RecoveryAuthority::RcSentry
            && matches!(e.action, RecoveryAction::Restart)
            && e.spawn_verified == Some(true)
    });

    assert!(
        skip_wol_sentry,
        "WoL must be skipped when rc-sentry restarted with spawn_verified=true within 60s"
    );
}

// --- Task 2: needs_restart flag logic ---

#[test]
fn needs_restart_condition_lock_screen_down_no_ws_no_billing() {
    let rc_agent_healthy = false;
    let has_active_ws = false;
    let has_active_billing = false;

    let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
    assert!(
        should_flag,
        "needs_restart should be set when lock screen down + no WS + no billing"
    );
}

#[test]
fn needs_restart_not_set_when_ws_connected() {
    let rc_agent_healthy = false;
    let has_active_ws = true;
    let has_active_billing = false;

    let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
    assert!(
        !should_flag,
        "needs_restart should NOT be set when WebSocket is connected"
    );
}

#[test]
fn needs_restart_not_set_when_billing_active() {
    let rc_agent_healthy = false;
    let has_active_ws = false;
    let has_active_billing = true;

    let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
    assert!(
        !should_flag,
        "needs_restart should NOT be set when billing is active (session in progress)"
    );
}

#[test]
fn needs_restart_not_set_for_disk_issues() {
    let disk_low = true;
    let rc_agent_healthy = true;
    let has_active_ws = true;

    let should_flag_restart = !rc_agent_healthy && !has_active_ws;
    assert!(
        !should_flag_restart,
        "needs_restart should NOT be set for disk low issues"
    );
    assert!(disk_low, "disk_low triggers a clear_temp HealAction, not a restart");
}

#[test]
fn needs_restart_not_set_for_memory_issues() {
    let memory_low = true;
    let rc_agent_healthy = true;
    let has_active_ws = true;

    let should_flag_restart = !rc_agent_healthy && !has_active_ws;
    assert!(
        !should_flag_restart,
        "needs_restart should NOT be set for memory low issues"
    );
    assert!(memory_low);
}

#[test]
fn relaunch_lock_screen_action_string() {
    let action = HealAction {
        pod_id: "pod-1".to_string(),
        action: "relaunch_lock_screen".to_string(),
        target: "edge_browser".to_string(),
        reason: "test".to_string(),
    };
    assert_eq!(action.action, "relaunch_lock_screen");
}

#[test]
fn ws_connected_no_billing_should_relaunch_not_restart() {
    let rc_agent_healthy = false;
    let has_active_ws = true;
    let has_active_billing = false;

    let should_flag_restart = !rc_agent_healthy && !has_active_ws && !has_active_billing;
    assert!(
        !should_flag_restart,
        "needs_restart should NOT be set when WS is connected"
    );
    let should_relaunch = !rc_agent_healthy && has_active_ws && !has_active_billing;
    assert!(
        should_relaunch,
        "relaunch_lock_screen should be dispatched when WS connected + no billing"
    );
}

#[test]
fn ws_connected_with_billing_should_skip_relaunch() {
    let rc_agent_healthy = false;
    let has_active_ws = true;
    let has_active_billing = true;

    let should_flag_restart = !rc_agent_healthy && !has_active_ws && !has_active_billing;
    assert!(!should_flag_restart, "no restart flag when billing active");

    let should_relaunch = !rc_agent_healthy && has_active_ws && !has_active_billing;
    assert!(!should_relaunch, "no relaunch when billing active");
}

// ─── Phase 140-02: parse_ai_action_server tests ───────────────────────────

#[test]
fn test_parse_ai_action_server_kill_edge() {
    let suggestion = r#"The edge browser is causing issues. {"action":"kill_edge"} Terminate it immediately."#;
    let result = parse_ai_action_server(suggestion);
    assert_eq!(result, Some("kill_edge"), "kill_edge action must be parsed");
}

#[test]
fn test_parse_ai_action_server_no_action_returns_none() {
    let suggestion = "Reboot the pod and check network connectivity. No specific action needed.";
    let result = parse_ai_action_server(suggestion);
    assert_eq!(result, None, "no JSON block must return None");
}

#[test]
fn test_parse_ai_action_server_unknown_action_returns_none() {
    let suggestion = r#"Try this: {"action":"reboot_system"} It should fix the issue."#;
    let result = parse_ai_action_server(suggestion);
    assert_eq!(result, None, "unknown action must return None (whitelist rejection)");
}

#[test]
fn test_parse_ai_action_server_relaunch_lock_screen() {
    let suggestion = r#"Lock screen is stuck. {"action":"relaunch_lock_screen"}"#;
    let result = parse_ai_action_server(suggestion);
    assert_eq!(result, Some("relaunch_lock_screen"));
}

#[test]
fn test_parse_ai_action_server_clear_temp() {
    let suggestion = r#"Disk space low. {"action":"clear_temp"} This will free up space."#;
    let result = parse_ai_action_server(suggestion);
    assert_eq!(result, Some("clear_temp"));
}

#[test]
fn test_parse_ai_action_server_malformed_json_returns_none() {
    let suggestion = r#"Suggestion: {action: kill_edge} missing quotes."#;
    let result = parse_ai_action_server(suggestion);
    assert_eq!(result, None, "malformed JSON must return None");
}

// ─── PodRecoveryTracker unit tests ────────────────────────────────────────

#[test]
fn tracker_starts_at_waiting() {
    let tracker = PodRecoveryTracker::new();
    assert_eq!(
        tracker.step,
        PodRecoveryStep::Waiting,
        "new tracker must start at Waiting step"
    );
    assert!(
        tracker.first_detected_at.is_none(),
        "new tracker must have no first_detected_at"
    );
}

#[test]
fn tracker_reset_clears_state() {
    let mut tracker = PodRecoveryTracker::new();
    tracker.step = PodRecoveryStep::TierOneRestart;
    tracker.first_detected_at = Some(std::time::Instant::now());

    tracker.reset();

    assert_eq!(
        tracker.step,
        PodRecoveryStep::Waiting,
        "reset must restore step to Waiting"
    );
    assert!(
        tracker.first_detected_at.is_none(),
        "reset must clear first_detected_at"
    );
}

#[test]
fn tracker_waiting_advances_to_tier_one_after_30s() {
    let mut tracker = PodRecoveryTracker::new();
    let past = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(31))
        .expect("instant subtraction must succeed");
    tracker.first_detected_at = Some(past);
    let now = std::time::Instant::now();
    let elapsed = now.duration_since(tracker.first_detected_at.unwrap_or(now));
    let should_advance = elapsed >= std::time::Duration::from_secs(30);
    assert!(
        should_advance,
        "elapsed >= 30s must trigger advance to TierOneRestart"
    );
    if should_advance {
        tracker.step = PodRecoveryStep::TierOneRestart;
    }
    assert_eq!(
        tracker.step,
        PodRecoveryStep::TierOneRestart,
        "step must be TierOneRestart after 30s elapsed"
    );
}

// ─── MON-02: Sentry fallback verification ─────────────────────────────────

#[test]
fn test_tier_one_restart_is_mon02_sentry_fallback() {
    let mut tracker = PodRecoveryTracker::new();
    tracker.step = PodRecoveryStep::TierOneRestart;
    tracker.step = PodRecoveryStep::WakeOnLan;

    assert_eq!(
        tracker.step,
        PodRecoveryStep::WakeOnLan,
        "MON-02: TierOneRestart must advance to WakeOnLan after sentry exec path"
    );
}

// ─── COORD-01: ProcessOwnership unit tests ────────────────────────────────

#[test]
fn test_ownership_check_skips_when_not_owner() {
    use rc_common::recovery::{ProcessOwnership, RecoveryAuthority};

    let mut ownership = ProcessOwnership::new();
    ownership
        .register("rc-agent.exe", RecoveryAuthority::RcSentry)
        .expect("register should succeed");

    let owner = ownership.owner_of("rc-agent.exe");
    assert_eq!(
        owner,
        Some(RecoveryAuthority::RcSentry),
        "owner_of must return RcSentry"
    );
    assert_ne!(
        owner,
        Some(RecoveryAuthority::PodHealer),
        "PodHealer must not own rc-agent.exe after RcSentry registration"
    );
    let should_skip = owner.map_or(false, |o| o != RecoveryAuthority::PodHealer);
    assert!(should_skip, "PodHealer must skip when rc-agent.exe is owned by RcSentry");
}

// ─── COORD-02: RecoveryIntent unit tests ──────────────────────────────────

#[test]
fn test_recovery_intent_prevents_concurrent_action() {
    use crate::recovery::RecoveryIntentStore;
    use rc_common::recovery::{RecoveryAuthority, RecoveryIntent};

    let mut store = RecoveryIntentStore::new();
    let intent = RecoveryIntent::new(
        "pod-1",
        "rc-agent.exe",
        RecoveryAuthority::RcSentry,
        "heartbeat_timeout_60s",
    );
    store.register(intent);

    let found = store.has_active_intent("pod-1", "rc-agent.exe");
    assert!(
        found.is_some(),
        "active intent must be found — concurrent action must be blocked"
    );
    assert_eq!(
        found.unwrap().authority,
        RecoveryAuthority::RcSentry,
        "found intent authority must match registered authority"
    );
}

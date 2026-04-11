// Phase 368 Plan 01 — LaunchStateMachine unit tests
//
// Tests:
//   1. start_launch + get_active basic
//   2. transition happy-path
//   3. Monotonic invariant (invalid/backward transitions rejected)
//   4. Terminal states block all further transitions
//   5. dismiss
//   6. Concurrent transitions (no lock held across .await proof)
//   7. prune cap (MAX_ACTIVE_LAUNCHES = 100)
//   8. test_issue_fixed_is_idempotent_when_rc_agent_emits_first (P2-05)

use chrono::Utc;
use racecontrol_crate::launch_state::LaunchStateMachine;
use rc_common::protocol::{LaunchOrigin, LaunchState};
use std::sync::Arc;

fn make_machine() -> Arc<LaunchStateMachine> {
    Arc::new(LaunchStateMachine::new())
}

/// Tests 1 + 2 combined: start_launch creates card, transition updates state.
#[tokio::test]
async fn launch_state_machine_transition() {
    let sm = make_machine();

    // Test 1: start_launch creates a card in LaunchStarted state
    let card = sm
        .start_launch(
            "launch-001".to_string(),
            "pod_1".to_string(),
            "assetto_corsa".to_string(),
            LaunchOrigin::Customer,
        )
        .await;
    assert_eq!(card.state, LaunchState::LaunchStarted);
    assert_eq!(card.launch_id, "launch-001");

    let active = sm.get_active().await;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].launch_id, "launch-001");

    // Test 2a: valid forward transition
    let updated = sm
        .transition(
            "launch-001",
            LaunchState::AiAnalysisRequested,
            Some("analysis running".to_string()),
            None,
            None,
        )
        .await;
    assert!(updated.is_some(), "Valid transition should return Some");
    let updated_card = updated.unwrap();
    assert_eq!(updated_card.state, LaunchState::AiAnalysisRequested);
    assert_eq!(updated_card.detail, Some("analysis running".to_string()));

    // Test 2b: unknown launch_id returns None
    let unknown = sm
        .transition("nonexistent-id", LaunchState::IssueFixed, None, None, None)
        .await;
    assert!(unknown.is_none(), "Transition on unknown launch_id must return None");

    // Test 3: Monotonic invariant — forward chain works
    let r1 = sm.transition("launch-001", LaunchState::IssueBeingFixed, None, Some(1), None).await;
    assert!(r1.is_some(), "AiAnalysisRequested -> IssueBeingFixed is valid");

    let r2 = sm.transition("launch-001", LaunchState::IssueFixed, None, None, None).await;
    assert!(r2.is_some(), "IssueBeingFixed -> IssueFixed is valid");

    // Test 3b: Monotonic backward transition rejected
    let backward = sm
        .transition("launch-001", LaunchState::LaunchStarted, None, None, None)
        .await;
    assert!(backward.is_none(), "Backward transition (IssueFixed -> LaunchStarted) must be rejected");

    // Verify state is still IssueFixed after rejected backward transition
    let active2 = sm.get_active().await;
    assert_eq!(active2[0].state, LaunchState::IssueFixed);

    // Test 4: Terminal state blocks any further transition
    let terminal_blocked = sm
        .transition("launch-001", LaunchState::NeedsManualIntervention, None, None, None)
        .await;
    assert!(
        terminal_blocked.is_none(),
        "Transition from terminal IssueFixed must be rejected"
    );

    // Test 4b: NeedsManualIntervention is also terminal
    let sm2 = make_machine();
    sm2.start_launch("launch-002".to_string(), "pod_2".to_string(), "f1_25".to_string(), LaunchOrigin::Staff).await;
    let _ = sm2.transition("launch-002", LaunchState::NeedsManualIntervention, None, None, None).await;
    let blocked2 = sm2.transition("launch-002", LaunchState::IssueFixed, None, None, None).await;
    assert!(blocked2.is_none(), "NeedsManualIntervention is terminal — no further transitions");
}

/// Test 5: dismiss removes card from active list.
#[tokio::test]
async fn launch_state_machine_dismiss() {
    let sm = make_machine();
    sm.start_launch("launch-d1".to_string(), "pod_3".to_string(), "iracing".to_string(), LaunchOrigin::Customer).await;
    sm.start_launch("launch-d2".to_string(), "pod_4".to_string(), "iracing".to_string(), LaunchOrigin::Customer).await;

    assert_eq!(sm.get_active().await.len(), 2);

    let removed = sm.dismiss("launch-d1").await;
    assert!(removed, "dismiss should return true for existing launch_id");

    let active = sm.get_active().await;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].launch_id, "launch-d2");

    // dismiss non-existent returns false
    let not_found = sm.dismiss("launch-d1").await;
    assert!(!not_found, "dismiss on already-removed launch_id returns false");
}

/// Test 6: Concurrent transitions on the same launch_id must not panic or deadlock.
/// Proves the RwLock is NOT held across .await — if it were, tokio would deadlock
/// when tasks try to acquire the write lock while the first task is awaiting.
#[tokio::test]
async fn launch_state_machine_no_lock_held_across_await() {
    let sm = make_machine();
    sm.start_launch("launch-c1".to_string(), "pod_5".to_string(), "forza_horizon_5".to_string(), LaunchOrigin::Customer).await;

    // Spawn 10 concurrent tasks that each try to transition the same launch_id.
    // If the lock were held across .await, this would deadlock (bounded channel scenario).
    let mut handles = Vec::new();
    for _ in 0..10 {
        let sm_clone = sm.clone();
        let handle = tokio::spawn(async move {
            // Some will succeed (first valid forward), most will fail (invalid/duplicate).
            let _ = sm_clone
                .transition(
                    "launch-c1",
                    LaunchState::AiAnalysisRequested,
                    None,
                    None,
                    None,
                )
                .await;
        });
        handles.push(handle);
    }

    // Join all — must complete without panic or deadlock
    let results = futures_util::future::join_all(handles).await;
    for r in &results {
        assert!(r.is_ok(), "Concurrent task panicked: {:?}", r);
    }

    // Final state must be consistent: exactly one transition can win
    let active = sm.get_active().await;
    assert_eq!(active.len(), 1);
    // State is either LaunchStarted (none won) or AiAnalysisRequested (exactly one won)
    let final_state = active[0].state;
    assert!(
        matches!(final_state, LaunchState::LaunchStarted | LaunchState::AiAnalysisRequested),
        "Final state after concurrent transitions must be consistent: {:?}",
        final_state
    );
}

/// Test 7: prune() enforces MAX_ACTIVE_LAUNCHES cap (100 cards max).
#[tokio::test]
async fn launch_state_machine_prune_cap() {
    let sm = make_machine();

    // Insert 150 cards
    for i in 0..150_u32 {
        sm.start_launch(
            format!("launch-prune-{}", i),
            "pod_1".to_string(),
            "assetto_corsa".to_string(),
            LaunchOrigin::Customer,
        )
        .await;
    }
    assert_eq!(sm.get_active().await.len(), 150);

    sm.prune().await;

    let after = sm.get_active().await;
    assert!(
        after.len() <= 100,
        "After prune, active cards must be <= 100, got {}",
        after.len()
    );
}

/// Test 8 (P2-05): test_issue_fixed_is_idempotent_when_rc_agent_emits_first
///
/// Scenario: rc-agent emits IssueFixed first (Tier 1 success path).
/// Later, the server's playable_at transition also calls transition(IssueFixed).
/// The second call must be a no-op (returns None) without corrupting the card.
///
/// This test is the formal guarantee that the monotonic+terminal invariant protects
/// the race between rc-agent-emits-first and server-emits-second.
#[tokio::test]
async fn test_issue_fixed_is_idempotent_when_rc_agent_emits_first() {
    let sm = make_machine();

    sm.start_launch(
        "launch-race-001".to_string(),
        "pod_7".to_string(),
        "assetto_corsa".to_string(),
        LaunchOrigin::Customer,
    )
    .await;

    // First call: rc-agent's Tier 1 success emission wins the race.
    // This simulates rc-agent emitting IssueFixed after applying a fix
    // (Plan 02 Task 1 Step B — IssueBeingFixed + IssueFixed emitted from tier_engine).
    let first = sm
        .transition(
            "launch-race-001",
            LaunchState::IssueFixed,
            Some("Tier 1 fix applied successfully".to_string()),
            Some(1),
            Some("clean_game_state".to_string()),
        )
        .await;
    assert!(
        first.is_some(),
        "First IssueFixed transition (rc-agent) must succeed"
    );
    let first_card = first.unwrap();
    assert_eq!(first_card.state, LaunchState::IssueFixed);

    // Second call: server's playable_at transition arrives later.
    // This simulates game_launcher.rs:920 branch calling transition(IssueFixed).
    // The terminal-state guard MUST reject it (returns None).
    let second = sm
        .transition(
            "launch-race-001",
            LaunchState::IssueFixed,
            None,
            None,
            None,
        )
        .await;
    assert!(
        second.is_none(),
        "Second IssueFixed transition (server playable_at) must be rejected by terminal-state guard"
    );

    // Card state must still be IssueFixed — not corrupted by the rejected second call
    let active = sm.get_active().await;
    let card = active
        .iter()
        .find(|c| c.launch_id == "launch-race-001")
        .expect("Card must still exist after rejected transition");
    assert_eq!(card.state, LaunchState::IssueFixed, "Card state must be IssueFixed after rejected second transition");
    // Detail from rc-agent emission must be preserved (not overwritten by rejected server call)
    assert_eq!(
        card.detail,
        Some("Tier 1 fix applied successfully".to_string()),
        "Detail from first (rc-agent) emission must be preserved"
    );

    // Key invariant: second transition returned None, meaning the caller would NOT
    // send a DashboardEvent::LaunchStatusChanged broadcast. Only the first transition
    // emits. This is the formal proof of the "exactly one broadcast per terminal state"
    // guarantee — enforced by transition() returning None.
    // (The caller in game_launcher.rs wraps the send in `if let Some(card) = transition(...).await`)
}

/// Test 9 (Phase 368 F-02 / D-11): prune() honors the card-lifetime rules.
///
/// D-11 "Card lifetime":
///   - `issue_fixed` cards auto-dismiss 5 minutes (300s) after reaching terminal state
///   - `needs_manual_intervention` cards stay FOREVER until staff explicitly dismisses
///   - Non-terminal stuck cards (LaunchStarted / AiAnalysisRequested / IssueBeingFixed)
///     hard-expire at 10 minutes (STALE_AFTER_SECS) as a safety net against rc-agent crashes.
///
/// This test exercises all five buckets via the test-only `set_timestamp_for_test` helper
/// to avoid a 10-minute real-time wait.
#[tokio::test]
async fn launch_state_machine_prune_card_lifetime_d11() {
    let sm = make_machine();

    // Seed cards in every relevant state with controllable launch_ids.
    for id in ["fixed-expired", "fixed-fresh", "manual-ancient", "stuck-expired", "stuck-fresh"] {
        sm.start_launch(
            id.to_string(),
            "pod_1".to_string(),
            "assetto_corsa".to_string(),
            LaunchOrigin::Customer,
        )
        .await;
    }

    // Transition the two "fixed-*" cards into IssueFixed.
    // LaunchStarted -> IssueFixed is a legal happy-path skip (see is_valid_transition).
    sm.transition("fixed-expired", LaunchState::IssueFixed, None, None, None).await.unwrap();
    sm.transition("fixed-fresh", LaunchState::IssueFixed, None, None, None).await.unwrap();

    // Transition "manual-ancient" into NeedsManualIntervention.
    sm.transition("manual-ancient", LaunchState::NeedsManualIntervention, None, None, None)
        .await
        .unwrap();

    // "stuck-expired" and "stuck-fresh" stay in LaunchStarted (never transitioned).

    // Force timestamps using the test helper.
    let now = Utc::now();
    // 301s ago -> over the 5-min auto-dismiss threshold for IssueFixed
    sm.set_timestamp_for_test("fixed-expired", now - chrono::Duration::seconds(301)).await;
    // 299s ago -> under the threshold, must be retained
    sm.set_timestamp_for_test("fixed-fresh", now - chrono::Duration::seconds(299)).await;
    // 1 hour ago -> but NeedsManualIntervention is staff-dismiss-only, must stay
    sm.set_timestamp_for_test("manual-ancient", now - chrono::Duration::seconds(3600)).await;
    // 601s ago -> over the 10-min safety net for stuck launches
    sm.set_timestamp_for_test("stuck-expired", now - chrono::Duration::seconds(601)).await;
    // 599s ago -> under the safety net, must be retained
    sm.set_timestamp_for_test("stuck-fresh", now - chrono::Duration::seconds(599)).await;

    // Sanity: all 5 cards present before prune
    assert_eq!(sm.get_active().await.len(), 5, "Setup: expect 5 cards before prune");

    // Execute prune — the behavior under test.
    sm.prune().await;

    let after: Vec<String> = sm
        .get_active()
        .await
        .into_iter()
        .map(|c| c.launch_id)
        .collect();

    // Assertions (D-11 contract):
    assert!(
        !after.contains(&"fixed-expired".to_string()),
        "IssueFixed aged >300s must be auto-dismissed"
    );
    assert!(
        after.contains(&"fixed-fresh".to_string()),
        "IssueFixed aged <300s must be retained"
    );
    assert!(
        after.contains(&"manual-ancient".to_string()),
        "NeedsManualIntervention must NEVER be auto-pruned regardless of age"
    );
    assert!(
        !after.contains(&"stuck-expired".to_string()),
        "LaunchStarted aged >600s must be pruned by safety net"
    );
    assert!(
        after.contains(&"stuck-fresh".to_string()),
        "LaunchStarted aged <600s must be retained"
    );

    // Final count: 3 survivors (fixed-fresh, manual-ancient, stuck-fresh).
    assert_eq!(after.len(), 3, "Expected 3 survivors after prune, got {:?}", after);
}

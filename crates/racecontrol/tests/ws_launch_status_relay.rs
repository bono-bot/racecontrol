// Phase 368 Plan 02 — Server-side LaunchStatusUpdate relay tests
//
// Tests:
//   1. ws_launch_status_relay_single_subscriber
//      Calling LaunchStateMachine::transition() + sending DashboardEvent::LaunchStatusChanged
//      reaches a single dashboard subscriber with the correct card.
//
//   2. ws_launch_status_relay_multiple_subscribers
//      Dashboard broadcast is received by N concurrent subscribers.
//
//   3. ws_launch_status_relay_transition_sequence
//      A sequence of LaunchStatusUpdate calls (LaunchStarted → AiAnalysisRequested →
//      IssueBeingFixed → IssueFixed) produces the correct terminal card.
//
//   4. ws_launch_status_relay_unknown_launch_id_is_dropped
//      LaunchStatusUpdate for an unregistered launch_id returns None from transition()
//      and does NOT broadcast — matches the `None =>` branch added to ws/mod.rs.
//
//   5. ws_launch_status_relay_split_deploy_id_still_relays
//      An rcagent-local-* launch_id is relayed to subscribers as long as
//      start_launch() was called first (split-deploy shim scenario).
//      (The split-deploy ERROR log is not testable here but the relay path must work.)

use racecontrol_crate::launch_state::LaunchStateMachine;
use rc_common::protocol::{DashboardEvent, LaunchOrigin, LaunchState, LaunchStatusCard};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Build an isolated (AppState-free) relay harness:
///   - Arc<LaunchStateMachine>
///   - broadcast::Sender<DashboardEvent>  +  Receiver
fn make_harness() -> (Arc<LaunchStateMachine>, broadcast::Sender<DashboardEvent>, broadcast::Receiver<DashboardEvent>) {
    let sm = Arc::new(LaunchStateMachine::new());
    let (tx, rx) = broadcast::channel(64);
    (sm, tx, rx)
}

/// Helper: simulate what ws/mod.rs does when it receives AgentMessage::LaunchStatusUpdate.
async fn relay_update(
    sm: &LaunchStateMachine,
    dashboard_tx: &broadcast::Sender<DashboardEvent>,
    launch_id: &str,
    new_state: LaunchState,
    detail: Option<String>,
    ai_tier: Option<u8>,
    fix_action: Option<String>,
) -> Option<LaunchStatusCard> {
    let maybe_card = sm
        .transition(launch_id, new_state, detail, ai_tier, fix_action)
        .await;

    if let Some(ref card) = maybe_card {
        // Mirror the ws/mod.rs arm: send to dashboard, ignore "no receivers" error.
        let _ = dashboard_tx.send(DashboardEvent::LaunchStatusChanged(card.clone()));
    }

    maybe_card
}

// ─── Test 1 ─────────────────────────────────────────────────────────────────

/// Single subscriber receives a LaunchStatusChanged event with the correct card.
#[tokio::test]
async fn ws_launch_status_relay_single_subscriber() {
    let (sm, tx, mut rx) = make_harness();

    sm.start_launch(
        "relay-t1".to_string(),
        "pod_1".to_string(),
        "assetto_corsa".to_string(),
        LaunchOrigin::Customer,
    )
    .await;

    let card = relay_update(&sm, &tx, "relay-t1", LaunchState::AiAnalysisRequested, Some("ai running".to_string()), None, None).await;
    assert!(card.is_some(), "Valid transition must return Some(card)");

    let event = rx.try_recv().expect("Subscriber must receive DashboardEvent::LaunchStatusChanged");
    match event {
        DashboardEvent::LaunchStatusChanged(c) => {
            assert_eq!(c.launch_id, "relay-t1");
            assert_eq!(c.state, LaunchState::AiAnalysisRequested);
            assert_eq!(c.detail, Some("ai running".to_string()));
        }
        other => panic!("Expected LaunchStatusChanged, got {:?}", other),
    }
}

// ─── Test 2 ─────────────────────────────────────────────────────────────────

/// Broadcast reaches ALL N concurrent subscribers.
#[tokio::test]
async fn ws_launch_status_relay_multiple_subscribers() {
    let (sm, tx, _) = make_harness();
    const N: usize = 4;

    // Subscribe N receivers BEFORE starting
    let mut receivers: Vec<broadcast::Receiver<DashboardEvent>> = (0..N).map(|_| tx.subscribe()).collect();

    sm.start_launch(
        "relay-t2".to_string(),
        "pod_2".to_string(),
        "forza_horizon_5".to_string(),
        LaunchOrigin::Staff,
    )
    .await;

    // LaunchStarted → AiAnalysisRequested is the first valid transition.
    let card = relay_update(
        &sm, &tx, "relay-t2",
        LaunchState::AiAnalysisRequested,
        Some("ai analysis running".to_string()),
        None,
        None,
    )
    .await;
    assert!(card.is_some(), "AiAnalysisRequested from LaunchStarted must succeed");

    for (i, rx) in receivers.iter_mut().enumerate() {
        let event = rx.try_recv()
            .unwrap_or_else(|e| panic!("Subscriber {} did not receive event: {:?}", i, e));
        match event {
            DashboardEvent::LaunchStatusChanged(c) => {
                assert_eq!(c.launch_id, "relay-t2", "Subscriber {} got wrong launch_id", i);
                assert_eq!(c.state, LaunchState::AiAnalysisRequested, "Subscriber {} got wrong state", i);
            }
            other => panic!("Subscriber {} got unexpected event: {:?}", i, other),
        }
    }
}

// ─── Test 3 ─────────────────────────────────────────────────────────────────

/// Multi-step sequence produces the correct terminal card.
///
/// Simulates the rc-agent emission sequence from game_launch_retry.rs:
///   LaunchStarted (start_launch) → AiAnalysisRequested → IssueBeingFixed → IssueFixed
#[tokio::test]
async fn ws_launch_status_relay_transition_sequence() {
    let (sm, tx, mut rx) = make_harness();

    sm.start_launch(
        "relay-t3".to_string(),
        "pod_3".to_string(),
        "iracing".to_string(),
        LaunchOrigin::Retry,
    )
    .await;

    // Step 1: AiAnalysisRequested (entry emission from retry_game_launch)
    let c1 = relay_update(&sm, &tx, "relay-t3", LaunchState::AiAnalysisRequested, Some("entry".to_string()), None, None).await;
    assert!(c1.is_some(), "AiAnalysisRequested transition must succeed");
    let e1 = rx.try_recv().expect("Must receive event 1");
    assert!(matches!(e1, DashboardEvent::LaunchStatusChanged(ref c) if c.state == LaunchState::AiAnalysisRequested));

    // Step 2: IssueBeingFixed (tier 1 success — before fix applied)
    let c2 = relay_update(&sm, &tx, "relay-t3", LaunchState::IssueBeingFixed, Some("applying".to_string()), Some(1), None).await;
    assert!(c2.is_some(), "IssueBeingFixed transition must succeed");
    let e2 = rx.try_recv().expect("Must receive event 2");
    assert!(matches!(e2, DashboardEvent::LaunchStatusChanged(ref c) if c.state == LaunchState::IssueBeingFixed));

    // Step 3: IssueFixed (terminal — fix applied successfully)
    let c3 = relay_update(&sm, &tx, "relay-t3", LaunchState::IssueFixed, Some("done".to_string()), None, Some("clean_game_state".to_string())).await;
    assert!(c3.is_some(), "IssueFixed transition must succeed");
    let e3 = rx.try_recv().expect("Must receive event 3");
    match e3 {
        DashboardEvent::LaunchStatusChanged(c) => {
            assert_eq!(c.state, LaunchState::IssueFixed);
            assert_eq!(c.fix_action, Some("clean_game_state".to_string()));
        }
        other => panic!("Expected LaunchStatusChanged(IssueFixed), got {:?}", other),
    }

    // Terminal guard: no further transitions allowed
    let c4 = relay_update(&sm, &tx, "relay-t3", LaunchState::NeedsManualIntervention, None, None, None).await;
    assert!(c4.is_none(), "Transition from terminal IssueFixed must return None");
    // No new event should have been sent
    assert!(rx.try_recv().is_err(), "No broadcast after rejected terminal transition");
}

// ─── Test 4 ─────────────────────────────────────────────────────────────────

/// LaunchStatusUpdate for an unregistered launch_id returns None and does NOT broadcast.
/// This matches the `None =>` warn branch in ws/mod.rs.
#[tokio::test]
async fn ws_launch_status_relay_unknown_launch_id_is_dropped() {
    let (sm, tx, mut rx) = make_harness();

    // Do NOT call start_launch — unknown launch_id
    let card = relay_update(
        &sm, &tx,
        "never-registered",
        LaunchState::AiAnalysisRequested,
        None, None, None,
    )
    .await;

    assert!(card.is_none(), "Transition on unknown launch_id must return None");
    assert!(rx.try_recv().is_err(), "No broadcast should be sent for unknown launch_id");
}

// ─── Test 5 ─────────────────────────────────────────────────────────────────

/// An rcagent-local-* launch_id is relayed when start_launch was pre-registered.
///
/// In the split-deploy scenario ws/mod.rs logs an ERROR but STILL relays the event.
/// The relay logic (transition + broadcast) is identical regardless of the id prefix.
#[tokio::test]
async fn ws_launch_status_relay_split_deploy_id_still_relays() {
    let (sm, tx, mut rx) = make_harness();

    // Simulate: rc-agent minted a local id because server was on older build.
    // In the real path, start_launch was NOT called by the server (it didn't know
    // the id). This test verifies that IF start_launch was called with the local id
    // (which could happen if the server receives the PodRegistered/LaunchGame first
    // via a different path), the relay works.
    let local_id = format!("rcagent-local-{}", uuid::Uuid::new_v4());

    sm.start_launch(
        local_id.clone(),
        "pod_5".to_string(),
        "assetto_corsa".to_string(),
        LaunchOrigin::Retry,
    )
    .await;

    let card = relay_update(
        &sm, &tx,
        &local_id,
        LaunchState::NeedsManualIntervention,
        Some("split-deploy exhaustion".to_string()),
        None,
        None,
    )
    .await;

    assert!(card.is_some(), "Relay must work even for rcagent-local-* ids");
    let event = rx.try_recv().expect("Subscriber must receive event for local id");
    match event {
        DashboardEvent::LaunchStatusChanged(c) => {
            assert_eq!(c.launch_id, local_id);
            assert_eq!(c.state, LaunchState::NeedsManualIntervention);
        }
        other => panic!("Expected LaunchStatusChanged, got {:?}", other),
    }
}

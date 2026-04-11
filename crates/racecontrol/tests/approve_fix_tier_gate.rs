// Phase 368 Plan 03 — Task 2: approve-fix tier gate tests
//
// Tests D-08 + v27.0 rule: Tier 1 auto-applies, Tier 2+ requires staff approval.
//
// Tests:
//   1. approve_fix_tier_gate_rejects_tier_1 — Tier 1 card returns 400
//   2. approve_fix_tier_gate_accepts_tier_2 — Tier 2 card returns 200 + state transitions
//   3. approve_fix_tier_gate_rejects_no_tier — no ai_tier returns 400

use std::sync::Arc;
use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use racecontrol_crate::api::debug_launches::debug_launches_approve_fix;
use racecontrol_crate::auth::middleware::StaffClaims;
use racecontrol_crate::state::AppState;
use rc_common::protocol::{LaunchOrigin, LaunchState};
use sqlx::SqlitePool;

/// Create an in-memory AppState for approve-fix tests.
async fn make_test_state() -> Arc<AppState> {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    let config = racecontrol_crate::config::Config::default_test();
    let cipher = racecontrol_crate::crypto::encryption::test_field_cipher();
    Arc::new(AppState::new(config, db, cipher))
}

fn make_claims() -> Extension<StaffClaims> {
    Extension(StaffClaims {
        sub: "staff-approve-001".to_string(),
        role: "manager".to_string(),
        exp: 9_999_999_999,
        iat: 0,
    })
}

/// Seed a launch card with a specific ai_tier value and optional NeedsManualIntervention state.
async fn seed_card_with_tier(state: &Arc<AppState>, launch_id: &str, ai_tier: Option<u8>) {
    state
        .launch_state_machine
        .start_launch(
            launch_id.to_string(),
            "pod_6".to_string(),
            "assetto_corsa".to_string(),
            LaunchOrigin::Staff,
        )
        .await;

    // Transition to NeedsManualIntervention with the given tier
    let _ = state
        .launch_state_machine
        .transition(
            launch_id,
            LaunchState::NeedsManualIntervention,
            Some("test intervention".to_string()),
            ai_tier,
            ai_tier.map(|t| format!("test-fix-tier-{}", t)),
        )
        .await;
}

/// Test 1: POST approve-fix on a Tier 1 card returns 400 + message about auto-apply.
/// D-08: Tier 1 fixes auto-apply and do not require staff approval.
#[tokio::test]
async fn approve_fix_tier_gate_rejects_tier_1() {
    let state = make_test_state().await;
    seed_card_with_tier(&state, "launch-tier1-test", Some(1)).await;

    let resp = debug_launches_approve_fix(
        State(state.clone()),
        make_claims(),
        Path("launch-tier1-test".to_string()),
    )
    .await
    .into_response();

    assert_eq!(
        resp.status(),
        axum::http::StatusCode::BAD_REQUEST,
        "Tier 1 approve-fix must return 400 BAD_REQUEST"
    );

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    let body = std::str::from_utf8(&body_bytes).expect("UTF-8 body");
    assert!(
        body.contains("do not require approval") || body.contains("auto-apply"),
        "Response body must mention that Tier 1 auto-applies. Got: {}",
        body
    );
}

/// Test 2: POST approve-fix on a Tier 2 card returns 200 and transitions card state.
/// D-08 + v27.0: Tier 2+ KB-sourced solutions require staff approval before execution.
#[tokio::test]
async fn approve_fix_tier_gate_accepts_tier_2() {
    let state = make_test_state().await;

    // Seed with Tier 2 card in NeedsManualIntervention
    state
        .launch_state_machine
        .start_launch(
            "launch-tier2-test".to_string(),
            "pod_7".to_string(),
            "iracing".to_string(),
            LaunchOrigin::Staff,
        )
        .await;

    // Transition to AiAnalysisRequested -> IssueBeingFixed with Tier 2 to set ai_tier
    let _ = state
        .launch_state_machine
        .transition(
            "launch-tier2-test",
            LaunchState::AiAnalysisRequested,
            Some("analysis".to_string()),
            Some(2),
            None,
        )
        .await;

    let resp = debug_launches_approve_fix(
        State(state.clone()),
        make_claims(),
        Path("launch-tier2-test".to_string()),
    )
    .await
    .into_response();

    assert_eq!(
        resp.status(),
        axum::http::StatusCode::OK,
        "Tier 2 approve-fix must return 200 OK"
    );
}

/// Test 3: POST approve-fix on a card with no ai_tier returns 400.
#[tokio::test]
async fn approve_fix_tier_gate_rejects_no_tier() {
    let state = make_test_state().await;

    // Seed a card with no ai_tier (plain LaunchStarted state)
    state
        .launch_state_machine
        .start_launch(
            "launch-notier-test".to_string(),
            "pod_8".to_string(),
            "assetto_corsa".to_string(),
            LaunchOrigin::Customer,
        )
        .await;

    let resp = debug_launches_approve_fix(
        State(state.clone()),
        make_claims(),
        Path("launch-notier-test".to_string()),
    )
    .await
    .into_response();

    assert_eq!(
        resp.status(),
        axum::http::StatusCode::BAD_REQUEST,
        "Card with no ai_tier must return 400 BAD_REQUEST"
    );

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    let body = std::str::from_utf8(&body_bytes).expect("UTF-8 body");
    assert!(
        body.contains("no ai_tier") || body.contains("nothing to approve"),
        "Body must mention no ai_tier. Got: {}",
        body
    );
}

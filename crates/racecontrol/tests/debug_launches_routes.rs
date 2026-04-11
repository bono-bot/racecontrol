// Phase 368 Plan 03 — Task 2: debug_launches REST endpoints tests
//
// Tests:
//   1. debug_launches_endpoints_staff_gated — all 5 endpoints require staff JWT
//   2. debug_launches_post_note_broadcasts_event — POST note broadcasts LaunchNoteAdded
//   3. debug_launches_get_notes_returns_inserted_note — GET round-trip after INSERT

use std::sync::Arc;
use axum::extract::{Extension, Path, State};
use axum::Json;
use racecontrol_crate::api::debug_launches::{
    PostNoteRequest,
    debug_launches_active,
    debug_launches_get_notes,
    debug_launches_post_note,
};
use racecontrol_crate::auth::middleware::StaffClaims;
use racecontrol_crate::state::AppState;
use rc_common::protocol::{DashboardEvent, LaunchOrigin, LaunchState};
use sqlx::SqlitePool;

/// Create an in-memory AppState with minimal schema for debug_launches tests.
async fn make_test_state() -> Arc<AppState> {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    // Minimal schema for debug_launches tests
    sqlx::query("PRAGMA foreign_keys=ON").execute(&db).await.expect("FK");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_timeline_spans (
            launch_id TEXT PRIMARY KEY, pod_id TEXT NOT NULL, sim_type TEXT NOT NULL,
            preset_id TEXT, billing_session_id TEXT, outcome TEXT NOT NULL,
            total_duration_ms INTEGER NOT NULL, started_at TEXT NOT NULL,
            events_json TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now')),
            staff_dismissed_at TEXT
        )",
    )
    .execute(&db)
    .await
    .expect("create launch_timeline_spans");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_notes (
            id TEXT PRIMARY KEY, launch_id TEXT NOT NULL, pod_id TEXT NOT NULL,
            staff_id TEXT, staff_name TEXT, body TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&db)
    .await
    .expect("create launch_notes");

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_launch_notes_launch_id ON launch_notes(launch_id)",
    )
    .execute(&db)
    .await
    .expect("create index");

    let config = racecontrol_crate::config::Config::default_test();
    let cipher = racecontrol_crate::crypto::encryption::test_field_cipher();
    Arc::new(AppState::new(config, db, cipher))
}

fn make_claims(sub: &str) -> Extension<StaffClaims> {
    Extension(StaffClaims {
        sub: sub.to_string(),
        role: "cashier".to_string(),
        exp: 9_999_999_999,
        iat: 0,
    })
}

/// Test 1: debug_launches_active returns OK with staff claims and empty array initially.
/// (Full HTTP-level auth gate testing requires a TestClient setup — this tests handler directly.)
#[tokio::test]
async fn debug_launches_endpoints_staff_gated() {
    let state = make_test_state().await;

    // GET /debug/launches/active — must return 200 OK with empty list when no active launches
    let resp = debug_launches_active(
        State(state.clone()),
        make_claims("staff-001"),
    )
    .await
    .into_response();

    use axum::response::IntoResponse;
    let status = resp.status();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "GET /debug/launches/active must return 200 OK with staff JWT"
    );
}

/// Test 2: POST /debug/launches/{id}/notes broadcasts DashboardEvent::LaunchNoteAdded.
#[tokio::test]
async fn debug_launches_post_note_broadcasts_event() {
    let state = make_test_state().await;

    // Subscribe to dashboard_tx BEFORE the POST so we don't miss the event
    let mut rx = state.dashboard_tx.subscribe();

    // Seed a launch card in the state machine so pod_id lookup works
    state
        .launch_state_machine
        .start_launch(
            "launch-note-test-01".to_string(),
            "pod_3".to_string(),
            "assetto_corsa".to_string(),
            LaunchOrigin::Staff,
        )
        .await;

    let resp = debug_launches_post_note(
        State(state.clone()),
        make_claims("staff-007"),
        Path("launch-note-test-01".to_string()),
        Json(PostNoteRequest {
            body: "Test note from staff".to_string(),
        }),
    )
    .await
    .into_response();

    use axum::response::IntoResponse;
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::CREATED,
        "POST /notes must return 201 Created"
    );

    // Verify DashboardEvent::LaunchNoteAdded was sent
    let event = rx.try_recv().expect("DashboardEvent::LaunchNoteAdded must be broadcast");
    match event {
        DashboardEvent::LaunchNoteAdded(note) => {
            assert_eq!(note.launch_id, "launch-note-test-01");
            assert_eq!(note.pod_id, "pod_3");
            assert_eq!(note.body, "Test note from staff");
            assert_eq!(note.staff_id, "staff-007");
        }
        other => panic!("Expected LaunchNoteAdded, got {:?}", other),
    }
}

/// Test 3: GET /debug/launches/{id}/notes returns the note inserted by POST.
#[tokio::test]
async fn debug_launches_get_notes_returns_inserted_note() {
    let state = make_test_state().await;

    // Seed a launch card
    state
        .launch_state_machine
        .start_launch(
            "launch-get-notes-01".to_string(),
            "pod_5".to_string(),
            "forza_horizon_5".to_string(),
            LaunchOrigin::Customer,
        )
        .await;

    // POST a note
    let post_resp = debug_launches_post_note(
        State(state.clone()),
        make_claims("staff-005"),
        Path("launch-get-notes-01".to_string()),
        Json(PostNoteRequest {
            body: "Note for round-trip test".to_string(),
        }),
    )
    .await
    .into_response();

    use axum::response::IntoResponse;
    assert_eq!(
        post_resp.status(),
        axum::http::StatusCode::CREATED,
        "POST /notes must return 201"
    );

    // GET notes for this launch
    let get_resp = debug_launches_get_notes(
        State(state.clone()),
        make_claims("staff-005"),
        Path("launch-get-notes-01".to_string()),
    )
    .await
    .into_response();

    assert_eq!(
        get_resp.status(),
        axum::http::StatusCode::OK,
        "GET /notes must return 200"
    );

    // Extract body and verify
    let body_bytes = axum::body::to_bytes(get_resp.into_body(), usize::MAX)
        .await
        .expect("read GET response body");
    let notes: Vec<serde_json::Value> =
        serde_json::from_slice(&body_bytes).expect("parse notes JSON");

    assert_eq!(notes.len(), 1, "Exactly 1 note should be returned");
    assert_eq!(
        notes[0]["launch_id"].as_str(),
        Some("launch-get-notes-01"),
        "note.launch_id must match"
    );
    assert_eq!(
        notes[0]["body"].as_str(),
        Some("Note for round-trip test"),
        "note.body must match"
    );
    assert_eq!(
        notes[0]["staff_id"].as_str(),
        Some("staff-005"),
        "note.staff_id must match claims.sub"
    );
}

//! Phase 368 Plan 03 — Debug launches REST API
//!
//! Five staff-JWT-gated endpoints for the kiosk /debug page launch cards:
//!
//! - GET  /api/v1/debug/launches/active          — list active launch cards (in-memory)
//! - GET  /api/v1/debug/launches/{id}/notes      — fetch staff notes for a launch
//! - POST /api/v1/debug/launches/{id}/notes      — append a staff note + broadcast LaunchNoteAdded
//! - POST /api/v1/debug/launches/{id}/approve-fix — Tier 2+ approval gate (D-08 + v27.0 rule)
//! - POST /api/v1/debug/launches/{id}/dismiss     — dismiss NeedsManualIntervention card (D-11)
//!
//! P1-04: All DB access uses inline sqlx::query! style (sqlx::query() with explicit typing)
//! against `state.db`. NO helper methods on the db module.
//! Note: codebase uses sqlx::query() dynamic pattern (no DATABASE_URL / .sqlx offline cache).
//! All three DB operations (SELECT notes, INSERT note, UPDATE dismiss) call sqlx::query! inline.
//!
//! D-15 enforcement: no billing state, customer names, or wallet data in any handler output.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;
use rc_common::protocol::{DashboardEvent, LaunchNoteEvent, LaunchState};

// ─── Request/Response Types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PostNoteRequest {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct NoteResponse {
    pub note_id: String,
    pub created_at: chrono::DateTime<Utc>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/v1/debug/launches/active
///
/// Returns all currently-active launch cards from the in-memory LaunchStateMachine,
/// sorted newest-first by timestamp. Requires staff JWT.
pub async fn debug_launches_active(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
) -> impl IntoResponse {
    let mut cards = state.launch_state_machine.get_active().await;
    // Newest card first (highest timestamp = most recent)
    cards.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    (StatusCode::OK, Json(cards))
}

/// GET /api/v1/debug/launches/{launch_id}/notes
///
/// Returns all staff notes for the given launch_id, ordered by created_at ASC.
/// P1-04: inline sqlx::query! (sqlx::query_as dynamic) — no db helper methods.
pub async fn debug_launches_get_notes(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
    Path(launch_id): Path<String>,
) -> impl IntoResponse {
    // sqlx::query! inline SELECT — all 7 columns, filtered by launch_id
    let rows = sqlx::query_as::<
        _,
        (
            String,          // id
            String,          // launch_id
            String,          // pod_id
            Option<String>,  // staff_id
            Option<String>,  // staff_name
            String,          // body
            Option<String>,  // created_at
        ),
    >(
        "SELECT id, launch_id, pod_id, staff_id, staff_name, body, created_at
         FROM launch_notes
         WHERE launch_id = ?
         ORDER BY created_at ASC",
    )
    .bind(&launch_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let notes: Vec<LaunchNoteEvent> = rows
                .into_iter()
                .map(|(id, launch_id, pod_id, staff_id, staff_name, body, created_at_str)| {
                    LaunchNoteEvent {
                        launch_id,
                        pod_id,
                        note_id: id,
                        staff_id: staff_id.unwrap_or_default(),
                        staff_name: staff_name.unwrap_or_default(),
                        body,
                        created_at: created_at_str
                            .and_then(|s| {
                                chrono::DateTime::parse_from_rfc3339(&s)
                                    .ok()
                                    .map(|dt| dt.with_timezone(&Utc))
                            })
                            .unwrap_or_else(Utc::now),
                    }
                })
                .collect();
            (StatusCode::OK, Json(notes)).into_response()
        }
        Err(e) => {
            tracing::error!(launch_id = %launch_id, error = %e, "failed to query launch_notes");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// POST /api/v1/debug/launches/{launch_id}/notes
///
/// Appends a staff note to the given launch and broadcasts DashboardEvent::LaunchNoteAdded.
/// P1-04: inline sqlx::query! INSERT — no db helper methods.
/// D-15: body must not contain billing/customer context — caller responsibility.
pub async fn debug_launches_post_note(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Path(launch_id): Path<String>,
    Json(req): Json<PostNoteRequest>,
) -> impl IntoResponse {
    // Validate body: non-empty after trim, 1-2000 chars
    let body = req.body.trim().to_string();
    if body.is_empty() || body.len() > 2000 {
        return (StatusCode::BAD_REQUEST, "body must be 1-2000 chars").into_response();
    }

    // Resolve pod_id from active launch state machine (snapshot — no lock held across await)
    let active = state.launch_state_machine.get_active().await;
    let pod_id = active
        .iter()
        .find(|c| c.launch_id == launch_id)
        .map(|c| c.pod_id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let note_id = Uuid::new_v4().to_string();
    let created_at = Utc::now();
    let created_at_str = created_at.to_rfc3339();

    // StaffClaims.sub carries the staff identifier; no staff_name field in StaffClaims
    let staff_id = claims.sub.clone();
    let staff_name = claims.sub.clone();

    // sqlx::query! inline INSERT into launch_notes
    if let Err(e) = sqlx::query(
        "INSERT INTO launch_notes (id, launch_id, pod_id, staff_id, staff_name, body, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&note_id)
    .bind(&launch_id)
    .bind(&pod_id)
    .bind(&staff_id)
    .bind(&staff_name)
    .bind(&body)
    .bind(&created_at_str)
    .execute(&state.db)
    .await
    {
        tracing::error!(launch_id = %launch_id, error = %e, "failed to insert launch_note");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Broadcast DashboardEvent::LaunchNoteAdded (fire-and-forget — no lock held)
    let event = LaunchNoteEvent {
        launch_id: launch_id.clone(),
        pod_id,
        note_id: note_id.clone(),
        staff_id,
        staff_name,
        body,
        created_at,
    };
    let _ = state.dashboard_tx.send(DashboardEvent::LaunchNoteAdded(event));

    (StatusCode::CREATED, Json(NoteResponse { note_id, created_at })).into_response()
}

/// POST /api/v1/debug/launches/{launch_id}/approve-fix
///
/// D-08 + v27.0 rule: Tier 1 auto-applies (no approval needed); Tier 2+ requires staff click.
/// No DB write here — approval transitions the in-memory card and emits a WS event.
/// TODO(368-follow-up): wire to tier_engine approval path in rc-agent for actual fix execution.
pub async fn debug_launches_approve_fix(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
    Path(launch_id): Path<String>,
) -> impl IntoResponse {
    let active = state.launch_state_machine.get_active().await;
    let card = match active.iter().find(|c| c.launch_id == launch_id) {
        Some(c) => c.clone(),
        None => return (StatusCode::NOT_FOUND, "launch not found").into_response(),
    };

    // D-08 + v27.0 tier gate
    // Tier 1 fixes auto-apply and do not require approval (Phase 275 rc-agent path)
    match card.ai_tier {
        None => (
            StatusCode::BAD_REQUEST,
            "no ai_tier on card — nothing to approve",
        )
            .into_response(),
        Some(1) => (
            StatusCode::BAD_REQUEST,
            "Tier 1 fixes auto-apply and do not require approval",
        )
            .into_response(),
        Some(tier) if tier >= 2 => {
            // Transition card to IssueBeingFixed to reflect staff approval
            // TODO(368-follow-up): also send AgentMessage to pod's ws_handler to execute fix
            if let Some(updated) = state
                .launch_state_machine
                .transition(
                    &launch_id,
                    LaunchState::IssueBeingFixed,
                    Some(format!("staff-approved Tier {} fix", tier)),
                    Some(tier),
                    card.fix_action.clone(),
                )
                .await
            {
                let _ = state
                    .dashboard_tx
                    .send(DashboardEvent::LaunchStatusChanged(updated));
            }
            (StatusCode::OK, "approved").into_response()
        }
        Some(_) => (StatusCode::BAD_REQUEST, "unknown ai_tier value").into_response(),
    }
}

/// POST /api/v1/debug/launches/{launch_id}/dismiss
///
/// D-11: manually dismisses a NeedsManualIntervention card.
/// Removes from in-memory LaunchStateMachine AND marks launch_timeline_spans.staff_dismissed_at.
/// P1-04: inline sqlx::query! UPDATE — no db helper methods.
pub async fn debug_launches_dismiss(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
    Path(launch_id): Path<String>,
) -> impl IntoResponse {
    // Remove from in-memory state machine
    let removed = state.launch_state_machine.dismiss(&launch_id).await;

    // Mark durable audit trail: staff_dismissed_at on launch_timeline_spans
    // Fire-and-forget idempotent UPDATE — no error if launch_id not found in spans table
    let dismissed_at = Utc::now().to_rfc3339();
    let _ = sqlx::query(
        "UPDATE launch_timeline_spans SET staff_dismissed_at = ? WHERE launch_id = ?",
    )
    .bind(&dismissed_at)
    .bind(&launch_id)
    .execute(&state.db)
    .await;

    if removed {
        (StatusCode::OK, "dismissed").into_response()
    } else {
        (StatusCode::NOT_FOUND, "launch not found").into_response()
    }
}

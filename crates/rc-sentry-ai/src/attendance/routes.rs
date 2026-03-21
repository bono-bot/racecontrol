use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::db;

/// Shared state for attendance API handlers.
pub struct AttendanceState {
    pub db_path: String,
    pub present_timeout_secs: u64,
    pub min_shift_hours: u64,
}

type SharedAttendance = Arc<AttendanceState>;

/// Build the attendance API router.
pub fn attendance_router(state: SharedAttendance) -> axum::Router {
    axum::Router::new()
        .route("/api/v1/attendance/present", get(present_handler))
        .route("/api/v1/attendance/history", get(history_handler))
        .route("/api/v1/attendance/shifts", get(shifts_day_handler))
        .route(
            "/api/v1/attendance/shifts/{person_id}",
            get(shifts_person_handler),
        )
        .with_state(state)
}

/// Return today in IST (UTC+5:30) as "YYYY-MM-DD".
fn today_ist() -> String {
    use chrono::Utc;
    let ist = chrono::FixedOffset::east_opt(19800).expect("valid IST offset");
    let now_ist = Utc::now().with_timezone(&ist);
    now_ist.format("%Y-%m-%d").to_string()
}

// --- Present handler ---

async fn present_handler(State(state): State<SharedAttendance>) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let timeout_secs = state.present_timeout_secs;

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;

        let ist = chrono::FixedOffset::east_opt(19800).expect("valid IST offset");
        let now_ist = chrono::Utc::now().with_timezone(&ist);
        let day = now_ist.format("%Y-%m-%d").to_string();
        let since = (now_ist - chrono::Duration::seconds(timeout_secs as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let present = db::get_present_persons(&conn, &day, &since)
            .map_err(|e| format!("get_present_persons: {e}"))?;
        let count = present.len();
        Ok(json!({
            "present": present,
            "count": count,
            "timeout_secs": timeout_secs,
        }))
    })
    .await;

    match result {
        Ok(Ok(val)) => Json(val).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// --- History handler ---

#[derive(Deserialize)]
struct HistoryParams {
    day: Option<String>,
}

async fn history_handler(
    State(state): State<SharedAttendance>,
    Query(params): Query<HistoryParams>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let day = params.day.unwrap_or_else(today_ist);

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let entries = db::get_attendance_for_day(&conn, &day)
            .map_err(|e| format!("get_attendance_for_day: {e}"))?;
        let count = entries.len();
        Ok(json!({
            "day": day,
            "entries": entries,
            "count": count,
        }))
    })
    .await;

    match result {
        Ok(Ok(val)) => Json(val).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// --- Shifts by day handler ---

#[derive(Deserialize)]
struct ShiftsDayParams {
    day: Option<String>,
}

#[derive(Serialize)]
struct ShiftResponse {
    id: i64,
    person_id: i64,
    person_name: String,
    day: String,
    clock_in: String,
    clock_out: Option<String>,
    shift_minutes: Option<i64>,
    complete: bool,
}

async fn shifts_day_handler(
    State(state): State<SharedAttendance>,
    Query(params): Query<ShiftsDayParams>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let day = params.day.unwrap_or_else(today_ist);
    let min_shift_hours = state.min_shift_hours;

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let shifts = db::get_shifts_for_day(&conn, &day)
            .map_err(|e| format!("get_shifts_for_day: {e}"))?;

        let shift_responses: Vec<ShiftResponse> = shifts
            .into_iter()
            .map(|s| {
                let complete = s.shift_minutes.unwrap_or(0) >= (min_shift_hours as i64) * 60;
                ShiftResponse {
                    id: s.id,
                    person_id: s.person_id,
                    person_name: s.person_name,
                    day: s.day,
                    clock_in: s.clock_in,
                    clock_out: s.clock_out,
                    shift_minutes: s.shift_minutes,
                    complete,
                }
            })
            .collect();

        let count = shift_responses.len();
        Ok(json!({
            "day": day,
            "shifts": shift_responses,
            "count": count,
        }))
    })
    .await;

    match result {
        Ok(Ok(val)) => Json(val).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// --- Shifts by person handler ---

#[derive(Deserialize)]
struct LimitParams {
    limit: Option<u32>,
}

async fn shifts_person_handler(
    State(state): State<SharedAttendance>,
    Path(person_id): Path<i64>,
    Query(params): Query<LimitParams>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let limit = params.limit.unwrap_or(30);

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let shifts = db::get_shifts_for_person(&conn, person_id, limit)
            .map_err(|e| format!("get_shifts_for_person: {e}"))?;
        let count = shifts.len();
        Ok(json!({
            "person_id": person_id,
            "shifts": shifts,
            "count": count,
        }))
    })
    .await;

    match result {
        Ok(Ok(val)) => Json(val).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

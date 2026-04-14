#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Phase 367: Session replay + batch export handlers ──────────────────────

/// GET /api/v1/admin/sessions/{id}/replay
///
/// Returns all telemetry events for a completed billing session as an ordered JSON array.
/// Events are: lap_start, telemetry (per sample), lap_end.
/// Capped at 10,000 total events to prevent OOM on very long sessions.
/// Requires manager role. Phase 367-03 (GLD-G-03).
pub(crate) async fn session_replay_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let laps_result = sqlx::query_as::<_, (String, i64, i64, i64)>(
        "SELECT id, lap_number, lap_time_ms, valid
         FROM laps
         WHERE billing_session_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await;

    let laps = match laps_result {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("DB error: {}", e) })),
            ).into_response();
        }
    };

    if laps.is_empty() {
        return Json(json!({
            "session_id": session_id,
            "events": [],
            "truncated": false,
            "total_events": 0
        })).into_response();
    }

    let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);

    const EVENT_CAP: usize = 10_000;
    let mut events: Vec<Value> = Vec::with_capacity(512);
    let mut truncated = false;

    'outer: for (lap_id, lap_number, lap_time_ms, valid) in &laps {
        events.push(json!({
            "type": "lap_start",
            "lap": lap_number,
            "valid": valid == &1
        }));

        let samples = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)>(
            "SELECT offset_ms, speed, throttle, brake, steering, gear, rpm
             FROM telemetry_samples
             WHERE lap_id = ?
             ORDER BY offset_ms ASC",
        )
        .bind(lap_id)
        .fetch_all(telem_pool)
        .await
        .unwrap_or_default();

        for (offset_ms, speed, throttle, brake, steering, gear, rpm) in samples {
            if events.len() >= EVENT_CAP {
                truncated = true;
                break 'outer;
            }
            events.push(json!({
                "type": "telemetry",
                "lap": lap_number,
                "offset_ms": offset_ms,
                "speed": speed,
                "throttle": throttle,
                "brake": brake,
                "steering": steering,
                "gear": gear,
                "rpm": rpm
            }));
        }

        events.push(json!({
            "type": "lap_end",
            "lap": lap_number,
            "lap_time_ms": lap_time_ms,
            "valid": valid == &1
        }));
    }

    let total = events.len();
    Json(json!({
        "session_id": session_id,
        "lap_count": laps.len(),
        "events": events,
        "truncated": truncated,
        "total_events": total
    })).into_response()
}

/// GET /api/v1/admin/export/estimate?from=YYYY-MM-DD&to=YYYY-MM-DD&include=billing,laps,telemetry
///
/// Returns estimated row counts for a batch export query. Cheap COUNT(*) only.
/// Requires manager role. Phase 367-04 (GLD-G-04).
pub(crate) async fn admin_export_estimate_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let from = params.get("from").cloned().unwrap_or_else(|| "1970-01-01".to_string());
    let to = params.get("to").cloned().unwrap_or_else(|| "2099-12-31".to_string());
    let include = params.get("include").cloned().unwrap_or_else(|| "billing,laps".to_string());
    let includes: Vec<&str> = include.split(',').collect();

    let mut billing_rows: i64 = 0;
    let mut lap_rows: i64 = 0;
    let mut telemetry_rows: i64 = 0;

    if includes.contains(&"billing") {
        billing_rows = sqlx::query_scalar(
            "SELECT COUNT(*) FROM billing_sessions WHERE DATE(ended_at) >= ? AND DATE(ended_at) <= ?",
        )
        .bind(&from)
        .bind(&to)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    }

    if includes.contains(&"laps") {
        lap_rows = sqlx::query_scalar(
            "SELECT COUNT(*) FROM laps l
             JOIN billing_sessions bs ON bs.id = l.billing_session_id
             WHERE DATE(bs.ended_at) >= ? AND DATE(bs.ended_at) <= ?",
        )
        .bind(&from)
        .bind(&to)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    }

    if includes.contains(&"telemetry") {
        let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);
        let lap_ids: Vec<String> = sqlx::query_scalar(
            "SELECT l.id FROM laps l
             JOIN billing_sessions bs ON bs.id = l.billing_session_id
             WHERE DATE(bs.ended_at) >= ? AND DATE(bs.ended_at) <= ?
             LIMIT 10000",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        if !lap_ids.is_empty() {
            let sample_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM telemetry_samples WHERE lap_id = ?",
            )
            .bind(lap_ids.first().expect("lap_ids is non-empty"))
            .fetch_one(telem_pool)
            .await
            .unwrap_or(0);
            telemetry_rows = sample_count * lap_ids.len() as i64;
        }
    }

    Json(json!({
        "billing_rows": billing_rows,
        "lap_rows": lap_rows,
        "telemetry_rows": telemetry_rows,
        "total_rows": billing_rows + lap_rows + telemetry_rows
    }))
}

/// GET /api/v1/admin/export?from=YYYY-MM-DD&to=YYYY-MM-DD&format=csv|jsonl&include=billing,laps,telemetry
///
/// Exports session data as CSV or JSONL for a date range.
/// Max range: 30 days. Returns 400 if exceeded.
/// Requires manager role. Phase 367-04 (GLD-G-04).
pub(crate) async fn admin_export_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let from = params.get("from").cloned().unwrap_or_else(|| "1970-01-01".to_string());
    let to = params.get("to").cloned().unwrap_or_else(|| "2099-12-31".to_string());
    let format = params.get("format").cloned().unwrap_or_else(|| "csv".to_string());
    let include = params.get("include").cloned().unwrap_or_else(|| "billing,laps".to_string());
    let includes: Vec<&str> = include.split(',').collect();

    // Validate 30-day range
    if let (Ok(from_date), Ok(to_date)) = (
        chrono::NaiveDate::parse_from_str(&from, "%Y-%m-%d"),
        chrono::NaiveDate::parse_from_str(&to, "%Y-%m-%d"),
    ) {
        let days = (to_date - from_date).num_days();
        if days > 30 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Date range exceeds 30 days maximum" })),
            ).into_response();
        }
    }

    let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);
    let filename = format!("rp-export-{}-{}.{}", from, to, format);
    let is_csv = format == "csv";

    let mut output = String::new();

    // --- BILLING ---
    if includes.contains(&"billing") {
        if is_csv {
            output.push_str("session_id,driver_id,driver_name,pod_id,started_at,ended_at,allocated_seconds,driving_seconds,wallet_debit_paise,status,suspect,telemetry_coverage_pct,lap_count_actual,lap_count_expected\n");
        }
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<i64>, Option<f64>, Option<i64>, Option<i64>)>(
            "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, bs.started_at, bs.ended_at,
                    bs.allocated_seconds, bs.driving_seconds, bs.wallet_debit_paise,
                    bs.status, bs.suspect, bs.telemetry_coverage_pct,
                    bs.lap_count_actual, bs.lap_count_expected
             FROM billing_sessions bs
             LEFT JOIN drivers d ON d.id = bs.driver_id
             WHERE DATE(bs.ended_at) >= ? AND DATE(bs.ended_at) <= ?
             ORDER BY bs.ended_at ASC",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        for (id, driver_id, driver_name, pod_id, started_at, ended_at, alloc_s, drive_s, debit, status, suspect, coverage, lap_actual, lap_expected) in rows {
            if is_csv {
                output.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    id,
                    driver_id.unwrap_or_default(),
                    driver_name.unwrap_or_default().replace(',', ";"),
                    pod_id.unwrap_or_default(),
                    started_at.unwrap_or_default(),
                    ended_at.unwrap_or_default(),
                    alloc_s.unwrap_or(0),
                    drive_s.unwrap_or(0),
                    debit.unwrap_or(0),
                    status.unwrap_or_default(),
                    suspect.unwrap_or(0),
                    coverage.map(|v| format!("{:.1}", v)).unwrap_or_default(),
                    lap_actual.unwrap_or(0),
                    lap_expected.unwrap_or(0),
                ));
            } else {
                output.push_str(&json!({
                    "type": "billing",
                    "session_id": id,
                    "driver_id": driver_id,
                    "driver_name": driver_name,
                    "pod_id": pod_id,
                    "started_at": started_at,
                    "ended_at": ended_at,
                    "allocated_seconds": alloc_s,
                    "driving_seconds": drive_s,
                    "wallet_debit_paise": debit,
                    "status": status,
                    "suspect": suspect,
                    "telemetry_coverage_pct": coverage,
                    "lap_count_actual": lap_actual,
                    "lap_count_expected": lap_expected
                }).to_string());
                output.push('\n');
            }
        }
    }

    // --- LAPS ---
    if includes.contains(&"laps") {
        if is_csv {
            output.push_str("lap_id,billing_session_id,driver_id,lap_number,lap_time_ms,sector1_ms,sector2_ms,sector3_ms,valid,suspect\n");
        }
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, i64, i64, Option<i64>, Option<i64>, Option<i64>, i64, Option<i64>)>(
            "SELECT l.id, l.billing_session_id, l.driver_id, l.lap_number, l.lap_time_ms,
                    l.sector1_ms, l.sector2_ms, l.sector3_ms, l.valid, l.suspect
             FROM laps l
             JOIN billing_sessions bs ON bs.id = l.billing_session_id
             WHERE DATE(bs.ended_at) >= ? AND DATE(bs.ended_at) <= ?
             ORDER BY bs.ended_at ASC, l.lap_number ASC",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        for (id, session_id, driver_id, lap_num, lap_ms, s1, s2, s3, valid, suspect) in rows {
            if is_csv {
                output.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{}\n",
                    id,
                    session_id.unwrap_or_default(),
                    driver_id.unwrap_or_default(),
                    lap_num, lap_ms,
                    s1.map(|v| v.to_string()).unwrap_or_default(),
                    s2.map(|v| v.to_string()).unwrap_or_default(),
                    s3.map(|v| v.to_string()).unwrap_or_default(),
                    valid, suspect.unwrap_or(0)
                ));
            } else {
                output.push_str(&json!({
                    "type": "lap",
                    "lap_id": id,
                    "billing_session_id": session_id,
                    "driver_id": driver_id,
                    "lap_number": lap_num,
                    "lap_time_ms": lap_ms,
                    "sector1_ms": s1, "sector2_ms": s2, "sector3_ms": s3,
                    "valid": valid == 1,
                    "suspect": suspect.unwrap_or(0) == 1
                }).to_string());
                output.push('\n');
            }
        }
    }

    // --- TELEMETRY (if requested) ---
    if includes.contains(&"telemetry") {
        if is_csv {
            output.push_str("lap_id,offset_ms,speed,throttle,brake,steering,gear,rpm\n");
        }
        let lap_ids: Vec<String> = sqlx::query_scalar(
            "SELECT l.id FROM laps l
             JOIN billing_sessions bs ON bs.id = l.billing_session_id
             WHERE DATE(bs.ended_at) >= ? AND DATE(bs.ended_at) <= ?
             ORDER BY bs.ended_at ASC, l.lap_number ASC",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        for lap_id in &lap_ids {
            let samples = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)>(
                "SELECT offset_ms, speed, throttle, brake, steering, gear, rpm
                 FROM telemetry_samples WHERE lap_id = ? ORDER BY offset_ms ASC",
            )
            .bind(lap_id)
            .fetch_all(telem_pool)
            .await
            .unwrap_or_default();

            for (offset_ms, speed, throttle, brake, steering, gear, rpm) in samples {
                if is_csv {
                    output.push_str(&format!(
                        "{},{},{},{},{},{},{},{}\n",
                        lap_id,
                        offset_ms,
                        speed.map(|v| format!("{:.2}", v)).unwrap_or_default(),
                        throttle.map(|v| format!("{:.3}", v)).unwrap_or_default(),
                        brake.map(|v| format!("{:.3}", v)).unwrap_or_default(),
                        steering.map(|v| format!("{:.3}", v)).unwrap_or_default(),
                        gear.unwrap_or(0),
                        rpm.unwrap_or(0),
                    ));
                } else {
                    output.push_str(&json!({
                        "type": "telemetry",
                        "lap_id": lap_id,
                        "offset_ms": offset_ms,
                        "speed": speed, "throttle": throttle, "brake": brake,
                        "steering": steering, "gear": gear, "rpm": rpm
                    }).to_string());
                    output.push('\n');
                }
            }
        }
    }

    let content_type = if is_csv { "text/csv" } else { "application/x-ndjson" };
    let disposition = format!("attachment; filename=\"{}\"", filename);

    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type.to_string()),
            (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
        ],
        output,
    ).into_response()
}

#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Public Events Endpoints ─────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct EventsListQuery {
    status: Option<String>,
    sim_type: Option<String>,
}

/// GET /public/events — list all non-cancelled events, sorted by status priority then date
pub(crate) async fn public_events_list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventsListQuery>,
) -> Json<Value> {
    let mut conditions = vec!["status != 'cancelled'".to_string()];
    if let Some(ref s) = params.status {
        conditions.push(format!("status = '{}'", s.replace('\'', "''")));
    }
    if let Some(ref st) = params.sim_type {
        conditions.push(format!("sim_type = '{}'", st.replace('\'', "''")));
    }
    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms, created_at,
                (SELECT COUNT(*) FROM hotlap_event_entries WHERE event_id = hotlap_events.id) as entry_count
         FROM hotlap_events
         WHERE {}
         ORDER BY
           CASE status
             WHEN 'active' THEN 1
             WHEN 'upcoming' THEN 2
             WHEN 'scoring' THEN 3
             WHEN 'completed' THEN 4
             ELSE 5
           END,
           starts_at DESC",
        where_clause
    );

    let rows: Vec<(String, String, Option<String>, String, String, String, String, String,
                   Option<String>, Option<String>, Option<i64>, Option<String>, i64)> =
        match sqlx::query_as(&sql).fetch_all(&state.db).await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let events: Vec<Value> = rows.into_iter().map(|(id, name, description, track, car, car_class, sim_type, status,
                                                     starts_at, ends_at, reference_time_ms, created_at, entry_count)| {
        json!({
            "id": id,
            "name": name,
            "description": description,
            "track": track,
            "car": car,
            "car_class": car_class,
            "sim_type": sim_type,
            "status": status,
            "starts_at": starts_at,
            "ends_at": ends_at,
            "reference_time_ms": reference_time_ms,
            "created_at": created_at,
            "entry_count": entry_count,
        })
    }).collect();

    Json(json!({ "events": events }))
}

/// GET /public/events/{id} — event leaderboard with per-class grouping, badges, 107% flags
pub(crate) async fn public_event_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Fetch event metadata
    let event_row: Option<(String, String, Option<String>, String, String, String, String, String,
                           Option<String>, Option<String>, Option<i64>)> = match sqlx::query_as(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms
         FROM hotlap_events WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let event_row = match event_row {
        Some(r) => r,
        None => return Json(json!({ "error": "Event not found" })),
    };

    let event = json!({
        "id": event_row.0,
        "name": event_row.1,
        "description": event_row.2,
        "track": event_row.3,
        "car": event_row.4,
        "car_class": event_row.5,
        "sim_type": event_row.6,
        "status": event_row.7,
        "starts_at": event_row.8,
        "ends_at": event_row.9,
        "reference_time_ms": event_row.10,
    });

    // Fetch leaderboard entries — PII excluded, nickname/name display logic applied
    let entries_rows: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, Option<i64>,
                           Option<i64>, Option<i64>, Option<String>, Option<i64>, Option<i64>,
                           String, Option<String>, Option<String>, Option<String>)> =
        match sqlx::query_as(
            "SELECT hee.driver_id,
                    CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                         THEN d.nickname ELSE d.name END as display_name,
                    hee.lap_time_ms, hee.sector1_ms, hee.sector2_ms, hee.sector3_ms,
                    hee.position, hee.points, hee.badge, hee.gap_to_leader_ms,
                    hee.within_107_percent, hee.result_status, hee.entered_at,
                    l.car as vehicle, l.sim_type
             FROM hotlap_event_entries hee
             LEFT JOIN drivers d ON d.id = hee.driver_id
             LEFT JOIN laps l ON l.id = hee.lap_id
             WHERE hee.event_id = ?
             ORDER BY hee.position ASC",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let entries: Vec<Value> = entries_rows.into_iter().map(|(driver_id, display_name, lap_time_ms,
                                                              sector1_ms, sector2_ms, sector3_ms,
                                                              position, points, badge,
                                                              gap_to_leader_ms, within_107_percent,
                                                              result_status, entered_at,
                                                              vehicle, sim_type)| {
        json!({
            "driver_id": driver_id,
            "display_name": display_name,
            "lap_time_ms": lap_time_ms,
            "sector1_ms": sector1_ms,
            "sector2_ms": sector2_ms,
            "sector3_ms": sector3_ms,
            "position": position,
            "points": points,
            "badge": badge,
            "gap_to_leader_ms": gap_to_leader_ms,
            "within_107_percent": within_107_percent.map(|v| v == 1),
            "result_status": result_status,
            "entered_at": entered_at,
            "vehicle": vehicle,
            "sim_type": sim_type,
        })
    }).collect();

    // Determine car classes from actual laps
    let car_classes: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT l.car_class FROM hotlap_event_entries hee
         JOIN laps l ON l.id = hee.lap_id
         WHERE hee.event_id = ?",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let car_classes_list: Vec<&str> = car_classes.iter().map(|(c,)| c.as_str()).collect();

    Json(json!({
        "event": event,
        "car_classes": car_classes_list,
        "entries": entries,
    }))
}

// ─── Public Event Sessions (Group Racing) ────────────────────────────────────

/// GET /public/events/{id}/sessions — group session results linked to a hotlap event
/// Returns per-session multiplayer results with F1 points and gap-to-leader
pub(crate) async fn public_event_sessions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get group sessions linked to this event
    let sessions: Vec<(String, String, Option<String>, Option<String>)> = match sqlx::query_as(
        "SELECT gs.id, gs.status, gs.started_at, gs.completed_at
         FROM group_sessions gs
         WHERE gs.hotlap_event_id = ?
         ORDER BY gs.started_at DESC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let mut sessions_json: Vec<Value> = Vec::new();

    for (session_id, status, started_at, completed_at) in &sessions {
        // Fetch multiplayer results with PII-safe display name
        let results: Vec<(String, String, i64, Option<i64>, Option<i64>, i64, i64)> =
            sqlx::query_as(
                "SELECT mr.driver_id,
                        CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                             THEN d.nickname ELSE d.name END as display_name,
                        mr.position, mr.best_lap_ms, mr.total_time_ms, mr.laps_completed, mr.dnf
                 FROM multiplayer_results mr
                 LEFT JOIN drivers d ON d.id = mr.driver_id
                 WHERE mr.group_session_id = ?
                 ORDER BY mr.position ASC",
            )
            .bind(session_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

        // Find minimum best_lap_ms among non-DNF drivers for gap calculation
        let min_best_lap: Option<i64> = results
            .iter()
            .filter(|(_, _, _, _, _, _, dnf)| *dnf == 0)
            .filter_map(|(_, _, _, best_lap_ms, _, _, _)| *best_lap_ms)
            .min();

        use crate::lap_tracker::f1_points_for_position;

        let results_json: Vec<Value> = results.into_iter().map(|(driver_id, display_name, position,
                                                                  best_lap_ms, total_time_ms,
                                                                  laps_completed, dnf)| {
            let race_points = f1_points_for_position(position, dnf == 1);
            let gap_to_leader_ms: Option<i64> = match (best_lap_ms, min_best_lap, dnf) {
                (Some(bl), Some(min_bl), 0) => Some(bl - min_bl),
                _ => None,
            };
            json!({
                "position": position,
                "driver_id": driver_id,
                "display_name": display_name,
                "best_lap_ms": best_lap_ms,
                "total_time_ms": total_time_ms,
                "laps_completed": laps_completed,
                "dnf": dnf == 1,
                "race_points": race_points,
                "gap_to_leader_ms": gap_to_leader_ms,
            })
        }).collect();

        sessions_json.push(json!({
            "session_id": session_id,
            "status": status,
            "started_at": started_at,
            "completed_at": completed_at,
            "results": results_json,
        }));
    }

    Json(json!({
        "event_id": id,
        "sessions": sessions_json,
    }))
}

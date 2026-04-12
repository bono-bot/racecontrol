#![allow(unused_imports)]
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Public Lap Telemetry (No Auth Required) ────────────────────────────────

pub(crate) async fn public_lap_telemetry(
    State(state): State<Arc<AppState>>,
    Path(lap_id): Path<String>,
) -> Json<Value> {
    // First verify lap exists and get metadata
    let lap = sqlx::query_as::<_, (String, String, String, i64, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT track, car, sim_type, lap_time_ms, sector1_ms, sector2_ms, sector3_ms FROM laps WHERE id = ?",
    )
    .bind(&lap_id)
    .fetch_optional(&state.db)
    .await;

    let lap = match lap {
        Ok(Some(l)) => l,
        Ok(None) => return Json(json!({ "error": "Lap not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    // Phase 251: Fetch telemetry samples from telemetry.db if available, else fall back to main DB
    let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);
    let samples = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)>(
        "SELECT offset_ms, speed, throttle, brake, steering, gear, rpm
         FROM telemetry_samples
         WHERE lap_id = ?
         ORDER BY offset_ms ASC",
    )
    .bind(&lap_id)
    .fetch_all(telem_pool)
    .await;

    match samples {
        Ok(rows) => {
            let data: Vec<Value> = rows.iter().map(|s| {
                json!({
                    "offset_ms": s.0,
                    "speed": s.1,
                    "throttle": s.2,
                    "brake": s.3,
                    "steering": s.4,
                    "gear": s.5,
                    "rpm": s.6,
                })
            }).collect();

            let sample_count = data.len();
            Json(json!({
                "lap_id": lap_id,
                "track": lap.0,
                "car": lap.1,
                "sim_type": lap.2,
                "lap_time_ms": lap.3,
                "sector1_ms": lap.4,
                "sector2_ms": lap.5,
                "sector3_ms": lap.6,
                "samples": data,
                "sample_count": sample_count,
            }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Public Session Summary ──────────────────────────────────────────────────

/// Public session summary — no auth required. Shows first name only (privacy).
/// Used for shareable session links.
pub(crate) async fn public_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Query session + driver name + pricing tier (no auth - public endpoint)
    let row = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, d.name, bs.status, bs.allocated_seconds, bs.driving_seconds,
                pt.name, bs.car, bs.track, bs.sim_type
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match row {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Extract first name only (privacy -- per user decision)
    let first_name = session.1.split_whitespace().next().unwrap_or("Racer");

    // Best lap from laps table (valid laps only)
    let best_lap: Option<(i64,)> = sqlx::query_as(
        "SELECT MIN(lap_time_ms) FROM laps WHERE session_id = ? AND valid = 1",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let total_laps: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "driver_first_name": first_name,
        "status": session.2,
        "duration_seconds": session.4,
        "pricing_tier": session.5,
        "car": session.6,
        "track": session.7,
        "sim_type": session.8,
        "best_lap_ms": best_lap.map(|b| b.0),
        "total_laps": total_laps.map(|t| t.0).unwrap_or(0),
    }))
}

// ─── Public Championship Standings ───────────────────────────────────────────

/// GET /public/championships/{id}/standings — public championship standings with F1 tiebreaker
///
/// Returns championship metadata plus live-computed standings from hotlap_event_entries.
/// Standings are ordered by: total_points DESC, wins DESC, p2_count DESC, p3_count DESC.
pub(crate) async fn public_championship_standings_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Fetch championship metadata
    let champ = sqlx::query_as::<_, (String, String, Option<String>, String, String, i64, i64)>(
        "SELECT id, name, season, status, scoring_system, total_rounds, completed_rounds
         FROM championships WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let (champ_name, season, champ_status, scoring_system, total_rounds, completed_rounds) =
        match champ {
            Ok(Some((_, name, season, status, scoring, total, completed))) => {
                (name, season, status, scoring, total, completed)
            }
            Ok(None) => return Json(json!({ "error": "Championship not found" })),
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    // Compute standings live from hotlap_event_entries
    let standings_rows: Vec<(String, String, i64, i64, i64, i64, i64, Option<i64>)> =
        sqlx::query_as(
            "SELECT hee.driver_id,
                    COALESCE(d.nickname, d.name, 'Unknown') as display_name,
                    COALESCE(SUM(hee.points), 0) as total_points,
                    COUNT(DISTINCT cr.event_id) as rounds_entered,
                    SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                    SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2_count,
                    SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3_count,
                    MIN(hee.position) as best_result
             FROM hotlap_event_entries hee
             INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
             LEFT JOIN drivers d ON d.id = hee.driver_id
             WHERE cr.championship_id = ?
               AND hee.result_status IN ('finished', 'dnf', 'dns')
             GROUP BY hee.driver_id
             ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let standings: Vec<Value> = standings_rows
        .iter()
        .enumerate()
        .map(|(i, (driver_id, display_name, total_points, rounds_entered, wins, p2_count, p3_count, best_result))| {
            json!({
                "position": i as i64 + 1,
                "driver_id": driver_id,
                "display_name": display_name,
                "total_points": total_points,
                "rounds_entered": rounds_entered,
                "wins": wins,
                "p2_count": p2_count,
                "p3_count": p3_count,
                "best_result": best_result,
            })
        })
        .collect();

    // Fetch rounds list
    let rounds: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT cr.round_number, cr.event_id, he.name
         FROM championship_rounds cr
         LEFT JOIN hotlap_events he ON he.id = cr.event_id
         WHERE cr.championship_id = ?
         ORDER BY cr.round_number",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let rounds_json: Vec<Value> = rounds
        .iter()
        .map(|(num, evt_id, name)| {
            json!({
                "round_number": num,
                "event_id": evt_id,
                "event_name": name,
            })
        })
        .collect();

    Json(json!({
        "championship": {
            "id": id,
            "name": champ_name,
            "season": season,
            "status": champ_status,
            "scoring_system": scoring_system,
            "total_rounds": total_rounds,
            "completed_rounds": completed_rounds,
        },
        "standings": standings,
        "rounds": rounds_json,
    }))
}

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

// ─── Public Championships Endpoints ──────────────────────────────────────────

/// GET /public/championships — list all non-cancelled championships, active first
pub(crate) async fn public_championships_list(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows: Vec<(String, String, Option<String>, Option<String>, String, String, String, String, i64, i64, Option<String>)> =
        match sqlx::query_as(
            "SELECT c.id, c.name, c.description, c.season, c.car_class, c.sim_type,
                    c.status, c.scoring_system, c.total_rounds, c.completed_rounds, c.created_at
             FROM championships c
             WHERE c.status != 'cancelled'
             ORDER BY
               CASE c.status WHEN 'active' THEN 1 WHEN 'upcoming' THEN 2 WHEN 'completed' THEN 3 ELSE 4 END,
               c.created_at DESC",
        )
        .fetch_all(&state.db)
        .await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let championships: Vec<Value> = rows.into_iter().map(|(id, name, description, season,
                                                            car_class, sim_type, status,
                                                            scoring_system, total_rounds,
                                                            completed_rounds, created_at)| {
        json!({
            "id": id,
            "name": name,
            "description": description,
            "season": season,
            "car_class": car_class,
            "sim_type": sim_type,
            "status": status,
            "scoring_system": scoring_system,
            "total_rounds": total_rounds,
            "completed_rounds": completed_rounds,
            "created_at": created_at,
        })
    }).collect();

    Json(json!({ "championships": championships }))
}

/// GET /public/championships/{id} — championship metadata + standings + per-round breakdown
pub(crate) async fn public_championship_standings(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Fetch championship metadata
    let champ_row: Option<(String, String, Option<String>, Option<String>, String, String, String, String, i64, i64)> =
        match sqlx::query_as(
            "SELECT id, name, description, season, car_class, sim_type, status,
                    scoring_system, total_rounds, completed_rounds
             FROM championships WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let champ_row = match champ_row {
        Some(r) => r,
        None => return Json(json!({ "error": "Championship not found" })),
    };

    let championship = json!({
        "id": champ_row.0,
        "name": champ_row.1,
        "description": champ_row.2,
        "season": champ_row.3,
        "car_class": champ_row.4,
        "sim_type": champ_row.5,
        "status": champ_row.6,
        "scoring_system": champ_row.7,
        "total_rounds": champ_row.8,
        "completed_rounds": champ_row.9,
    });

    // Compute live standings (same tiebreaker as assign_championship_positions)
    let standings_rows: Vec<(String, String, i64, i64, i64, i64, i64, Option<i64>)> =
        sqlx::query_as(
            "SELECT hee.driver_id,
                    CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                         THEN d.nickname ELSE d.name END as display_name,
                    COALESCE(SUM(hee.points), 0) as total_points,
                    COUNT(DISTINCT cr.event_id) as rounds_entered,
                    SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                    SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2_count,
                    SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3_count,
                    MIN(hee.position) as best_result
             FROM hotlap_event_entries hee
             INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
             LEFT JOIN drivers d ON d.id = hee.driver_id
             WHERE cr.championship_id = ?
               AND hee.result_status IN ('finished', 'dnf', 'dns')
             GROUP BY hee.driver_id
             ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let standings: Vec<Value> = standings_rows
        .iter()
        .enumerate()
        .map(|(i, (driver_id, display_name, total_points, rounds_entered, wins, p2_count, p3_count, best_result))| {
            json!({
                "position": i as i64 + 1,
                "driver_id": driver_id,
                "display_name": display_name,
                "total_points": total_points,
                "rounds_entered": rounds_entered,
                "wins": wins,
                "p2_count": p2_count,
                "p3_count": p3_count,
                "best_result": best_result,
            })
        })
        .collect();

    // Per-round breakdown: for each round, driver results
    let round_rows: Vec<(i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>)> =
        sqlx::query_as(
            "SELECT cr.round_number, cr.event_id, he.name as event_name,
                    hee.driver_id, hee.points, hee.position, hee.result_status
             FROM championship_rounds cr
             INNER JOIN hotlap_events he ON he.id = cr.event_id
             LEFT JOIN hotlap_event_entries hee ON hee.event_id = cr.event_id
             WHERE cr.championship_id = ?
             ORDER BY cr.round_number, hee.position",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    // Group by (round_number, event_id, event_name)
    let mut rounds_map: std::collections::BTreeMap<i64, Value> = std::collections::BTreeMap::new();
    for (round_number, event_id, event_name, driver_id, points, position, result_status) in &round_rows {
        let entry = rounds_map.entry(*round_number).or_insert_with(|| {
            json!({
                "round_number": round_number,
                "event_id": event_id,
                "event_name": event_name,
                "results": [],
            })
        });
        if let Some(driver_id) = driver_id {
            if let Some(results) = entry.get_mut("results").and_then(|v| v.as_array_mut()) {
                results.push(json!({
                    "driver_id": driver_id,
                    "points": points,
                    "position": position,
                    "result_status": result_status,
                }));
            }
        }
    }
    let rounds: Vec<Value> = rounds_map.into_values().collect();

    Json(json!({
        "championship": championship,
        "standings": standings,
        "rounds": rounds,
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

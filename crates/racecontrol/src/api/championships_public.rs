#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

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
        if let Some(driver_id) = driver_id
            && let Some(results) = entry.get_mut("results").and_then(|v| v.as_array_mut()) {
                results.push(json!({
                    "driver_id": driver_id,
                    "points": points,
                    "position": position,
                    "result_status": result_status,
                }));
            }
    }
    let rounds: Vec<Value> = rounds_map.into_values().collect();

    Json(json!({
        "championship": championship,
        "standings": standings,
        "rounds": rounds,
    }))
}

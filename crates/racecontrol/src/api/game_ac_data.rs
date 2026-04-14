#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

/// AC Session Leaderboard — returns drivers ranked by best lap within an AC server session.
/// Finds all laps recorded on the session's pods during its active time window.
pub(crate) async fn ac_session_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // 1. Get the AC session record
    let session = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, config_json, status, pod_ids, started_at, ended_at, created_at FROM ac_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "AC session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (_id, config_json, status, pod_ids_str, started_at, ended_at, created_at) = session;

    // Parse config to get track/car info
    let track = config_json.as_deref()
        .and_then(|cj| serde_json::from_str::<Value>(cj).ok())
        .and_then(|v| v.get("track").and_then(|t| t.as_str().map(String::from)));

    // Parse pod_ids (comma-separated or JSON array)
    let pod_ids: Vec<String> = pod_ids_str
        .as_deref()
        .map(|s| {
            // Try JSON array first, fall back to comma-separated
            serde_json::from_str::<Vec<String>>(s)
                .unwrap_or_else(|_| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
        })
        .unwrap_or_default();

    if pod_ids.is_empty() {
        return Json(json!({
            "session_id": id, "status": status, "track": track,
            "started_at": started_at, "ended_at": ended_at, "created_at": created_at,
            "leaderboard": [], "total_laps": 0
        }));
    }

    // 2. Query laps on these pods during the session window
    let start_time = started_at.as_deref().unwrap_or(created_at.as_str());
    let end_time = ended_at.as_deref().unwrap_or("9999-12-31T23:59:59");

    // Use a CTE: find each driver's best lap, then join back for sectors.
    // The subquery LIMIT 1 ensures deterministic results when a driver has
    // multiple laps tied at the same best time.
    let placeholders = pod_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "WITH session_laps AS (
           SELECT l.id, l.driver_id, l.car, l.track, l.lap_time_ms,
                  l.sector1_ms, l.sector2_ms, l.sector3_ms, l.pod_id
           FROM laps l
           WHERE l.pod_id IN ({placeholders})
             AND l.created_at >= ?
             AND l.created_at <= ?
             AND l.valid = 1
             AND l.lap_time_ms > 0
         ),
         driver_best AS (
           SELECT driver_id, MIN(lap_time_ms) as best_lap_ms, COUNT(*) as lap_count
           FROM session_laps
           GROUP BY driver_id
         ),
         best_rows AS (
           SELECT db.driver_id, db.best_lap_ms, db.lap_count,
                  sl.car, sl.track,
                  sl.sector1_ms, sl.sector2_ms, sl.sector3_ms, sl.pod_id,
                  ROW_NUMBER() OVER (PARTITION BY db.driver_id ORDER BY sl.id) AS rn
           FROM driver_best db
           JOIN session_laps sl ON sl.driver_id = db.driver_id
                                AND sl.lap_time_ms = db.best_lap_ms
         )
         SELECT br.driver_id, d.name AS driver_name,
                br.car, br.track, br.best_lap_ms, br.lap_count,
                br.sector1_ms, br.sector2_ms, br.sector3_ms
         FROM best_rows br
         JOIN drivers d ON br.driver_id = d.id
         WHERE br.rn = 1
         ORDER BY br.best_lap_ms ASC
         LIMIT 50"
    );

    let mut q = sqlx::query(&sql);
    for pid in &pod_ids {
        q = q.bind(pid.as_str());
    }
    q = q.bind(start_time).bind(end_time);

    use sqlx::Row;
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(rows) => {
            let mut leaderboard: Vec<Value> = Vec::new();
            let mut best_time: Option<i64> = None;

            for (i, row) in rows.iter().enumerate() {
                let lap_ms: i64 = row.get("best_lap_ms");
                let gap_ms = best_time.map(|bt| lap_ms - bt);
                if best_time.is_none() {
                    best_time = Some(lap_ms);
                }

                leaderboard.push(json!({
                    "position": i + 1,
                    "driver_id": row.get::<String, _>("driver_id"),
                    "driver": row.get::<String, _>("driver_name"),
                    "car": row.get::<String, _>("car"),
                    "track": row.get::<String, _>("track"),
                    "best_lap_ms": lap_ms,
                    "lap_count": row.get::<i64, _>("lap_count"),
                    "sector1_ms": row.try_get::<Option<i64>, _>("sector1_ms").unwrap_or(None),
                    "sector2_ms": row.try_get::<Option<i64>, _>("sector2_ms").unwrap_or(None),
                    "sector3_ms": row.try_get::<Option<i64>, _>("sector3_ms").unwrap_or(None),
                    "gap_ms": gap_ms,
                }));
            }

            let total_laps: i64 = leaderboard.iter().map(|e| e["lap_count"].as_i64().unwrap_or(0)).sum();

            Json(json!({
                "session_id": id,
                "status": status,
                "track": track,
                "started_at": started_at,
                "ended_at": ended_at,
                "created_at": created_at,
                "pod_ids": pod_ids,
                "leaderboard": leaderboard,
                "total_laps": total_laps,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn list_ac_tracks(State(_state): State<Arc<AppState>>) -> Json<Value> {
    // Curated list of popular AC tracks
    Json(json!({ "tracks": [
        { "id": "monza", "name": "Monza", "configs": ["", "junior"] },
        { "id": "spa", "name": "Spa-Francorchamps", "configs": [""] },
        { "id": "silverstone", "name": "Silverstone", "configs": ["", "international", "national", "gp"] },
        { "id": "brands_hatch", "name": "Brands Hatch", "configs": ["", "gp", "indy"] },
        { "id": "nurburgring", "name": "Nurburgring", "configs": ["", "sprint"] },
        { "id": "nordschleife", "name": "Nordschleife", "configs": ["", "endurance", "tourist"] },
        { "id": "mugello", "name": "Mugello", "configs": [""] },
        { "id": "imola", "name": "Imola", "configs": [""] },
        { "id": "barcelona", "name": "Barcelona", "configs": ["", "moto", "national"] },
        { "id": "ks_red_bull_ring", "name": "Red Bull Ring", "configs": ["", "national"] },
        { "id": "vallelunga", "name": "Vallelunga", "configs": ["", "club"] },
        { "id": "drift", "name": "Drift Track", "configs": [""] },
        { "id": "ks_zandvoort", "name": "Zandvoort", "configs": [""] },
        { "id": "ks_laguna_seca", "name": "Laguna Seca", "configs": [""] },
        { "id": "suzuka", "name": "Suzuka", "configs": ["", "east"] },
        { "id": "ks_highlands", "name": "Highlands", "configs": [""] },
        { "id": "ks_black_cat_county", "name": "Black Cat County", "configs": ["", "long"] },
        { "id": "magione", "name": "Magione", "configs": [""] },
        { "id": "trento-bondone", "name": "Trento Bondone", "configs": [""] },
    ]}))
}

pub(crate) async fn list_ac_cars(State(_state): State<Arc<AppState>>) -> Json<Value> {
    // Curated list of popular AC cars grouped by class
    Json(json!({ "cars": [
        { "id": "ks_ferrari_488_gt3", "name": "Ferrari 488 GT3", "class": "GT3" },
        { "id": "ks_lamborghini_huracan_gt3", "name": "Lamborghini Huracan GT3", "class": "GT3" },
        { "id": "ks_mercedes_amg_gt3", "name": "Mercedes AMG GT3", "class": "GT3" },
        { "id": "ks_audi_r8_lms_2016", "name": "Audi R8 LMS 2016", "class": "GT3" },
        { "id": "ks_porsche_911_gt3_r_2016", "name": "Porsche 911 GT3 R", "class": "GT3" },
        { "id": "ks_mclaren_650_gt3", "name": "McLaren 650S GT3", "class": "GT3" },
        { "id": "ks_nissan_gtr_gt3", "name": "Nissan GT-R GT3", "class": "GT3" },
        { "id": "ks_bmw_m6_gt3", "name": "BMW M6 GT3", "class": "GT3" },
        { "id": "ks_ferrari_488_gtb", "name": "Ferrari 488 GTB", "class": "Street" },
        { "id": "ks_lamborghini_huracan_performante", "name": "Lamborghini Huracan Performante", "class": "Street" },
        { "id": "ks_porsche_911_r", "name": "Porsche 911 R", "class": "Street" },
        { "id": "ks_mclaren_p1", "name": "McLaren P1", "class": "Hypercar" },
        { "id": "ks_ferrari_laferrari", "name": "Ferrari LaFerrari", "class": "Hypercar" },
        { "id": "ks_porsche_918_spyder", "name": "Porsche 918 Spyder", "class": "Hypercar" },
        { "id": "ks_audi_r18_etron_quattro", "name": "Audi R18 e-tron", "class": "LMP" },
        { "id": "ks_porsche_919_hybrid_2016", "name": "Porsche 919 Hybrid", "class": "LMP" },
        { "id": "ks_toyota_ts040", "name": "Toyota TS040", "class": "LMP" },
        { "id": "tatuusfa1", "name": "Tatuus FA01", "class": "Open Wheel" },
        { "id": "ks_ferrari_sf15t", "name": "Ferrari SF15-T", "class": "Open Wheel" },
        { "id": "lotus_exos_125_s1", "name": "Lotus Exos 125 S1", "class": "Open Wheel" },
        { "id": "ks_mazda_mx5_cup", "name": "Mazda MX-5 Cup", "class": "Cup" },
        { "id": "ks_toyota_gt86", "name": "Toyota GT86", "class": "Street" },
        { "id": "ks_ford_mustang_2015", "name": "Ford Mustang 2015", "class": "Street" },
        { "id": "ks_abarth_595ss_s2", "name": "Abarth 595 SS", "class": "Street" },
        { "id": "lotus_2_eleven", "name": "Lotus 2-Eleven", "class": "Track Day" },
        { "id": "ks_toyota_ae86_drift", "name": "Toyota AE86 Drift", "class": "Drift" },
        { "id": "ks_nissan_370z", "name": "Nissan 370Z", "class": "Drift" },
    ]}))
}

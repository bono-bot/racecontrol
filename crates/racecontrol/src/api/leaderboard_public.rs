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

// ─── Public Leaderboard (No Auth Required) ───────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct PublicLeaderboardQuery {
    /// Filter by game/simulator (sim_type field)
    sim_type: Option<String>,
    /// Filter by car class (e.g. 'A', 'B', 'C') — UX-05 segmentation
    car_class: Option<String>,
    /// Filter by assist tier: 'pro', 'semi-pro', 'amateur', 'unknown' — UX-05 segmentation
    assist_tier: Option<String>,
}

pub(crate) async fn public_leaderboard(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PublicLeaderboardQuery>,
) -> Json<Value> {
    // UX-04: Only show laps from billing sessions (verified)
    // UX-05: Segment by game + car_class + assist_tier
    // UX-07: Never show laps marked unverifiable (telemetry adapter crash)
    let sim_clause = if params.sim_type.is_some() { " AND tr.sim_type = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { " AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { " AND l.assist_tier = ?" } else { "" };

    // All-time track records, filtered by game + car_class + assist_tier
    // JOIN laps to apply UX-04/UX-05/UX-07 integrity filters
    let records_query = format!(
        "SELECT tr.track, tr.car,
                CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                tr.best_lap_ms, tr.achieved_at, tr.lap_id, tr.sim_type
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         LEFT JOIN laps l ON l.id = tr.lap_id
         WHERE (l.billing_session_id IS NOT NULL OR tr.lap_id IS NULL)
           AND (l.validity IS NULL OR l.validity = 'valid')
           AND (l.suspect IS NULL OR l.suspect = 0)
           {}{}{}
         ORDER BY tr.achieved_at DESC",
        sim_clause, car_class_clause, assist_tier_clause
    );

    let mut rec_q = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, String)>(&records_query);
    if let Some(ref st) = params.sim_type { rec_q = rec_q.bind(st); }
    if let Some(ref cc) = params.car_class { rec_q = rec_q.bind(cc); }
    if let Some(ref at) = params.assist_tier { rec_q = rec_q.bind(at); }
    let records = rec_q.fetch_all(&state.db).await;

    // Available tracks — only tracks with billing-verified valid laps
    let laps_sim_clause = if params.sim_type.is_some() { " AND sim_type = ?" } else { "" };
    let laps_cc_clause = if params.car_class.is_some() { " AND car_class = ?" } else { "" };
    let laps_at_clause = if params.assist_tier.is_some() { " AND assist_tier = ?" } else { "" };
    let tracks_query = format!(
        "SELECT DISTINCT track, COUNT(*) as laps FROM laps
         WHERE valid = 1
           AND billing_session_id IS NOT NULL
           AND (validity IS NULL OR validity = 'valid')
           AND (suspect IS NULL OR suspect = 0)
           {}{}{}
         GROUP BY track ORDER BY laps DESC",
        laps_sim_clause, laps_cc_clause, laps_at_clause
    );
    let mut track_q = sqlx::query_as::<_, (String, i64)>(&tracks_query);
    if let Some(ref st) = params.sim_type { track_q = track_q.bind(st); }
    if let Some(ref cc) = params.car_class { track_q = track_q.bind(cc); }
    if let Some(ref at) = params.assist_tier { track_q = track_q.bind(at); }
    let tracks = track_q.fetch_all(&state.db).await.unwrap_or_default();

    // Top drivers by total valid billing-session laps, optionally filtered
    let top_drivers_query = format!(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                COUNT(l.id) as lap_count, MIN(l.lap_time_ms) as fastest,
                MAX(dr.composite_rating),
                (SELECT dr2.rating_class FROM driver_ratings dr2 WHERE dr2.driver_id = l.driver_id ORDER BY dr2.composite_rating DESC LIMIT 1)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         LEFT JOIN driver_ratings dr ON dr.driver_id = l.driver_id AND dr.sim_type = l.sim_type
         WHERE l.valid = 1
           AND l.billing_session_id IS NOT NULL
           AND (l.validity IS NULL OR l.validity = 'valid')
           AND (l.suspect IS NULL OR l.suspect = 0)
           {}{}{}
         GROUP BY l.driver_id ORDER BY lap_count DESC LIMIT 20",
        laps_sim_clause, laps_cc_clause, laps_at_clause
    );
    let mut td_q = sqlx::query_as::<_, (String, i64, Option<i64>, Option<f64>, Option<String>)>(&top_drivers_query);
    if let Some(ref st) = params.sim_type { td_q = td_q.bind(st); }
    if let Some(ref cc) = params.car_class { td_q = td_q.bind(cc); }
    if let Some(ref at) = params.assist_tier { td_q = td_q.bind(at); }
    let top_drivers = td_q.fetch_all(&state.db).await.unwrap_or_default();

    // Available sim_types for frontend game picker (billing-verified only)
    let available_sim_types: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT sim_type FROM laps
         WHERE valid = 1 AND billing_session_id IS NOT NULL
           AND (validity IS NULL OR validity = 'valid')
         ORDER BY sim_type",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    // Available assist tiers for frontend assist picker
    let available_assist_tiers: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT assist_tier FROM laps
         WHERE valid = 1 AND billing_session_id IS NOT NULL
           AND (validity IS NULL OR validity = 'valid')
           AND assist_tier IS NOT NULL
         ORDER BY assist_tier",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    // Active time trial
    let time_trial = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end
         FROM time_trials
         WHERE date('now') BETWEEN week_start AND week_end
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "records": records.unwrap_or_default().iter().map(|r| json!({
            "track": r.0, "car": r.1, "driver": r.2,
            "best_lap_ms": r.3,
            "best_lap_display": format!("{}:{:02}.{:03}", r.3 / 60000, (r.3 % 60000) / 1000, r.3 % 1000),
            "achieved_at": r.4,
            "lap_id": r.5,
            "sim_type": r.6,
        })).collect::<Vec<_>>(),
        "tracks": tracks.iter().map(|t| json!({
            "name": t.0, "total_laps": t.1,
        })).collect::<Vec<_>>(),
        "top_drivers": top_drivers.iter().enumerate().map(|(i, d)| json!({
            "position": i + 1,
            "name": d.0,
            "total_laps": d.1,
            "fastest_lap_ms": d.2,
            "composite_rating": d.3,
            "rating_class": d.4,
        })).collect::<Vec<_>>(),
        "available_sim_types": available_sim_types,
        "available_assist_tiers": available_assist_tiers,
        "sim_type": params.sim_type,
        "car_class": params.car_class,
        "assist_tier": params.assist_tier,
        "time_trial": time_trial.map(|tt| json!({
            "id": tt.0, "track": tt.1, "car": tt.2,
            "week_start": tt.3, "week_end": tt.4,
        })),
        "venue": "RacingPoint",
        "tagline": "May the Fastest Win.",
        "last_updated": chrono::Utc::now().to_rfc3339(),
    }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct LeaderboardQuery {
    sim_type: Option<String>,
    car: Option<String>,
    /// Filter by car class — UX-05 segmentation
    car_class: Option<String>,
    /// Filter by assist tier: 'pro', 'semi-pro', 'amateur' — UX-05 segmentation
    assist_tier: Option<String>,
    show_invalid: Option<bool>,
}

pub(crate) async fn public_track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
    Query(params): Query<LeaderboardQuery>,
) -> Json<Value> {
    // sim_type is optional — None means all games (backward compatible)
    let sim_type = params.sim_type.clone();
    let show_invalid = params.show_invalid.unwrap_or(false);

    // Build validity filter: suspect laps are ALWAYS hidden.
    // show_invalid=true drops the valid=1 requirement but keeps suspect filter.
    // UX-04: billing_session_id IS NOT NULL — only billed-session laps on leaderboard
    // UX-07: validity = 'valid' — never show unverifiable laps
    let validity_clause = if show_invalid {
        "AND (l.suspect IS NULL OR l.suspect = 0) AND l.billing_session_id IS NOT NULL AND (l.validity IS NULL OR l.validity = 'valid')"
    } else {
        "AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0) AND l.billing_session_id IS NOT NULL AND (l.validity IS NULL OR l.validity = 'valid')"
    };

    let sim_type_clause = if sim_type.is_some() { "AND l.sim_type = ?" } else { "" };
    let sim_type_subq_clause = if sim_type.is_some() { "AND l2.sim_type = ?" } else { "" };
    let car_clause = if params.car.is_some() { "AND l.car = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { "AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { "AND l.assist_tier = ?" } else { "" };

    // Top 50 fastest laps on this track (best per driver per car)
    // UX-04: billing_session_id enforced via validity_clause
    // UX-05: car_class + assist_tier segmentation
    // UX-07: validity = 'valid' enforced via validity_clause
    // Phase 253: LEFT JOIN driver_ratings to include composite_rating and rating_class
    // Response includes assist_tier for frontend display
    let main_query = format!(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                l.car, MIN(l.lap_time_ms), MAX(l.created_at),
                (SELECT l2.id FROM laps l2 WHERE l2.driver_id = l.driver_id AND l2.car = l.car AND l2.track = l.track
                    {} {} ORDER BY l2.lap_time_ms ASC LIMIT 1),
                l.sim_type,
                dr.composite_rating,
                dr.rating_class,
                l.assist_tier
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         LEFT JOIN driver_ratings dr ON dr.driver_id = l.driver_id AND dr.sim_type = l.sim_type
         WHERE l.track = ? {} {} {} {} {}
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50",
        sim_type_subq_clause,
        if show_invalid { "AND (l2.suspect IS NULL OR l2.suspect = 0)" } else { "AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)" },
        sim_type_clause,
        validity_clause,
        car_clause,
        car_class_clause,
        assist_tier_clause,
    );

    let mut query = sqlx::query_as::<_, (String, String, i64, String, Option<String>, String, Option<f64>, Option<String>, Option<String>)>(&main_query);

    // Bind subquery sim_type first (if present)
    if let Some(ref st) = sim_type {
        query = query.bind(st);
    }
    // Bind main WHERE params
    query = query.bind(&track);
    if let Some(ref st) = sim_type {
        query = query.bind(st);
    }
    if let Some(ref car) = params.car {
        query = query.bind(car);
    }
    if let Some(ref cc) = params.car_class {
        query = query.bind(cc);
    }
    if let Some(ref at) = params.assist_tier {
        query = query.bind(at);
    }

    let records = query.fetch_all(&state.db).await;

    // Track stats (filtered by same criteria including UX-04/UX-07)
    let stats_query = format!(
        "SELECT COUNT(*) as total_laps, COUNT(DISTINCT driver_id) as drivers, COUNT(DISTINCT car) as cars
         FROM laps WHERE track = ? {} {} {} {}",
        sim_type_clause,
        validity_clause,
        car_class_clause,
        assist_tier_clause,
    );

    let stats: Option<(i64, i64, i64)> = {
        let mut sq = sqlx::query_as::<_, (i64, i64, i64)>(&stats_query).bind(&track);
        if let Some(ref st) = sim_type {
            sq = sq.bind(st);
        }
        if let Some(ref cc) = params.car_class {
            sq = sq.bind(cc);
        }
        if let Some(ref at) = params.assist_tier {
            sq = sq.bind(at);
        }
        sq.fetch_optional(&state.db).await.ok().flatten()
    };

    Json(json!({
        "track": track,
        "sim_type": sim_type,
        "car_class": params.car_class,
        "assist_tier": params.assist_tier,
        "stats": stats.map(|s| json!({
            "total_laps": s.0,
            "unique_drivers": s.1,
            "unique_cars": s.2,
        })),
        "leaderboard": records.unwrap_or_default().iter().enumerate().map(|(i, r)| json!({
            "position": i + 1,
            "driver": r.0,
            "car": r.1,
            "best_lap_ms": r.2,
            "best_lap_display": format!("{}:{:02}.{:03}", r.2 / 60000, (r.2 % 60000) / 1000, r.2 % 1000),
            "achieved_at": r.3,
            "lap_id": r.4,
            "sim_type": r.5,
            "composite_rating": r.6,
            "rating_class": r.7,
            "assist_tier": r.8,
        })).collect::<Vec<_>>(),
        "last_updated": chrono::Utc::now().to_rfc3339(),
    }))
}

// ─── Circuit Records (Public, No Auth) ────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct CircuitRecordsQuery {
    sim_type: Option<String>,
}

pub(crate) async fn public_circuit_records(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CircuitRecordsQuery>,
) -> Json<Value> {
    let records = if let Some(ref sim_type) = params.sim_type {
        sqlx::query_as::<_, (String, String, String, i64, String)>(
            "SELECT l.track, l.car, l.sim_type, MIN(l.lap_time_ms),
                    (SELECT CASE WHEN d2.show_nickname_on_leaderboard = 1 AND d2.nickname IS NOT NULL THEN d2.nickname ELSE d2.name END
                     FROM laps l2 JOIN drivers d2 ON l2.driver_id = d2.id
                     WHERE l2.track = l.track AND l2.car = l.car AND l2.sim_type = l.sim_type
                       AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)
                     ORDER BY l2.lap_time_ms ASC LIMIT 1)
             FROM laps l
             WHERE l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0) AND l.sim_type = ?
             GROUP BY l.track, l.car, l.sim_type
             ORDER BY l.track, l.car",
        )
        .bind(sim_type)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, i64, String)>(
            "SELECT l.track, l.car, l.sim_type, MIN(l.lap_time_ms),
                    (SELECT CASE WHEN d2.show_nickname_on_leaderboard = 1 AND d2.nickname IS NOT NULL THEN d2.nickname ELSE d2.name END
                     FROM laps l2 JOIN drivers d2 ON l2.driver_id = d2.id
                     WHERE l2.track = l.track AND l2.car = l.car AND l2.sim_type = l.sim_type
                       AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)
                     ORDER BY l2.lap_time_ms ASC LIMIT 1)
             FROM laps l
             WHERE l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
             GROUP BY l.track, l.car, l.sim_type
             ORDER BY l.track, l.car",
        )
        .fetch_all(&state.db)
        .await
    };

    let records = records.unwrap_or_default();
    let count = records.len();

    Json(json!({
        "records": records.iter().map(|r| json!({
            "track": r.0,
            "car": r.1,
            "sim_type": r.2,
            "best_lap_ms": r.3,
            "best_lap_display": format!("{}:{:02}.{:03}", r.3 / 60000, (r.3 % 60000) / 1000, r.3 % 1000),
            "driver": r.4,
        })).collect::<Vec<_>>(),
        "count": count,
    }))
}

pub(crate) async fn public_time_trial(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Current week's time trial
    let trial = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end
         FROM time_trials
         WHERE date('now') BETWEEN week_start AND week_end
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let trial = match trial {
        Some(t) => t,
        None => return Json(json!({ "time_trial": null, "message": "No active time trial this week" })),
    };

    // Leaderboard for this time trial (laps on this track+car this week)
    let entries = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, MIN(l.lap_time_ms), COUNT(l.id)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = ? AND l.car = ? AND l.valid = 1
           AND l.created_at >= ? AND l.created_at < datetime(?, '+1 day')
         GROUP BY l.driver_id
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 20",
    )
    .bind(&trial.1)
    .bind(&trial.2)
    .bind(&trial.3)
    .bind(&trial.4)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "time_trial": {
            "id": trial.0,
            "track": trial.1,
            "car": trial.2,
            "week_start": trial.3,
            "week_end": trial.4,
        },
        "leaderboard": entries.iter().enumerate().map(|(i, e)| json!({
            "position": i + 1,
            "driver": e.0,
            "best_lap_ms": e.1,
            "best_lap_display": format!("{}:{:02}.{:03}", e.1 / 60000, (e.1 % 60000) / 1000, e.1 % 1000),
            "attempts": e.2,
        })).collect::<Vec<_>>(),
    }))
}

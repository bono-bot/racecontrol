#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
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

// ─── Time Trial Admin ────────────────────────────────────────────────────────

pub(crate) async fn list_time_trials(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, track, car, week_start, week_end, is_active
         FROM time_trials ORDER BY week_start DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(trials) => {
            let list: Vec<Value> = trials.iter().map(|t| json!({
                "id": t.0, "track": t.1, "car": t.2,
                "week_start": t.3, "week_end": t.4, "is_active": t.5,
            })).collect();
            Json(json!({ "time_trials": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_time_trial(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let track = match body.get("track").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "track required" })),
    };
    let car = match body.get("car").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "car required" })),
    };
    let week_start = match body.get("week_start").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return Json(json!({ "error": "week_start required (YYYY-MM-DD)" })),
    };
    let week_end = match body.get("week_end").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return Json(json!({ "error": "week_end required (YYYY-MM-DD)" })),
    };

    let result = sqlx::query(
        "INSERT INTO time_trials (id, track, car, week_start, week_end) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(track)
    .bind(car)
    .bind(week_start)
    .bind(week_end)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn update_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE time_trials SET
            track = COALESCE(?, track), car = COALESCE(?, car),
            week_start = COALESCE(?, week_start), week_end = COALESCE(?, week_end),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("week_start").and_then(|v| v.as_str()))
    .bind(body.get("week_end").and_then(|v| v.as_str()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn delete_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM time_trials WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;
    Json(json!({ "ok": true }))
}

// ─── Tournaments ─────────────────────────────────────────────────────────────

pub(crate) async fn list_tournaments(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise,
                status, registration_start, registration_end, event_date
         FROM tournaments ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tournaments) => {
            let list: Vec<Value> = tournaments.iter().map(|t| json!({
                "id": t.0, "name": t.1, "description": t.2,
                "track": t.3, "car": t.4, "format": t.5,
                "max_participants": t.6, "entry_fee_paise": t.7,
                "prize_pool_paise": t.8, "status": t.9,
                "registration_start": t.10, "registration_end": t.11,
                "event_date": t.12,
            })).collect();
            Json(json!({ "tournaments": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_tournament(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return Json(json!({ "error": "name required" })),
    };

    let result = sqlx::query(
        "INSERT INTO tournaments (id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise, status, registration_start, registration_end, event_date, rules, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'upcoming', ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(body.get("description").and_then(|v| v.as_str()))
    .bind(body.get("track").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(body.get("car").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(body.get("format").and_then(|v| v.as_str()).unwrap_or("time_attack"))
    .bind(body.get("max_participants").and_then(|v| v.as_i64()).unwrap_or(16))
    .bind(body.get("entry_fee_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(body.get("prize_pool_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(body.get("registration_start").and_then(|v| v.as_str()))
    .bind(body.get("registration_end").and_then(|v| v.as_str()))
    .bind(body.get("event_date").and_then(|v| v.as_str()))
    .bind(body.get("rules").and_then(|v| v.as_str()))
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn get_tournament(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let tournament = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise,
                status, registration_start, registration_end, event_date, rules
         FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let t = match tournament {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Tournament not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Count registrations
    let reg_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    Json(json!({
        "tournament": {
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6, "entry_fee_paise": t.7,
            "prize_pool_paise": t.8, "status": t.9,
            "registration_start": t.10, "registration_end": t.11,
            "event_date": t.12, "rules": t.13,
            "registered_count": reg_count,
        }
    }))
}

pub(crate) async fn update_tournament(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE tournaments SET
            name = COALESCE(?, name), description = ?,
            track = COALESCE(?, track), car = COALESCE(?, car),
            format = COALESCE(?, format), max_participants = COALESCE(?, max_participants),
            entry_fee_paise = COALESCE(?, entry_fee_paise),
            prize_pool_paise = COALESCE(?, prize_pool_paise),
            status = COALESCE(?, status),
            registration_start = ?, registration_end = ?, event_date = ?,
            rules = ?
         WHERE id = ?",
    )
    .bind(body.get("name").and_then(|v| v.as_str()))
    .bind(body.get("description").and_then(|v| v.as_str()))
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("format").and_then(|v| v.as_str()))
    .bind(body.get("max_participants").and_then(|v| v.as_i64()))
    .bind(body.get("entry_fee_paise").and_then(|v| v.as_i64()))
    .bind(body.get("prize_pool_paise").and_then(|v| v.as_i64()))
    .bind(body.get("status").and_then(|v| v.as_str()))
    .bind(body.get("registration_start").and_then(|v| v.as_str()))
    .bind(body.get("registration_end").and_then(|v| v.as_str()))
    .bind(body.get("event_date").and_then(|v| v.as_str()))
    .bind(body.get("rules").and_then(|v| v.as_str()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn tournament_registrations(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<i64>, String, Option<i64>)>(
        "SELECT tr.id, tr.driver_id, d.name, tr.seed, tr.status, tr.best_time_ms
         FROM tournament_registrations tr
         JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.tournament_id = ?
         ORDER BY COALESCE(tr.seed, 9999), tr.created_at",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(regs) => {
            let list: Vec<Value> = regs.iter().map(|r| json!({
                "id": r.0, "driver_id": r.1, "driver_name": r.2,
                "seed": r.3, "status": r.4, "best_time_ms": r.5,
            })).collect();
            Json(json!({ "registrations": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn tournament_matches(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, i64, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>, String)>(
        "SELECT tm.id, tm.round, tm.match_number,
                da.name, db.name,
                tm.time_a_ms, tm.time_b_ms, dw.name, tm.status
         FROM tournament_matches tm
         LEFT JOIN drivers da ON tm.driver_a = da.id
         LEFT JOIN drivers db ON tm.driver_b = db.id
         LEFT JOIN drivers dw ON tm.winner_id = dw.id
         WHERE tm.tournament_id = ?
         ORDER BY tm.round, tm.match_number",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(matches) => {
            let list: Vec<Value> = matches.iter().map(|m| json!({
                "id": m.0, "round": m.1, "match_number": m.2,
                "driver_a": m.3, "driver_b": m.4,
                "time_a_ms": m.5, "time_b_ms": m.6,
                "winner": m.7, "status": m.8,
            })).collect();
            Json(json!({ "matches": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn generate_bracket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get all registered drivers
    let regs = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT driver_id, seed FROM tournament_registrations
         WHERE tournament_id = ? AND status IN ('registered', 'checked_in')
         ORDER BY COALESCE(seed, 9999), created_at",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    let regs = match regs {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    if regs.len() < 2 {
        return Json(json!({ "error": "Need at least 2 registrations" }));
    }

    // Delete existing matches
    let _ = sqlx::query("DELETE FROM tournament_matches WHERE tournament_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    // Generate round 1 matches (pair sequential registrations)
    let mut match_count = 0;
    let mut i = 0;
    while i < regs.len() {
        let driver_a = &regs[i].0;
        let driver_b = if i + 1 < regs.len() {
            Some(&regs[i + 1].0)
        } else {
            None // Bye
        };

        match_count += 1;
        let match_id = uuid::Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO tournament_matches (id, tournament_id, round, match_number, driver_a, driver_b, status, venue_id)
             VALUES (?, ?, 1, ?, ?, ?, ?, ?)",
        )
        .bind(&match_id)
        .bind(&id)
        .bind(match_count as i64)
        .bind(driver_a)
        .bind(driver_b)
        .bind(if driver_b.is_some() { "pending" } else { "completed" })
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;

        // Auto-advance bye
        if driver_b.is_none() {
            let _ = sqlx::query(
                "UPDATE tournament_matches SET winner_id = ?, status = 'completed' WHERE id = ?",
            )
            .bind(driver_a)
            .bind(&match_id)
            .execute(&state.db)
            .await;
        }

        i += 2;
    }

    // Update tournament status
    let _ = sqlx::query("UPDATE tournaments SET status = 'in_progress' WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Json(json!({ "ok": true, "round_1_matches": match_count }))
}

pub(crate) async fn record_match_result(
    State(state): State<Arc<AppState>>,
    Path((tournament_id, match_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let winner_id = match body.get("winner_id").and_then(|v| v.as_str()) {
        Some(w) => w,
        None => return Json(json!({ "error": "winner_id required" })),
    };

    let _ = sqlx::query(
        "UPDATE tournament_matches SET
            winner_id = ?, status = 'completed', completed_at = datetime('now'),
            time_a_ms = ?, time_b_ms = ?
         WHERE id = ? AND tournament_id = ?",
    )
    .bind(winner_id)
    .bind(body.get("time_a_ms").and_then(|v| v.as_i64()))
    .bind(body.get("time_b_ms").and_then(|v| v.as_i64()))
    .bind(&match_id)
    .bind(&tournament_id)
    .execute(&state.db)
    .await;

    // Check if all matches in current round are done, generate next round
    let current_round: Option<(i64,)> = sqlx::query_as(
        "SELECT round FROM tournament_matches WHERE id = ?",
    )
    .bind(&match_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((round,)) = current_round {
        let pending: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM tournament_matches
             WHERE tournament_id = ? AND round = ? AND status != 'completed'",
        )
        .bind(&tournament_id)
        .bind(round)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if pending.map(|p| p.0 == 0).unwrap_or(false) {
            // All done in this round — get winners and create next round
            let winners = sqlx::query_as::<_, (String,)>(
                "SELECT winner_id FROM tournament_matches
                 WHERE tournament_id = ? AND round = ? AND winner_id IS NOT NULL
                 ORDER BY match_number",
            )
            .bind(&tournament_id)
            .bind(round)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            if winners.len() > 1 {
                let next_round = round + 1;
                let mut match_num = 0;
                let mut i = 0;
                while i < winners.len() {
                    match_num += 1;
                    let driver_a = &winners[i].0;
                    let driver_b = if i + 1 < winners.len() {
                        Some(&winners[i + 1].0)
                    } else {
                        None
                    };

                    let mid = uuid::Uuid::new_v4().to_string();
                    let _ = sqlx::query(
                        "INSERT INTO tournament_matches (id, tournament_id, round, match_number, driver_a, driver_b, status, venue_id)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&mid)
                    .bind(&tournament_id)
                    .bind(next_round)
                    .bind(match_num as i64)
                    .bind(driver_a)
                    .bind(driver_b)
                    .bind(if driver_b.is_some() { "pending" } else { "completed" })
                    .bind(&state.config.venue.venue_id)
                    .execute(&state.db)
                    .await;

                    if driver_b.is_none() {
                        let _ = sqlx::query(
                            "UPDATE tournament_matches SET winner_id = ?, status = 'completed' WHERE id = ?",
                        )
                        .bind(driver_a)
                        .bind(&mid)
                        .execute(&state.db)
                        .await;
                    }
                    i += 2;
                }
            } else if winners.len() == 1 {
                // Tournament complete!
                let _ = sqlx::query("UPDATE tournaments SET status = 'completed' WHERE id = ?")
                    .bind(&tournament_id)
                    .execute(&state.db)
                    .await;
                let _ = sqlx::query(
                    "UPDATE tournament_registrations SET status = 'winner' WHERE tournament_id = ? AND driver_id = ?",
                )
                .bind(&tournament_id)
                .bind(&winners[0].0)
                .execute(&state.db)
                .await;
            }
        }
    }

    Json(json!({ "ok": true }))
}

// ─── Customer Tournament Endpoints ──────────────────────────────────────────

pub(crate) async fn customer_list_tournaments(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants,
                entry_fee_paise, prize_pool_paise, status, event_date
         FROM tournaments
         WHERE status IN ('upcoming', 'registration', 'in_progress')
         ORDER BY event_date ASC",
    )
    .fetch_all(&state.db)
    .await;

    let tournaments = match rows {
        Ok(t) => t,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Check which the driver is registered for
    let registered: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT tournament_id FROM tournament_registrations WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    let list: Vec<Value> = tournaments.iter().map(|t| {
        json!({
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6,
            "entry_fee_display": if t.7 > 0 { format!("Rs.{}", t.7 / 100) } else { "Free".to_string() },
            "prize_pool_display": if t.8 > 0 { format!("Rs.{}", t.8 / 100) } else { "TBD".to_string() },
            "status": t.9, "event_date": t.10,
            "is_registered": registered.contains(&t.0),
        })
    }).collect();

    Json(json!({ "tournaments": list }))
}

pub(crate) async fn customer_register_tournament(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check tournament exists and is open
    let status: Option<(String, i64)> = sqlx::query_as(
        "SELECT status, max_participants FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match &status {
        Some((s, _)) if s != "registration" && s != "upcoming" => {
            return Json(json!({ "error": "Registration is not open" }));
        }
        None => return Json(json!({ "error": "Tournament not found" })),
        _ => {}
    }

    let max = status.unwrap().1;

    // Check capacity
    let count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    if count >= max {
        return Json(json!({ "error": "Tournament is full" }));
    }

    let reg_id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO tournament_registrations (id, tournament_id, driver_id, venue_id) VALUES (?, ?, ?, ?)",
    )
    .bind(&reg_id)
    .bind(&id)
    .bind(&driver_id)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true, "registration_id": reg_id })),
        Err(e) if e.to_string().contains("UNIQUE") => {
            Json(json!({ "error": "Already registered" }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

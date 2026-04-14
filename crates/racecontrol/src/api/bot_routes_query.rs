#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::wallet;
use crate::state::AppState;

use super::bot_routes::validate_bot_secret;

// ─── Bot: pods-status ────────────────────────────────────────────────────

pub(crate) async fn bot_pods_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let pods = state.pods.read().await;
    let total = pods.len();
    let available = pods.values()
        .filter(|p| p.status == rc_common::types::PodStatus::Idle && p.billing_session_id.is_none())
        .count();
    let in_use = pods.values()
        .filter(|p| p.status == rc_common::types::PodStatus::InSession)
        .count();

    Json(json!({
        "total": total,
        "available": available,
        "in_use": in_use,
        "message": format!("{} of {} rigs are free right now", available, total),
    }))
}

// ─── Bot: events ─────────────────────────────────────────────────────────

pub(crate) async fn bot_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Upcoming/active tournaments
    let tournaments = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, name, status, event_date FROM tournaments
         WHERE status IN ('upcoming', 'registration', 'in_progress')
         ORDER BY event_date ASC LIMIT 5"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Active time trials (current week or future)
    let time_trials = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end FROM time_trials
         WHERE is_active = 1 AND week_end >= date('now')
         ORDER BY week_start ASC LIMIT 5"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let t_list: Vec<Value> = tournaments.iter().map(|t| json!({
        "id": t.0, "name": t.1, "status": t.2, "event_date": t.3,
    })).collect();

    let tt_list: Vec<Value> = time_trials.iter().map(|t| json!({
        "id": t.0, "track": t.1, "car": t.2, "week_start": t.3, "week_end": t.4,
    })).collect();

    Json(json!({
        "tournaments": t_list,
        "time_trials": tt_list,
        "has_events": !t_list.is_empty() || !tt_list.is_empty(),
    }))
}

// ─── Bot: leaderboard ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BotLeaderboardQuery {
    track: Option<String>,
    sim_type: Option<String>,
}

pub(crate) async fn bot_leaderboard(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotLeaderboardQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let entries: Result<Vec<(String, String, i64, String)>, _> = if let Some(ref track) = params.track {
        // Track-specific: query laps directly
        if let Some(ref st) = params.sim_type {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, l.track, MIN(l.lap_time_ms) as best_time, l.car
                 FROM laps l
                 JOIN drivers d ON l.driver_id = d.id
                 WHERE l.track = ? AND l.sim_type = ? AND l.lap_time_ms > 0
                 GROUP BY l.driver_id, l.track
                 ORDER BY best_time ASC LIMIT 10"
            )
            .bind(track)
            .bind(st)
            .fetch_all(&state.db)
            .await
        } else {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, l.track, MIN(l.lap_time_ms) as best_time, l.car
                 FROM laps l
                 JOIN drivers d ON l.driver_id = d.id
                 WHERE l.track = ? AND l.lap_time_ms > 0
                 GROUP BY l.driver_id, l.track
                 ORDER BY best_time ASC LIMIT 10"
            )
            .bind(track)
            .fetch_all(&state.db)
            .await
        }
    } else {
        // All-tracks: query track_records
        if let Some(ref st) = params.sim_type {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, tr.track, tr.best_lap_ms, tr.car
                 FROM track_records tr
                 JOIN drivers d ON tr.driver_id = d.id
                 WHERE tr.sim_type = ?
                 ORDER BY tr.best_lap_ms ASC LIMIT 10"
            )
            .bind(st)
            .fetch_all(&state.db)
            .await
        } else {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, tr.track, tr.best_lap_ms, tr.car
                 FROM track_records tr
                 JOIN drivers d ON tr.driver_id = d.id
                 ORDER BY tr.best_lap_ms ASC LIMIT 10"
            )
            .fetch_all(&state.db)
            .await
        }
    };

    let list: Vec<Value> = entries.unwrap_or_default().iter().enumerate().map(|(i, e)| json!({
        "position": i + 1,
        "driver": e.0,
        "track": e.1,
        "time_ms": e.2,
        "time_formatted": format!("{}:{:02}.{:03}", e.2 / 60000, (e.2 % 60000) / 1000, e.2 % 1000),
        "car": e.3,
    })).collect();

    let count = list.len();
    Json(json!({
        "entries": list,
        "track_filter": params.track,
        "sim_type": params.sim_type,
        "count": count,
        "last_updated": chrono::Utc::now().to_rfc3339(),
    }))
}

// ─── Bot: customer-stats ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BotCustomerStatsQuery {
    phone: String,
}

pub(crate) async fn bot_customer_stats(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotCustomerStatsQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let phone = params.phone.trim();

    let ph = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT id, name, COALESCE(total_laps, 0), COALESCE(total_time_ms, 0)
         FROM drivers WHERE phone_hash = ?"
    )
    .bind(&ph)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some((id, name, laps, time_ms))) => {
            let sessions = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM billing_sessions
                 WHERE driver_id = ? AND status IN ('completed', 'in_progress')"
            )
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map(|r| r.0)
            .unwrap_or(0);

            let pbs = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?"
            )
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map(|r| r.0)
            .unwrap_or(0);

            let balance = wallet::get_balance(&state, &id).await.unwrap_or(0);

            Json(json!({
                "found": true,
                "name": name,
                "total_laps": laps,
                "total_sessions": sessions,
                "total_time_ms": time_ms,
                "personal_bests": pbs,
                "wallet_balance_paise": balance,
            }))
        }
        Ok(None) => Json(json!({ "found": false, "message": "No customer found for this phone" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Bot: register-lead ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BotRegisterLeadRequest {
    phone: String,
    name: Option<String>,
    source: Option<String>,
    intent: Option<String>,
    notes: Option<String>,
}

pub(crate) async fn bot_register_lead(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BotRegisterLeadRequest>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Ensure leads table exists
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            phone TEXT NOT NULL,
            name TEXT,
            source TEXT DEFAULT 'whatsapp',
            intent TEXT DEFAULT 'general',
            stage TEXT DEFAULT 'inquiry',
            notes TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            converted_driver_id TEXT
        )"
    )
    .execute(&state.db)
    .await;

    // Check if lead already exists
    let existing = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM leads WHERE phone = ? LIMIT 1"
    )
    .bind(&req.phone)
    .fetch_optional(&state.db)
    .await;

    match existing {
        Ok(Some((id,))) => {
            // Update existing lead
            let _ = sqlx::query(
                "UPDATE leads SET name = COALESCE(?, name), intent = COALESCE(?, intent),
                 notes = COALESCE(?, notes) WHERE id = ?"
            )
            .bind(&req.name)
            .bind(&req.intent)
            .bind(&req.notes)
            .bind(id)
            .execute(&state.db)
            .await;

            Json(json!({ "status": "updated", "lead_id": id }))
        }
        Ok(None) => {
            let result = sqlx::query(
                "INSERT INTO leads (phone, name, source, intent, notes)
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&req.phone)
            .bind(&req.name)
            .bind(req.source.as_deref().unwrap_or("whatsapp"))
            .bind(req.intent.as_deref().unwrap_or("general"))
            .bind(&req.notes)
            .execute(&state.db)
            .await;

            match result {
                Ok(r) => Json(json!({ "status": "created", "lead_id": r.last_insert_rowid() })),
                Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
            }
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

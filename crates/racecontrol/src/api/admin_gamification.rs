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

pub(crate) async fn staff_gamification_opt_in(
    State(state): State<Arc<AppState>>,
    Path(staff_id): Path<String>,
) -> Json<Value> {
    // Toggle opt-in
    let current: Option<bool> = sqlx::query_scalar(
        "SELECT gamification_opt_in FROM staff_members WHERE id = ?"
    )
    .bind(&staff_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let new_val = match current {
        Some(true) => false,
        _ => true,
    };

    let _ = sqlx::query("UPDATE staff_members SET gamification_opt_in = ? WHERE id = ?")
        .bind(new_val)
        .bind(&staff_id)
        .execute(&state.db)
        .await;

    Json(json!({ "staff_id": staff_id, "gamification_opt_in": new_val }))
}

pub(crate) async fn staff_gamification_leaderboard(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Sessions hosted this month by opted-in staff
    let rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT sm.id, sm.name,
                (SELECT COUNT(*) FROM billing_sessions bs
                 WHERE bs.staff_id = sm.id
                 AND bs.started_at >= datetime('now', 'start of month')) as sessions_hosted
         FROM staff_members sm
         WHERE sm.gamification_opt_in = 1 AND sm.is_active = 1
         ORDER BY sessions_hosted DESC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let leaderboard: Vec<Value> = rows
        .into_iter()
        .enumerate()
        .map(|(i, (id, name, sessions))| {
            json!({
                "rank": i + 1,
                "staff_id": id,
                "name": name,
                "sessions_hosted": sessions,
            })
        })
        .collect();

    Json(json!({ "leaderboard": leaderboard }))
}

pub(crate) async fn staff_badges_list(
    State(state): State<Arc<AppState>>,
    Path(staff_id): Path<String>,
) -> Json<Value> {
    let badges = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(
        "SELECT sb.id, sb.name, sb.description, sb.badge_icon, seb.earned_at
         FROM staff_earned_badges seb
         JOIN staff_badges sb ON sb.id = seb.badge_id
         WHERE seb.staff_id = ?
         ORDER BY seb.earned_at DESC"
    )
    .bind(&staff_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let list: Vec<Value> = badges
        .into_iter()
        .map(|(id, name, desc, icon, earned_at)| {
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "badge_icon": icon,
                "earned_at": earned_at,
            })
        })
        .collect();

    Json(json!({ "badges": list }))
}

pub(crate) async fn staff_kudos_create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let sender_id = body.get("sender_id").and_then(|v| v.as_str()).unwrap_or("");
    let receiver_id = body.get("receiver_id").and_then(|v| v.as_str()).unwrap_or("");
    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let category = body.get("category").and_then(|v| v.as_str()).unwrap_or("teamwork");

    if sender_id.is_empty() || receiver_id.is_empty() || message.is_empty() {
        return Json(json!({ "error": "sender_id, receiver_id, and message are required" }));
    }
    if sender_id == receiver_id {
        return Json(json!({ "error": "Cannot give kudos to yourself" }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO staff_kudos (id, sender_id, receiver_id, message, category) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(sender_id)
    .bind(receiver_id)
    .bind(message)
    .bind(category)
    .execute(&state.db)
    .await
    {
        Ok(_) => Json(json!({ "id": id, "status": "created" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn staff_kudos_list(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT sk.id, sk.sender_id, s1.name, sk.receiver_id, s2.name, sk.message, sk.category
         FROM staff_kudos sk
         JOIN staff_members s1 ON s1.id = sk.sender_id
         JOIN staff_members s2 ON s2.id = sk.receiver_id
         WHERE sk.created_at >= datetime('now', '-30 days')
         ORDER BY sk.created_at DESC
         LIMIT 50"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let kudos: Vec<Value> = rows
        .into_iter()
        .map(|(id, sender_id, sender_name, receiver_id, receiver_name, message, category)| {
            json!({
                "id": id,
                "sender_id": sender_id,
                "sender_name": sender_name,
                "receiver_id": receiver_id,
                "receiver_name": receiver_name,
                "message": message,
                "category": category,
            })
        })
        .collect();

    Json(json!({ "kudos": kudos }))
}

pub(crate) async fn staff_challenges_list(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, i64, Option<String>, String, String, i64, String)>(
        "SELECT id, name, description, goal_type, goal_target, reward_description,
                start_date, end_date, current_progress, status
         FROM staff_challenges
         WHERE status = 'active'
         ORDER BY end_date ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let challenges: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, desc, goal_type, goal_target, reward, start, end, progress, status)| {
            let pct = if goal_target > 0 { (progress * 100 / goal_target).min(100) } else { 0 };
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "goal_type": goal_type,
                "goal_target": goal_target,
                "reward_description": reward,
                "start_date": start,
                "end_date": end,
                "current_progress": progress,
                "progress_percent": pct,
                "status": status,
            })
        })
        .collect();

    Json(json!({ "challenges": challenges }))
}

pub(crate) async fn staff_challenges_create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Challenge");
    let description = body.get("description").and_then(|v| v.as_str());
    let goal_type = body.get("goal_type").and_then(|v| v.as_str()).unwrap_or("sessions_hosted");
    let goal_target = body.get("goal_target").and_then(|v| v.as_i64()).unwrap_or(10);
    let reward = body.get("reward_description").and_then(|v| v.as_str());
    let start_date = body.get("start_date").and_then(|v| v.as_str()).unwrap_or("");
    let end_date = body.get("end_date").and_then(|v| v.as_str()).unwrap_or("");

    if start_date.is_empty() || end_date.is_empty() {
        return Json(json!({ "error": "start_date and end_date are required" }));
    }

    match sqlx::query(
        "INSERT INTO staff_challenges (id, name, description, goal_type, goal_target, reward_description, start_date, end_date)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(name)
    .bind(description)
    .bind(goal_type)
    .bind(goal_target)
    .bind(reward)
    .bind(start_date)
    .bind(end_date)
    .execute(&state.db)
    .await
    {
        Ok(_) => Json(json!({ "id": id, "status": "created" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn staff_challenge_update_progress(
    State(state): State<Arc<AppState>>,
    Path(challenge_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let progress = body.get("current_progress").and_then(|v| v.as_i64()).unwrap_or(0);

    // Check if goal met
    let goal_target: Option<i64> = sqlx::query_scalar(
        "SELECT goal_target FROM staff_challenges WHERE id = ?"
    )
    .bind(&challenge_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let new_status = match goal_target {
        Some(target) if progress >= target => "completed",
        _ => "active",
    };

    let _ = sqlx::query(
        "UPDATE staff_challenges SET current_progress = ?, status = ? WHERE id = ?"
    )
    .bind(progress)
    .bind(new_status)
    .bind(&challenge_id)
    .execute(&state.db)
    .await;

    Json(json!({ "id": challenge_id, "current_progress": progress, "status": new_status }))
}

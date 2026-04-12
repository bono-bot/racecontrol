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

pub(crate) async fn list_badges(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let badges: Vec<(String, String, Option<String>, String, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT id, name, description, category, criteria_json, badge_icon, reward_credits_paise, sort_order
         FROM achievements WHERE is_active = 1 ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let result: Vec<Value> = badges.into_iter().map(|(id, name, desc, cat, criteria, icon, reward, sort)| {
        json!({
            "id": id, "name": name, "description": desc, "category": cat,
            "criteria_json": criteria, "badge_icon": icon,
            "reward_credits_paise": reward, "sort_order": sort
        })
    }).collect();

    Json(json!({ "badges": result }))
}

pub(crate) async fn driver_badges(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let earned: Vec<(String, String, Option<String>, String, Option<String>, String)> = sqlx::query_as(
        "SELECT a.id, a.name, a.description, a.category, a.badge_icon, da.earned_at
         FROM driver_achievements da
         JOIN achievements a ON a.id = da.achievement_id
         WHERE da.driver_id = ?
         ORDER BY da.earned_at DESC"
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let count = earned.len();
    let result: Vec<Value> = earned.into_iter().map(|(id, name, desc, cat, icon, earned_at)| {
        json!({
            "id": id, "name": name, "description": desc,
            "category": cat, "badge_icon": icon, "earned_at": earned_at
        })
    }).collect();

    Json(json!({ "driver_id": driver_id, "badges": result, "count": count }))
}

pub(crate) async fn driver_streak(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let streak: Option<(i64, i64, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at
         FROM streaks WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match streak {
        Some((current, longest, last_visit, grace, started)) => {
            Json(json!({
                "driver_id": driver_id,
                "current_streak": current,
                "longest_streak": longest,
                "last_visit_date": last_visit,
                "grace_expires_date": grace,
                "streak_started_at": started
            }))
        }
        None => {
            Json(json!({
                "driver_id": driver_id,
                "current_streak": 0,
                "longest_streak": 0,
                "last_visit_date": null,
                "grace_expires_date": null,
                "streak_started_at": null
            }))
        }
    }
}

pub(crate) async fn list_nudge_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params.get("limit").and_then(|v| v.parse::<i64>().ok()).unwrap_or(50);
    let status_filter = params.get("status").cloned();

    let nudges: Vec<(String, String, String, i32, String, String, String, Option<String>, Option<String>)> = if let Some(status) = &status_filter {
        sqlx::query_as(
            "SELECT id, driver_id, channel, priority, template, payload_json, status, sent_at, created_at
             FROM nudge_queue WHERE status = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT id, driver_id, channel, priority, template, payload_json, status, sent_at, created_at
             FROM nudge_queue ORDER BY created_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let count = nudges.len();
    let result: Vec<Value> = nudges.into_iter().map(|(id, driver, ch, pri, tpl, payload, status, sent, created)| {
        json!({
            "id": id, "driver_id": driver, "channel": ch, "priority": pri,
            "template": tpl, "payload_json": payload, "status": status,
            "sent_at": sent, "created_at": created
        })
    }).collect();

    Json(json!({ "nudges": result, "count": count }))
}

pub(crate) async fn test_nudge(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let channel = body.get("channel").and_then(|v| v.as_str()).unwrap_or("pwa");
    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("Test notification");

    if driver_id.is_empty() {
        return Json(json!({ "error": "driver_id required" }));
    }

    let ch = psychology::NotificationChannel::from_str(channel)
        .unwrap_or(psychology::NotificationChannel::Pwa);

    psychology::queue_notification(&state, driver_id, ch, 5, message, "{}").await;

    Json(json!({ "ok": true, "queued_for": driver_id, "channel": channel }))
}

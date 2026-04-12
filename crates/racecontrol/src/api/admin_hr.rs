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

pub(crate) async fn ai_behavior_batch_trigger(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let batch_state = state.clone();
    tokio::spawn(async move {
        crate::ai_behavior_batch::run_ai_behavior_batch_cycle(batch_state).await;
    });
    (StatusCode::OK, Json(serde_json::json!({
        "status": "triggered",
        "message": "MMA batch queued"
    })))
}

// ─── HR & Hiring Psychology (v14.0 Phase 96) ─────────────────────────────

pub(crate) async fn list_hiring_sjts(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, scenario_text, options_json, scoring_json
         FROM hiring_sjts WHERE is_active = 1 ORDER BY id"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let sjts: Vec<Value> = rows
        .into_iter()
        .map(|(id, scenario, options, scoring)| {
            json!({
                "id": id,
                "scenario_text": scenario,
                "options": serde_json::from_str::<Value>(&options).unwrap_or(json!([])),
                "scoring": serde_json::from_str::<Value>(&scoring).unwrap_or(json!([])),
            })
        })
        .collect();

    Json(json!({ "sjts": sjts }))
}

pub(crate) async fn get_hiring_sjt(
    State(state): State<Arc<AppState>>,
    Path(sjt_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, scenario_text, options_json, scoring_json
         FROM hiring_sjts WHERE id = ? AND is_active = 1"
    )
    .bind(&sjt_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((id, scenario, options, scoring)) => Json(json!({
            "id": id,
            "scenario_text": scenario,
            "options": serde_json::from_str::<Value>(&options).unwrap_or(json!([])),
            "scoring": serde_json::from_str::<Value>(&scoring).unwrap_or(json!([])),
        })),
        None => Json(json!({ "error": "SJT not found" })),
    }
}

pub(crate) async fn list_job_preview(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
        "SELECT id, title, content, media_url, sort_order
         FROM job_preview ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, title, content, media_url, sort_order)| {
            json!({
                "id": id,
                "title": title,
                "content": content,
                "media_url": media_url,
                "sort_order": sort_order,
            })
        })
        .collect();

    Json(json!({ "items": items }))
}

pub(crate) async fn list_campaign_templates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, name, cialdini_principle, message_template, target_segment, is_active
         FROM campaign_templates ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let templates: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, principle, template, segment, active)| {
            json!({
                "id": id,
                "name": name,
                "cialdini_principle": principle,
                "message_template": template,
                "target_segment": segment,
                "is_active": active,
            })
        })
        .collect();

    Json(json!({ "templates": templates }))
}

pub(crate) async fn list_nudge_templates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool)>(
        "SELECT id, template_type, copy_text, timing_rules_json, is_active
         FROM nudge_templates ORDER BY template_type"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let templates: Vec<Value> = rows
        .into_iter()
        .map(|(id, ttype, copy, timing, active)| {
            json!({
                "id": id,
                "template_type": ttype,
                "copy_text": copy,
                "timing_rules": serde_json::from_str::<Value>(&timing).unwrap_or(json!({})),
                "is_active": active,
            })
        })
        .collect();

    Json(json!({ "templates": templates }))
}

pub(crate) async fn hr_recognition_data(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Combine kudos + badges for recognition page
    let kudos = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT sk.id, s1.name, s2.name, sk.message, sk.category, sk.created_at
         FROM staff_kudos sk
         JOIN staff_members s1 ON s1.id = sk.sender_id
         JOIN staff_members s2 ON s2.id = sk.receiver_id
         ORDER BY sk.created_at DESC LIMIT 20"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let kudos_list: Vec<Value> = kudos
        .into_iter()
        .map(|(id, sender, receiver, msg, cat, created)| {
            json!({
                "id": id, "sender_name": sender, "receiver_name": receiver,
                "message": msg, "category": cat, "created_at": created,
            })
        })
        .collect();

    // Top badge earners
    let badge_leaders = sqlx::query_as::<_, (String, i64)>(
        "SELECT sm.name, COUNT(*) as badge_count
         FROM staff_earned_badges seb
         JOIN staff_members sm ON sm.id = seb.staff_id
         GROUP BY seb.staff_id
         ORDER BY badge_count DESC LIMIT 10"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let leaders: Vec<Value> = badge_leaders
        .into_iter()
        .map(|(name, count)| json!({ "name": name, "badge_count": count }))
        .collect();

    Json(json!({
        "recent_kudos": kudos_list,
        "badge_leaders": leaders,
    }))
}

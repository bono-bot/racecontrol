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

// ─── AI Chat ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct AiChatRequest {
    message: String,
    #[serde(default)]
    history: Vec<Value>,
}

/// Staff/admin AI chat — full business context.
pub(crate) async fn ai_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AiChatRequest>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.chat_enabled {
        return Json(json!({ "error": "AI chat is not enabled" }));
    }

    // Gather live business context
    let context = crate::ai::gather_business_context(
        &state.db,
        &state.pods,
        &state.billing,
        &state.game_launcher,
    )
    .await;

    let system_prompt = crate::ai::build_staff_prompt(&context);

    // Build messages array: system + history + new message
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in &req.history {
        messages.push(msg.clone());
    }

    messages.push(json!({
        "role": "user",
        "content": req.message,
    }));

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("staff_chat")).await {
        Ok((reply, model)) => Json(json!({
            "reply": reply,
            "model": model,
        })),
        Err(e) => Json(json!({
            "error": format!("AI query failed: {}", e),
        })),
    }
}

/// Customer AI chat — scoped to their own data only.
pub(crate) async fn customer_ai_chat(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AiChatRequest>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.chat_enabled {
        return Json(json!({ "error": "AI chat is not enabled" }));
    }

    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Gather customer-scoped context
    let context = crate::ai::gather_customer_context(&state.db, &driver_id).await;
    let system_prompt = crate::ai::build_customer_prompt(&context);

    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in &req.history {
        messages.push(msg.clone());
    }

    messages.push(json!({
        "role": "user",
        "content": req.message,
    }));

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("customer_chat")).await {
        Ok((reply, model)) => Json(json!({
            "reply": reply,
            "model": model,
        })),
        Err(e) => Json(json!({
            "error": format!("AI query failed: {}", e),
        })),
    }
}

// ─── AI Diagnose (on-demand) ────────────────────────────────────────────────

/// Staff-triggered on-demand AI analysis of recent operational errors.
pub(crate) async fn ai_diagnose(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled {
        return Json(json!({ "error": "AI debugger is not enabled" }));
    }

    let db = &state.db;
    let mut context_parts: Vec<String> = Vec::new();

    // Recent crashes (last 10 minutes)
    let crashes = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT pod_id, sim_type, error_message, created_at FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-10 minutes') \
         ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !crashes.is_empty() {
        let mut s = format!("RECENT CRASHES ({} in last 10 min):\n", crashes.len());
        for (pod, sim, err, time) in &crashes {
            s.push_str(&format!(
                "  - {} on pod {} at {} ({})\n",
                sim, pod, time,
                err.as_deref().unwrap_or("no details")
            ));
        }
        context_parts.push(s);
    }

    // Billing anomalies
    let stuck = sqlx::query_as::<_, (String, String)>(
        "SELECT pod_id, created_at FROM billing_sessions \
         WHERE status = 'pending' AND created_at < datetime('now', '-60 seconds') \
         AND created_at > datetime('now', '-10 minutes')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !stuck.is_empty() {
        context_parts.push(format!(
            "STUCK BILLING: {} session(s) stuck in 'pending' state",
            stuck.len()
        ));
    }

    let stale = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM billing_sessions \
         WHERE status = 'active' \
         AND datetime(started_at, '+' || allocated_seconds || ' seconds') < datetime('now', '-30 seconds')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    if stale > 0 {
        context_parts.push(format!(
            "STALE BILLING: {} session(s) still 'active' past allocated time",
            stale
        ));
    }

    // API error counts
    let api_errors = state.drain_api_error_counts();
    let high_errors: Vec<_> = api_errors.iter().filter(|(_, v)| **v >= 2).collect();
    if !high_errors.is_empty() {
        let mut s = String::from("API ERRORS (recent):\n");
        for (endpoint, count) in &high_errors {
            s.push_str(&format!("  {} — {} errors\n", endpoint, count));
        }
        context_parts.push(s);
    }

    // Pod connectivity
    let pods = state.pods.read().await;
    let connected = pods.len();
    let expected = state.config.pods.count as usize;
    if connected < expected {
        context_parts.push(format!(
            "POD CONNECTIVITY: {}/{} pods connected",
            connected, expected
        ));
    }
    drop(pods);

    if context_parts.is_empty() {
        return Json(json!({
            "status": "healthy",
            "message": "No operational issues detected in the last 10 minutes"
        }));
    }

    // Gather additional business context
    let biz_context = crate::ai::gather_business_context(
        &state.db,
        &state.pods,
        &state.billing,
        &state.game_launcher,
    )
    .await;

    let full_context = format!(
        "OPERATIONAL ISSUES:\n{}\n\nVENUE STATE:\n{}",
        context_parts.join("\n\n"),
        biz_context
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are James, AI operations assistant for RacingPoint eSports. \
                        Analyze the operational issues below alongside the current venue state. \
                        Provide root cause analysis, severity assessment, and specific actionable steps. \
                        Be concise but thorough."
        }),
        json!({
            "role": "user",
            "content": full_context
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("debug")).await {
        Ok((suggestion, model)) => {
            // Persist to ai_suggestions table
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, 'venue', 'diagnostic', ?, ?, ?, 'diagnose')"
            )
            .bind(&id)
            .bind(&context_parts.join("\n"))
            .bind(&suggestion)
            .bind(&model)
            .execute(db)
            .await;

            Json(json!({
                "status": "analyzed",
                "issues_found": context_parts.len(),
                "suggestion": suggestion,
                "model": model,
                "suggestion_id": id,
            }))
        }
        Err(e) => Json(json!({
            "status": "error",
            "issues_found": context_parts.len(),
            "issues": context_parts,
            "error": format!("AI analysis failed: {}", e),
        })),
    }
}

// ─── AI Suggestions History ─────────────────────────────────────────────────

/// GET /ops/stats — failed sessions today + active/resolved bug counts.
pub(crate) async fn ops_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let failed_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE status IN ('ended_early', 'cancelled') AND date(created_at) = ?",
    )
    .bind(&today)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let active_bugs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_suggestions WHERE dismissed = 0",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let resolved_bugs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_suggestions WHERE dismissed = 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Json(json!({
        "failed_sessions_today": failed_today,
        "active_bugs": active_bugs,
        "resolved_bugs": resolved_bugs,
    }))
}

pub(crate) async fn list_ai_suggestions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200)
        .max(1);

    let pod_filter = params.get("pod_id");

    let rows = if let Some(pod_id) = pod_filter {
        sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i32, String)>(
            "SELECT id, pod_id, sim_type, error_context, suggestion, model, source, dismissed, created_at \
             FROM ai_suggestions WHERE pod_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(pod_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i32, String)>(
            "SELECT id, pod_id, sim_type, error_context, suggestion, model, source, dismissed, created_at \
             FROM ai_suggestions ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(suggestions) => {
            let list: Vec<Value> = suggestions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0,
                        "pod_id": s.1,
                        "sim_type": s.2,
                        "error_context": s.3,
                        "suggestion": s.4,
                        "model": s.5,
                        "source": s.6,
                        "dismissed": s.7 != 0,
                        "created_at": s.8,
                    })
                })
                .collect();
            Json(json!({ "suggestions": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn dismiss_ai_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match sqlx::query("UPDATE ai_suggestions SET dismissed = 1 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "status": "dismissed" })),
        Ok(_) => Json(json!({ "error": "Suggestion not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// AI Training Management (ai_training_stats, ai_training_pairs, ai_training_import)
// moved to ai_training.rs

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

// ─── AI Training Management ─────────────────────────────────────────────────

/// GET /ai/training/stats — training pair counts, avg quality, top keywords.
pub(crate) async fn ai_training_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let db = &state.db;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_training_pairs")
        .fetch_one(db).await.unwrap_or(0);

    let avg_quality: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(quality_score), 0.0) FROM ai_training_pairs"
    ).fetch_one(db).await.unwrap_or(0.0);

    let by_source = sqlx::query_as::<_, (String, i64)>(
        "SELECT source, COUNT(*) as cnt FROM ai_training_pairs GROUP BY source ORDER BY cnt DESC"
    ).fetch_all(db).await.unwrap_or_default();

    let top_used = sqlx::query_as::<_, (String, i64)>(
        "SELECT query_text, use_count FROM ai_training_pairs ORDER BY use_count DESC LIMIT 10"
    ).fetch_all(db).await.unwrap_or_default();

    Json(json!({
        "total": total,
        "avg_quality_score": (avg_quality * 100.0).round() / 100.0,
        "by_source": by_source.iter().map(|(s, c)| json!({"source": s, "count": c})).collect::<Vec<_>>(),
        "top_used": top_used.iter().map(|(q, u)| json!({"query": q, "use_count": u})).collect::<Vec<_>>(),
    }))
}

/// GET /ai/training/pairs — paginated list for review.
pub(crate) async fn ai_training_pairs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
    let source_filter = params.get("source");

    let (pairs, total) = if let Some(src) = source_filter {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs WHERE source = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(src).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs WHERE source = ?"
        ).bind(src).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    } else {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs"
        ).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    };

    Json(json!({
        "total": total,
        "limit": limit,
        "offset": offset,
        "pairs": pairs.iter().map(|(id, q, r, src, model, quality, use_count, created)| json!({
            "id": id,
            "query": q,
            "response": r,
            "source": src,
            "model": model,
            "quality_score": quality,
            "use_count": use_count,
            "created_at": created,
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TrainingImportItem {
    query: String,
    response: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default = "default_quality")]
    quality_score: i64,
}
pub(crate) fn default_source() -> String { "import".to_string() }
pub(crate) fn default_quality() -> i64 { 1 }

/// POST /ai/training/import — bulk import training pairs.
pub(crate) async fn ai_training_import(
    State(state): State<Arc<AppState>>,
    Json(pairs): Json<Vec<TrainingImportItem>>,
) -> Json<Value> {
    let mut inserted = 0u32;
    let mut skipped = 0u32;

    for item in &pairs {
        // Reuse the same log_training_pair logic but with quality_score support
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        item.query.hash(&mut hasher);
        let qhash = format!("{:x}", hasher.finish());

        let keywords = crate::ai::extract_keywords_pub(&item.query);
        let id = uuid::Uuid::new_v4().to_string();

        let result = sqlx::query(
            "INSERT INTO ai_training_pairs \
             (id, query_hash, query_text, query_keywords, response_text, source, model, quality_score) \
             SELECT ?, ?, ?, ?, ?, ?, 'import', ? \
             WHERE NOT EXISTS (SELECT 1 FROM ai_training_pairs WHERE query_hash = ?)",
        )
        .bind(&id)
        .bind(&qhash)
        .bind(&item.query)
        .bind(&keywords)
        .bind(&item.response)
        .bind(&item.source)
        .bind(item.quality_score)
        .bind(&qhash)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => inserted += 1,
            _ => skipped += 1,
        }
    }

    Json(json!({
        "imported": inserted,
        "skipped": skipped,
        "total_submitted": pairs.len(),
    }))
}

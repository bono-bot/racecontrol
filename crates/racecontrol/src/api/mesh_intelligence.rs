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

// ─── Autonomous Pipeline (v26.0) ─────────────────────────────────────────────

pub(crate) async fn pipeline_status() -> Json<serde_json::Value> {
    let config_path = std::path::Path::new("audit/results/auto-detect-config.json");
    let config: serde_json::Value = if config_path.exists() {
        match tokio::fs::read_to_string(config_path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => serde_json::Value::Null,
        }
    } else {
        serde_json::Value::Null
    };

    let suggestions_path = std::path::Path::new("audit/results/suggestions.jsonl");
    let recent_findings: Vec<serde_json::Value> = if suggestions_path.exists() {
        match tokio::fs::read_to_string(suggestions_path).await {
            Ok(content) => content.lines().rev().take(50)
                .filter_map(|line| serde_json::from_str(line).ok()).collect(),
            Err(_) => vec![],
        }
    } else { vec![] };

    let proposals_dir = std::path::Path::new("audit/results/proposals");
    let proposals: Vec<serde_json::Value> = if proposals_dir.exists() {
        match tokio::fs::read_dir(proposals_dir).await {
            Ok(mut entries) => {
                let mut items = vec![];
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.path().extension().map_or(false, |e| e == "json") {
                        if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                items.push(val);
                            }
                        }
                    }
                }
                items
            }
            Err(_) => vec![],
        }
    } else { vec![] };

    let summary_path = std::path::Path::new("audit/results/last-run-summary.json");
    let last_run: serde_json::Value = if summary_path.exists() {
        match tokio::fs::read_to_string(summary_path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => serde_json::Value::Null,
        }
    } else { serde_json::Value::Null };

    Json(serde_json::json!({
        "config": config,
        "last_run": last_run,
        "recent_findings": recent_findings,
        "proposals": proposals,
        "finding_count": recent_findings.len(),
        "proposal_count": proposals.len(),
    }))
}

pub(crate) async fn pipeline_config_get() -> Json<serde_json::Value> {
    let config_path = std::path::Path::new("audit/results/auto-detect-config.json");
    match tokio::fs::read_to_string(config_path).await {
        Ok(content) => Json(serde_json::from_str(&content).unwrap_or_default()),
        Err(_) => Json(serde_json::json!({"error": "config not found"})),
    }
}

pub(crate) async fn pipeline_config_set(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let config_path = std::path::Path::new("audit/results/auto-detect-config.json");
    let mut config: serde_json::Value = match tokio::fs::read_to_string(config_path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => serde_json::json!({}),
    };
    if let (Some(existing), Some(incoming)) = (config.as_object_mut(), body.as_object()) {
        for (key, value) in incoming {
            existing.insert(key.clone(), value.clone());
        }
    }
    match tokio::fs::write(config_path, serde_json::to_string_pretty(&config).unwrap_or_default()).await {
        Ok(_) => Json(serde_json::json!({"ok": true, "config": config})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

// ─── Mesh Intelligence API (v26.0 Phase 222) ────────────────────────────────

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct MeshListParams {
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
    #[serde(default)]
    status: Option<String>,
}

pub(crate) fn default_limit() -> u32 { 50 }

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct MeshSearchParams {
    q: Option<String>,
    limit: Option<u32>,
}

pub(crate) async fn mesh_list_solutions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MeshListParams>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::list_solutions(&state.db, params.status.as_deref(), params.limit, params.offset).await {
        Ok(solutions) => Json(serde_json::json!({ "solutions": solutions, "count": solutions.len() })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn mesh_search_solutions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MeshSearchParams>,
) -> Json<serde_json::Value> {
    let q = params.q.as_deref().unwrap_or("");
    if q.is_empty() {
        return Json(serde_json::json!({ "solutions": [], "count": 0, "query": q }));
    }
    match crate::fleet_kb::search_solutions(&state.db, q, params.limit.unwrap_or(5)).await {
        Ok(solutions) => Json(serde_json::json!({
            "solutions": solutions,
            "count": solutions.len(),
            "query": q,
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn mesh_get_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::get_solution(&state.db, &id).await {
        Ok(Some(sol)) => Json(serde_json::json!(sol)),
        Ok(None) => Json(serde_json::json!({ "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn mesh_list_incidents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MeshListParams>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::list_incidents(&state.db, params.limit, params.offset).await {
        Ok(incidents) => Json(serde_json::json!({ "incidents": incidents, "count": incidents.len() })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn mesh_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let counts = crate::fleet_kb::solution_counts(&state.db).await.unwrap_or_default();
    let total: i64 = counts.iter().map(|(_, c)| c).sum();
    let status_map: std::collections::HashMap<String, i64> = counts.into_iter().collect();
    Json(serde_json::json!({
        "total_solutions": total,
        "by_status": status_map,
    }))
}

/// DEPLOY-AWARE-01: Fleet deployment status for Meshed Intelligence.
/// Returns version consistency, stale builds, crash patterns, and deployment issues.
pub(crate) async fn mesh_deploy_status(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let status = crate::deploy_awareness::get_fleet_deploy_status(&state).await;
    Json(serde_json::to_value(status).unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })))
}

/// Tier 0: Check if a symptom matches a known audit issue.
/// Called by rc-agent before running Tier 1-4 diagnosis.
/// GET /api/v1/mesh/audit-check?symptom=orphan+session+not+ending
pub(crate) async fn mesh_audit_check(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let symptom = params.get("symptom").map(|s| s.as_str()).unwrap_or("");
    if symptom.is_empty() {
        return Json(serde_json::json!({ "matched": false }));
    }
    match crate::fleet_kb::check_audit_known_issues(&state.db, symptom).await {
        Some(escalation) => Json(serde_json::json!({
            "matched": true,
            "escalation": escalation,
            "action": "skip_diagnosis",
        })),
        None => Json(serde_json::json!({ "matched": false })),
    }
}

/// Seed audit findings into the audit_known_issues table.
/// POST /api/v1/mesh/audit-seed with JSON body.
pub(crate) async fn mesh_audit_seed(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let findings = match body.get("findings").and_then(|f| f.as_array()) {
        Some(f) => f,
        None => return Json(serde_json::json!({ "ok": false, "error": "missing findings array" })),
    };
    let mut seeded = 0;
    let mut errors = Vec::new();
    for finding in findings {
        let id = finding.get("problem_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let problem_key = id.clone();
        let severity = finding.get("severity").and_then(|v| v.as_str()).unwrap_or("P2");
        let symptoms: Vec<String> = finding.get("symptom_patterns")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let root_cause = finding.get("root_cause").and_then(|v| v.as_str()).unwrap_or("");
        let fix_desc = finding.get("fix_action").and_then(|v| v.as_str()).unwrap_or("");
        let fix_status = finding.get("fix_status").and_then(|v| v.as_str()).unwrap_or("pending");
        let fixed_in = finding.get("fixed_in_build").and_then(|v| v.as_str());
        let affects: Vec<String> = finding.get("affects")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let audit_date = finding.get("audit_date").and_then(|v| v.as_str()).unwrap_or("2026-04-04");
        let escalation = finding.get("escalation_message").and_then(|v| v.as_str())
            .unwrap_or_else(|| finding.get("symptoms").and_then(|v| v.as_str()).unwrap_or(""));

        if let Err(e) = crate::fleet_kb::upsert_audit_known_issue(
            &state.db, &id, &problem_key, severity, &symptoms, root_cause,
            fix_desc, fix_status, fixed_in, &affects, audit_date, escalation,
        ).await {
            errors.push(format!("{}: {}", problem_key, e));
        } else {
            seeded += 1;
        }
    }
    Json(serde_json::json!({
        "ok": errors.is_empty(),
        "seeded": seeded,
        "errors": errors,
    }))
}

/// POST /api/v1/mesh/audit-seed-service — service-key-authed version of audit-seed.
/// CGP 4.1: Smart pipes and automated tools need to feed MI without staff JWT.
/// Auth: X-Service-Key header must match config sentry_service_key.
pub(crate) async fn mesh_audit_seed_service(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    let expected = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
    let provided = headers.get("X-Service-Key").and_then(|v| v.to_str().ok()).unwrap_or("");
    if expected.is_empty() || provided.is_empty() || provided != expected {
        return (axum::http::StatusCode::UNAUTHORIZED, "Invalid service key").into_response();
    }
    // Delegate to the existing handler logic
    let result = mesh_audit_seed(State(state), Json(body)).await;
    result.into_response()
}

// ─── GLD-C-03: CSV Telemetry Fallback Endpoint ───────────────────────────────

/// POST /api/v1/sessions/{id}/telemetry-fallback
///
/// GLD-C-03: Receive CSV telemetry fallback from rc-agent at session end (D-07/D-09).
/// Staff-authenticated via sentry_service_key (NOT public). Max body 50MB enforced
/// via DefaultBodyLimit layer at the route registration site.
///
/// On success:
/// - Writes CSV to `C:\RacingPoint\telemetry-fallback\{session_id}.csv`
/// - Updates `billing_sessions.csv_fallback_received_at = now()`
/// - Returns 200 "received"
///
/// Auth failures return 401. Path traversal attempts return 400. Missing 'csv'
/// multipart field returns 400. Billing session not found returns 404.
pub(crate) async fn telemetry_fallback_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    // Service key gate (inline, matching existing pattern in mesh_audit_seed_service)
    let expected = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
    let provided = headers
        .get("X-Service-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if expected.is_empty() || provided.is_empty() || provided != expected {
        return (StatusCode::UNAUTHORIZED, "Invalid service key").into_response();
    }

    // Sanitize session_id — reject path traversal attempts
    if session_id.contains("..") || session_id.contains('/') || session_id.contains('\\') {
        return (StatusCode::BAD_REQUEST, "invalid session id").into_response();
    }

    // Extract the 'csv' multipart field
    let mut csv_bytes: Option<Vec<u8>> = None;
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if field.name() == Some("csv") {
                    match field.bytes().await {
                        Ok(b) => { csv_bytes = Some(b.to_vec()); break; }
                        Err(e) => {
                            return (StatusCode::BAD_REQUEST, format!("read error: {e}"))
                                .into_response();
                        }
                    }
                }
                // skip fields that aren't 'csv'
            }
            Ok(None) => break,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("multipart error: {e}")).into_response();
            }
        }
    }
    let Some(csv_data) = csv_bytes else {
        return (StatusCode::BAD_REQUEST, "missing 'csv' field").into_response();
    };
    let csv_len = csv_data.len();

    // Write to C:\RacingPoint\telemetry-fallback\{session_id}.csv
    let dir = std::path::Path::new(r"C:\RacingPoint\telemetry-fallback");
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        tracing::error!(error = %e, "telemetry_fallback: failed to create dir");
        return (StatusCode::INTERNAL_SERVER_ERROR, "mkdir failed").into_response();
    }
    let path = dir.join(format!("{session_id}.csv"));
    if let Err(e) = tokio::fs::write(&path, &csv_data).await {
        tracing::error!(error = %e, path = %path.display(), "telemetry_fallback: write failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    // Update billing_sessions.csv_fallback_received_at
    let now_utc = chrono::Utc::now().to_rfc3339();
    let update_res = sqlx::query(
        "UPDATE billing_sessions SET csv_fallback_received_at = ? WHERE id = ?"
    )
    .bind(&now_utc)
    .bind(&session_id)
    .execute(&state.db)
    .await;

    match update_res {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!(
                session_id = %session_id,
                bytes = csv_len,
                "telemetry_fallback: csv received and stored"
            );
            (StatusCode::OK, "received").into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, "session not found").into_response(),
        Err(e) => {
            tracing::error!(error = %e, "telemetry_fallback: DB update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "db update failed").into_response()
        }
    }
}

pub(crate) async fn mesh_promote_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::update_status(&state.db, &id, rc_common::mesh_types::SolutionStatus::FleetVerified).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "status": "fleet_verified" })),
        Ok(false) => Json(serde_json::json!({ "ok": false, "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

pub(crate) async fn mesh_retire_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::update_status(&state.db, &id, rc_common::mesh_types::SolutionStatus::Retired).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "status": "retired" })),
        Ok(false) => Json(serde_json::json!({ "ok": false, "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub(crate) struct EvalQueryParams {
    model: Option<String>,
    from: Option<String>,
    to: Option<String>,
    #[serde(default = "default_eval_limit")]
    limit: i64,
}

pub(crate) fn default_eval_limit() -> i64 {
    1000
}

/// GET /api/v1/models/evaluations — query evaluation records pushed by rc-agent.
/// Query params: model=<model_id>, from=<ISO8601>, to=<ISO8601>, limit=<n>
/// Returns empty records array (not error) when no matching records exist.
pub(crate) async fn list_model_evaluations(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EvalQueryParams>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::query_eval_records(
        &state.db,
        params.model.as_deref(),
        params.from.as_deref(),
        params.to.as_deref(),
        params.limit,
    )
    .await
    {
        Ok(records) => {
            let count = records.len();
            Json(serde_json::json!({ "records": records, "count": count }))
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "records": [] })),
    }
}

// ─── Model Reputation Query (MREP-04 / Phase 292) ──────────────────────────

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct ReputationQueryParams {
    /// Optional status filter: "active" | "demoted" | "promoted"
    status: Option<String>,
}

/// GET /api/v1/models/reputation — query model reputation state from server DB.
/// Query params: status=<active|demoted|promoted>
/// Returns empty models array (not error) when no reputation data exists yet.
pub(crate) async fn list_model_reputation(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReputationQueryParams>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::query_reputation(&state.db, params.status.as_deref()).await {
        Ok(rows) => {
            let count = rows.len();
            Json(serde_json::json!({ "models": rows, "count": count }))
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "models": [] })),
    }
}

// ─── Cloud Mesh KB Sync (v26.0 Phase 227) ───────────────────────────────────

/// Venue pushes fleet-verified + hardened solutions to cloud KB.
/// Request body: { "venue_id": "rp-hyderabad", "solutions": [...] }
pub(crate) async fn cloud_mesh_sync(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let venue_id = body.get("venue_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let solutions = match body.get("solutions").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Json(serde_json::json!({ "ok": false, "error": "solutions array required" })),
    };

    let mut imported = 0u32;
    let mut errors = 0u32;

    for sol_val in solutions {
        // Parse and tag with venue_id
        let mut sol: rc_common::mesh_types::MeshSolution = match serde_json::from_value(sol_val.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Cloud mesh sync: failed to parse solution: {e}");
                errors += 1;
                continue;
            }
        };
        sol.venue_id = Some(venue_id.to_string());

        if let Err(e) = crate::fleet_kb::insert_solution(&state.db, &sol).await {
            tracing::warn!("Cloud mesh sync: failed to insert solution {}: {e}", sol.id);
            errors += 1;
        } else {
            imported += 1;
        }
    }

    tracing::info!("Cloud mesh sync from venue {venue_id}: imported={imported} errors={errors}");
    Json(serde_json::json!({ "ok": true, "imported": imported, "errors": errors }))
}

/// New venue pulls the full cloud KB to seed their local fleet KB.
/// Query params: ?venue_id=xxx (optional — excludes own solutions to avoid loops)
pub(crate) async fn cloud_mesh_pull(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let requesting_venue = params.get("venue_id").map(|s| s.as_str());

    // Pull all fleet_verified + hardened solutions
    let verified = crate::fleet_kb::list_solutions(&state.db, Some("fleet_verified"), 500, 0).await.unwrap_or_default();
    let hardened = crate::fleet_kb::list_solutions(&state.db, Some("hardened"), 500, 0).await.unwrap_or_default();

    let mut all: Vec<rc_common::mesh_types::MeshSolution> = verified.into_iter().chain(hardened).collect();

    // Exclude requesting venue's own solutions (prevent sync loop)
    if let Some(vid) = requesting_venue {
        all.retain(|s| s.venue_id.as_deref() != Some(vid));
    }

    // Mark external solutions for the requesting venue
    for sol in &mut all {
        if sol.venue_id.is_some() && sol.venue_id.as_deref() != requesting_venue {
            // Tag as external — needs local verification before auto-apply
            if let Some(tags) = sol.tags.as_mut() {
                if !tags.contains(&"external".to_string()) {
                    tags.push("external".to_string());
                }
            } else {
                sol.tags = Some(vec!["external".to_string()]);
            }
        }
    }

    Json(serde_json::json!({ "solutions": all, "count": all.len() }))
}


/// GET /api/v1/reconciliation/status — returns last reconciliation run info.
pub(crate) async fn reconciliation_status() -> Json<serde_json::Value> {
    Json(billing::get_reconciliation_status())
}

/// POST /api/v1/reconciliation/run — triggers an immediate reconciliation run.
pub(crate) async fn reconciliation_run(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    billing::run_reconciliation_public(&state).await;
    Json(billing::get_reconciliation_status())
}

#![allow(unused_imports)]
use axum::{
    Json,
    extract::State,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Deploy Audit Log (Phase 177) ──────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct CreateDeployLog {
    app: String,
    result: String,
    #[serde(default = "default_deployer")]
    deployer: String,
    pages_before: Option<i64>,
    pages_after: Option<i64>,
    pages_missing: Option<String>,
    duration_secs: Option<i64>,
    error: Option<String>,
    build_hash: Option<String>,
}

pub(crate) fn default_deployer() -> String {
    "james".to_string()
}

pub(crate) async fn create_deploy_log(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDeployLog>,
) -> (axum::http::StatusCode, Json<Value>) {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let db = state.db.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO deploy_logs (id, app, timestamp, deployer, result, pages_before, pages_after, pages_missing, duration_secs, error, build_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_clone)
        .bind(&body.app)
        .bind(&timestamp)
        .bind(&body.deployer)
        .bind(&body.result)
        .bind(body.pages_before)
        .bind(body.pages_after)
        .bind(&body.pages_missing)
        .bind(body.duration_secs)
        .bind(&body.error)
        .bind(&body.build_hash)
        .execute(&db)
        .await;
    });

    (
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": id, "status": "logged" })),
    )
}

pub(crate) async fn list_deploy_logs(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, DeployLogRow>(
        "SELECT id, app, timestamp, deployer, result, pages_before, pages_after, pages_missing, duration_secs, error, build_hash FROM deploy_logs ORDER BY timestamp DESC LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(logs) => {
            let entries: Vec<Value> = logs
                .into_iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "app": r.app,
                        "timestamp": r.timestamp,
                        "deployer": r.deployer,
                        "result": r.result,
                        "pages_before": r.pages_before,
                        "pages_after": r.pages_after,
                        "pages_missing": r.pages_missing,
                        "duration_secs": r.duration_secs,
                        "error": r.error,
                        "build_hash": r.build_hash,
                    })
                })
                .collect();
            Json(json!(entries))
        }
        Err(e) => {
            tracing::error!("Failed to fetch deploy_logs: {e}");
            Json(json!([]))
        }
    }
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
pub(crate) struct DeployLogRow {
    id: String,
    app: String,
    timestamp: String,
    deployer: String,
    result: String,
    pages_before: Option<i64>,
    pages_after: Option<i64>,
    pages_missing: Option<String>,
    duration_secs: Option<i64>,
    error: Option<String>,
    build_hash: Option<String>,
}

/// GET /api/v1/app-health — current health probe results for admin, kiosk, web.
/// v38.0: Now includes semantic_status, deep_check_passed, and server_alerts summary.
pub(crate) async fn get_app_health() -> Json<Value> {
    let entries = crate::app_health_monitor::get_current_health().await;

    let alerts: Vec<Value> = entries
        .iter()
        .filter(|e| e.status != "ok")
        .map(|e| {
            json!({
                "app": e.app,
                "status": e.status,
                "message": e.error.as_deref().unwrap_or("unhealthy"),
                "severity": if e.status == "unreachable" { "critical" } else { "warning" },
                "timestamp": e.last_checked,
            })
        })
        .collect();

    let result: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            json!({
                "app": e.app,
                "status": e.status,
                "pages_expected": e.pages_expected,
                "pages_available": e.pages_available,
                "last_checked": e.last_checked,
                "response_ms": e.response_ms,
                "error": e.error,
                "semantic_status": e.semantic_status,
                "deep_check_passed": e.deep_check_passed,
            })
        })
        .collect();

    Json(json!({
        "apps": result,
        "server_alerts": alerts,
    }))
}

//! Policy engine REST handlers — CRUD for policy rules + eval log query.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::state::AppState;

use super::{
    CreateRuleRequest, EvalLogQuery, PolicyEvalLogEntry, PolicyRule,
    LOG_TARGET, row_to_rule, validate_action, validate_condition,
};

// ─── REST handlers ────────────────────────────────────────────────────────────

/// GET /api/v1/policy/rules — list all rules ordered by name.
pub async fn list_rules_handler(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<Value>) {
    let rows = sqlx::query_as::<_, (String, String, String, String, f64, String, String, i64, Option<String>, Option<String>, i64)>(
        "SELECT id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count
         FROM policy_rules ORDER BY name ASC"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let rules: Vec<PolicyRule> = rows
                .into_iter()
                .map(|(id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count)| {
                    row_to_rule(id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count)
                })
                .collect();
            (StatusCode::OK, Json(json!({ "rules": rules })))
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "list_rules DB error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })))
        }
    }
}

/// POST /api/v1/policy/rules — create a new rule.
pub async fn create_rule_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRuleRequest>,
) -> (StatusCode, Json<Value>) {
    // Validate
    if body.name.trim().is_empty() || body.name.len() > 100 {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "name must be 1-100 characters" })));
    }
    if body.metric.trim().is_empty() || body.metric.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "metric must be 1-64 characters" })));
    }
    if !validate_condition(&body.condition) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "condition must be gt, lt, or eq" })));
    }
    if !validate_action(&body.action) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "action must be alert, config_change, flag_toggle, or budget_adjust" })));
    }

    let action_params_str = body.action_params.to_string();
    let enabled_int = if body.enabled { 1i64 } else { 0i64 };

    let insert_result = sqlx::query(
        "INSERT INTO policy_rules (name, metric, condition, threshold, action, action_params, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    )
    .bind(&body.name)
    .bind(&body.metric)
    .bind(&body.condition)
    .bind(body.threshold)
    .bind(&body.action)
    .bind(&action_params_str)
    .bind(enabled_int)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_result {
        tracing::error!(target: LOG_TARGET, "create_rule insert error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })));
    }

    // Re-select the inserted row
    let row = sqlx::query_as::<_, (String, String, String, String, f64, String, String, i64, Option<String>, Option<String>, i64)>(
        "SELECT id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count
         FROM policy_rules WHERE rowid = last_insert_rowid()"
    )
    .fetch_one(&state.db)
    .await;

    match row {
        Ok((id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count)) => {
            let rule = row_to_rule(id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count);
            tracing::info!(target: LOG_TARGET, "created policy rule '{}' ({})", rule.name, rule.id);
            (StatusCode::CREATED, Json(json!(rule)))
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "create_rule re-select error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "insert succeeded but re-select failed" })))
        }
    }
}

/// PUT /api/v1/policy/rules/{id} — update an existing rule.
pub async fn update_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<super::UpdateRuleRequest>,
) -> (StatusCode, Json<Value>) {
    // Load existing rule first
    let existing = sqlx::query_as::<_, (String, String, String, String, f64, String, String, i64, Option<String>, Option<String>, i64)>(
        "SELECT id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count
         FROM policy_rules WHERE id = ?1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let existing = match existing {
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "update_rule fetch error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })));
        }
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "rule not found" }))),
        Ok(Some(r)) => row_to_rule(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9, r.10),
    };

    // Apply provided fields
    let new_name = body.name.as_deref().unwrap_or(&existing.name);
    let new_metric = body.metric.as_deref().unwrap_or(&existing.metric);
    let new_condition = body.condition.as_deref().unwrap_or(&existing.condition);
    let new_action = body.action.as_deref().unwrap_or(&existing.action);
    let new_threshold = body.threshold.unwrap_or(existing.threshold);
    let new_enabled = body.enabled.unwrap_or(existing.enabled);
    let new_action_params = body.action_params
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| existing.action_params.clone());

    // Validate updated fields
    if new_name.trim().is_empty() || new_name.len() > 100 {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "name must be 1-100 characters" })));
    }
    if !validate_condition(new_condition) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "condition must be gt, lt, or eq" })));
    }
    if !validate_action(new_action) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "action must be alert, config_change, flag_toggle, or budget_adjust" })));
    }

    let enabled_int = if new_enabled { 1i64 } else { 0i64 };

    let update_result = sqlx::query(
        "UPDATE policy_rules SET name=?1, metric=?2, condition=?3, threshold=?4, action=?5, action_params=?6, enabled=?7
         WHERE id=?8"
    )
    .bind(new_name)
    .bind(new_metric)
    .bind(new_condition)
    .bind(new_threshold)
    .bind(new_action)
    .bind(&new_action_params)
    .bind(enabled_int)
    .bind(&id)
    .execute(&state.db)
    .await;

    match update_result {
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "update_rule error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })));
        }
        Ok(r) if r.rows_affected() == 0 => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "rule not found" })));
        }
        Ok(_) => {}
    }

    // Re-select updated rule
    let row = sqlx::query_as::<_, (String, String, String, String, f64, String, String, i64, Option<String>, Option<String>, i64)>(
        "SELECT id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count
         FROM policy_rules WHERE id = ?1"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await;

    match row {
        Ok((id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count)) => {
            let rule = row_to_rule(id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count);
            (StatusCode::OK, Json(json!(rule)))
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "update_rule re-select error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "update succeeded but re-select failed" })))
        }
    }
}

/// DELETE /api/v1/policy/rules/{id} — delete a rule.
pub async fn delete_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    let result = sqlx::query("DELETE FROM policy_rules WHERE id = ?1")
        .bind(&id)
        .execute(&state.db)
        .await;

    match result {
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "delete_rule error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })))
        }
        Ok(r) if r.rows_affected() == 0 => {
            (StatusCode::NOT_FOUND, Json(json!({ "error": "rule not found" })))
        }
        Ok(_) => {
            tracing::info!(target: LOG_TARGET, "deleted policy rule {}", id);
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
    }
}

/// GET /api/v1/policy/eval-log — list evaluation log entries (last 500, optional rule_id filter).
pub async fn list_eval_log_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EvalLogQuery>,
) -> (StatusCode, Json<Value>) {
    let rows = if let Some(rule_id) = &params.rule_id {
        sqlx::query_as::<_, (i64, String, String, i64, f64, String, String)>(
            "SELECT id, rule_id, rule_name, fired, metric_value, action_taken, evaluated_at
             FROM policy_eval_log WHERE rule_id = ?1 ORDER BY evaluated_at DESC LIMIT 500"
        )
        .bind(rule_id)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (i64, String, String, i64, f64, String, String)>(
            "SELECT id, rule_id, rule_name, fired, metric_value, action_taken, evaluated_at
             FROM policy_eval_log ORDER BY evaluated_at DESC LIMIT 500"
        )
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "list_eval_log error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })))
        }
        Ok(rows) => {
            let entries: Vec<PolicyEvalLogEntry> = rows
                .into_iter()
                .map(|(id, rule_id, rule_name, fired, metric_value, action_taken, evaluated_at)| {
                    PolicyEvalLogEntry {
                        id,
                        rule_id,
                        rule_name,
                        fired: fired == 1,
                        metric_value,
                        action_taken,
                        evaluated_at,
                    }
                })
                .collect();
            (StatusCode::OK, Json(json!({ "entries": entries })))
        }
    }
}

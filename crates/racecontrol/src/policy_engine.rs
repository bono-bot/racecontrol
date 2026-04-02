//! Phase 299: Policy Rules Engine — types, DB helpers, REST handlers.
//!
//! Tables: policy_rules, policy_eval_log
//! Endpoints:
//!   GET    /api/v1/policy/rules         — list all rules
//!   POST   /api/v1/policy/rules         — create rule
//!   PUT    /api/v1/policy/rules/{id}    — update rule
//!   DELETE /api/v1/policy/rules/{id}    — delete rule
//!   GET    /api/v1/policy/eval-log      — list evaluation log (last 500)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use crate::state::AppState;

const LOG_TARGET: &str = "policy_engine";

// ─── Domain types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyCondition { Gt, Lt, Eq }

impl PolicyCondition {
    pub fn check(&self, value: f64, threshold: f64) -> bool {
        match self {
            PolicyCondition::Gt => value > threshold,
            PolicyCondition::Lt => value < threshold,
            PolicyCondition::Eq => (value - threshold).abs() < f64::EPSILON,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyCondition::Gt => "gt",
            PolicyCondition::Lt => "lt",
            PolicyCondition::Eq => "eq",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gt" => Some(Self::Gt),
            "lt" => Some(Self::Lt),
            "eq" => Some(Self::Eq),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction { Alert, ConfigChange, FlagToggle, BudgetAdjust }

impl PolicyAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyAction::Alert => "alert",
            PolicyAction::ConfigChange => "config_change",
            PolicyAction::FlagToggle => "flag_toggle",
            PolicyAction::BudgetAdjust => "budget_adjust",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "alert" => Some(Self::Alert),
            "config_change" => Some(Self::ConfigChange),
            "flag_toggle" => Some(Self::FlagToggle),
            "budget_adjust" => Some(Self::BudgetAdjust),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub condition: String,     // "gt"|"lt"|"eq"
    pub threshold: f64,
    pub action: String,        // "alert"|"config_change"|"flag_toggle"|"budget_adjust"
    pub action_params: String, // JSON text
    pub enabled: bool,
    pub created_at: Option<String>,
    pub last_fired: Option<String>,
    pub eval_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyEvalLogEntry {
    pub id: i64,
    pub rule_id: String,
    pub rule_name: String,
    pub fired: bool,
    pub metric_value: f64,
    pub action_taken: String,
    pub evaluated_at: String,
}

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub metric: String,
    pub condition: String,
    pub threshold: f64,
    pub action: String,
    #[serde(default = "default_action_params")]
    pub action_params: serde_json::Value,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_action_params() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}
fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdateRuleRequest {
    pub name: Option<String>,
    pub metric: Option<String>,
    pub condition: Option<String>,
    pub threshold: Option<f64>,
    pub action: Option<String>,
    pub action_params: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EvalLogQuery {
    pub rule_id: Option<String>,
}

// ─── Validation ───────────────────────────────────────────────────────────────

fn validate_condition(s: &str) -> bool {
    matches!(s, "gt" | "lt" | "eq")
}

fn validate_action(s: &str) -> bool {
    matches!(s, "alert" | "config_change" | "flag_toggle" | "budget_adjust")
}

// ─── DB helpers ───────────────────────────────────────────────────────────────

/// Map a raw sqlx row to PolicyRule.
fn row_to_rule(
    id: String,
    name: String,
    metric: String,
    condition: String,
    threshold: f64,
    action: String,
    action_params: String,
    enabled: i64,
    created_at: Option<String>,
    last_fired: Option<String>,
    eval_count: i64,
) -> PolicyRule {
    PolicyRule {
        id,
        name,
        metric,
        condition,
        threshold,
        action,
        action_params,
        enabled: enabled != 0,
        created_at,
        last_fired,
        eval_count,
    }
}

/// Fetch all enabled rules from the DB (used by evaluation loop).
pub async fn get_active_rules(pool: &sqlx::SqlitePool) -> Result<Vec<PolicyRule>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, f64, String, String, i64, Option<String>, Option<String>, i64)>(
        "SELECT id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count
         FROM policy_rules WHERE enabled = 1 ORDER BY name"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count)| {
            row_to_rule(id, name, metric, condition, threshold, action, action_params, enabled, created_at, last_fired, eval_count)
        })
        .collect())
}

/// Append an evaluation log entry. If fired=true, also updates last_fired + eval_count on the rule.
pub async fn append_eval_log(
    pool: &sqlx::SqlitePool,
    rule_id: &str,
    rule_name: &str,
    fired: bool,
    metric_value: f64,
    action_taken: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO policy_eval_log (rule_id, rule_name, fired, metric_value, action_taken, evaluated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))"
    )
    .bind(rule_id)
    .bind(rule_name)
    .bind(fired as i64)
    .bind(metric_value)
    .bind(action_taken)
    .execute(pool)
    .await?;

    if fired {
        sqlx::query(
            "UPDATE policy_rules SET last_fired = datetime('now'), eval_count = eval_count + 1 WHERE id = ?1"
        )
        .bind(rule_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

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

    match insert_result {
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "create_rule insert error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "database error" })));
        }
        Ok(_) => {}
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
    Json(body): Json<UpdateRuleRequest>,
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

// ─── Background evaluation task ───────────────────────────────────────────────

pub async fn policy_engine_task(state: Arc<AppState>) {
    tracing::info!(target: LOG_TARGET, "policy engine task started");
    let mut last_fired: HashMap<String, std::time::Instant> = HashMap::new();
    let cooldown = std::time::Duration::from_secs(30 * 60);

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Re-load rules each cycle so edits/deletes take effect immediately
        let rules = match get_active_rules(&state.db).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "failed to load policy rules: {}", e);
                continue;
            }
        };

        if rules.is_empty() {
            continue;
        }

        tracing::debug!(target: LOG_TARGET, "evaluating {} active policy rules", rules.len());

        // Build metric snapshot — same as metric_alert_task
        let snapshot = crate::api::metrics_query::query_snapshot(&state.db, None).await;
        let mut latest: HashMap<String, Vec<f64>> = HashMap::new();
        for entry in snapshot {
            latest.entry(entry.name).or_default().push(entry.value);
        }

        for rule in &rules {
            let condition = match PolicyCondition::from_str(&rule.condition) {
                Some(c) => c,
                None => {
                    tracing::warn!(target: LOG_TARGET, "unknown condition '{}' in rule '{}'", rule.condition, rule.name);
                    continue;
                }
            };

            let Some(values) = latest.get(&rule.metric) else {
                // Metric not in snapshot — log eval as not-fired, continue
                let _ = append_eval_log(
                    &state.db, &rule.id, &rule.name, false, 0.0,
                    "metric_not_found"
                ).await;
                continue;
            };

            // Use most significant value for display
            let display_value = match condition {
                PolicyCondition::Gt => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                PolicyCondition::Lt => values.iter().cloned().fold(f64::INFINITY, f64::min),
                PolicyCondition::Eq => values.first().copied().unwrap_or(0.0),
            };

            let triggered = values.iter().any(|&v| condition.check(v, rule.threshold));

            // Log every evaluation (fired or not)
            let action_str = if triggered { rule.action.as_str() } else { "no_action" };
            let _ = append_eval_log(
                &state.db, &rule.id, &rule.name, triggered, display_value, action_str
            ).await;

            if !triggered {
                continue;
            }

            // Cooldown check
            let now = std::time::Instant::now();
            if let Some(&last) = last_fired.get(&rule.id) {
                if now.duration_since(last) < cooldown {
                    tracing::debug!(target: LOG_TARGET, "rule '{}' in cooldown, suppressing", rule.name);
                    continue;
                }
            }
            last_fired.insert(rule.id.clone(), now);

            tracing::warn!(
                target: LOG_TARGET,
                "policy rule '{}' fired: {} {} {} (value={:.2})",
                rule.name, rule.metric, rule.condition, rule.threshold, display_value
            );

            // Dispatch action
            dispatch_action(&state, rule, display_value).await;
        }
    }
}

async fn dispatch_action(state: &Arc<AppState>, rule: &PolicyRule, value: f64) {
    let params: serde_json::Value = serde_json::from_str(&rule.action_params)
        .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));

    match rule.action.as_str() {
        "alert" => {
            // action_params: { "message": "optional override" }
            let msg = params.get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.replace("{value}", &format!("{:.2}", value))
                          .replace("{metric}", &rule.metric))
                .unwrap_or_else(|| format!(
                    "[POLICY] {}: {} {} {:.2} (current={:.2})",
                    rule.name, rule.metric, rule.condition, rule.threshold, value
                ));
            crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
        }

        "flag_toggle" => {
            // action_params: { "flag_name": "some_flag", "enabled": true }
            let flag_name = match params.get("flag_name").and_then(|v| v.as_str()) {
                Some(n) => n.to_string(),
                None => {
                    tracing::warn!(target: LOG_TARGET, "flag_toggle rule '{}' missing flag_name in action_params", rule.name);
                    return;
                }
            };
            let new_enabled = params.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let result = sqlx::query(
                "UPDATE feature_flags SET enabled = ?1, version = version + 1, updated_at = datetime('now') WHERE name = ?2"
            )
            .bind(new_enabled as i64)
            .bind(&flag_name)
            .execute(&state.db)
            .await;
            match result {
                Ok(r) if r.rows_affected() > 0 => {
                    tracing::info!(target: LOG_TARGET, "policy flag_toggle: '{}' set to {}", flag_name, new_enabled);
                }
                Ok(_) => tracing::warn!(target: LOG_TARGET, "policy flag_toggle: flag '{}' not found", flag_name),
                Err(e) => tracing::warn!(target: LOG_TARGET, "policy flag_toggle DB error: {}", e),
            }
        }

        "config_change" => {
            // action_params: { "field": "some_config_field", "value": <json_value>, "target_pods": ["pod_1"] }
            let field = params.get("field").and_then(|v| v.as_str()).unwrap_or("unknown");
            let change_value = params.get("value").cloned().unwrap_or(serde_json::Value::Null);
            let target_pods: Vec<String> = params.get("target_pods")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let pods_str = if target_pods.is_empty() { "all".to_string() } else { target_pods.join(",") };
            tracing::info!(
                target: LOG_TARGET,
                "policy config_change: field='{}' value='{}' pods='{}' (queued for push)",
                field, change_value, pods_str
            );

            // Insert into config_push_queue for each target pod (or "__all__" sentinel)
            let pod_list = if target_pods.is_empty() {
                vec!["__all__".to_string()]
            } else {
                target_pods
            };
            let payload_json = serde_json::to_string(&serde_json::json!({ "fields": { field: change_value } }))
                .unwrap_or_default();
            for pod in &pod_list {
                let _ = sqlx::query(
                    "INSERT INTO config_push_queue (pod_id, payload, seq_num, status) VALUES (?1, ?2, (SELECT COALESCE(MAX(seq_num), 0) + 1 FROM config_push_queue), 'pending')"
                )
                .bind(pod)
                .bind(&payload_json)
                .execute(&state.db)
                .await;
            }
        }

        "budget_adjust" => {
            // action_params: { "daily_budget_usd": 3.0 }
            let new_budget = params.get("daily_budget_usd").and_then(|v| v.as_f64()).unwrap_or(3.0);
            let _ = sqlx::query(
                "INSERT INTO system_settings (key, value) VALUES ('mma.daily_budget_usd', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')"
            )
            .bind(new_budget.to_string())
            .execute(&state.db)
            .await;
            tracing::info!(target: LOG_TARGET, "policy budget_adjust: daily_budget_usd set to {}", new_budget);
        }

        other => {
            tracing::warn!(target: LOG_TARGET, "unknown policy action '{}' in rule '{}'", other, rule.name);
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_condition_gt_check() {
        assert!(PolicyCondition::Gt.check(86.0, 85.0), "86 > 85 should be true");
        assert!(!PolicyCondition::Gt.check(84.0, 85.0), "84 > 85 should be false");
        assert!(!PolicyCondition::Gt.check(85.0, 85.0), "85 > 85 (equal) should be false");
    }

    #[test]
    fn policy_condition_lt_check() {
        assert!(PolicyCondition::Lt.check(10.0, 20.0), "10 < 20 should be true");
        assert!(!PolicyCondition::Lt.check(20.0, 20.0), "20 < 20 (equal) should be false");
        assert!(!PolicyCondition::Lt.check(30.0, 20.0), "30 < 20 should be false");
    }

    #[test]
    fn policy_condition_eq_check() {
        assert!(PolicyCondition::Eq.check(5.0, 5.0), "5.0 == 5.0 should be true");
        assert!(!PolicyCondition::Eq.check(5.1, 5.0), "5.1 != 5.0 should be false");
    }

    #[test]
    fn policy_condition_from_str() {
        assert_eq!(PolicyCondition::from_str("gt"), Some(PolicyCondition::Gt));
        assert_eq!(PolicyCondition::from_str("lt"), Some(PolicyCondition::Lt));
        assert_eq!(PolicyCondition::from_str("eq"), Some(PolicyCondition::Eq));
        assert_eq!(PolicyCondition::from_str("invalid"), None);
        assert_eq!(PolicyCondition::from_str(""), None);
    }

    #[test]
    fn policy_action_from_str() {
        assert_eq!(PolicyAction::from_str("alert"), Some(PolicyAction::Alert));
        assert_eq!(PolicyAction::from_str("config_change"), Some(PolicyAction::ConfigChange));
        assert_eq!(PolicyAction::from_str("flag_toggle"), Some(PolicyAction::FlagToggle));
        assert_eq!(PolicyAction::from_str("budget_adjust"), Some(PolicyAction::BudgetAdjust));
        assert_eq!(PolicyAction::from_str("invalid"), None);
        assert_eq!(PolicyAction::from_str(""), None);
    }

    #[test]
    fn policy_action_as_str_round_trips() {
        assert_eq!(PolicyAction::Alert.as_str(), "alert");
        assert_eq!(PolicyAction::ConfigChange.as_str(), "config_change");
        assert_eq!(PolicyAction::FlagToggle.as_str(), "flag_toggle");
        assert_eq!(PolicyAction::BudgetAdjust.as_str(), "budget_adjust");
    }

    #[test]
    fn policy_condition_as_str_round_trips() {
        assert_eq!(PolicyCondition::Gt.as_str(), "gt");
        assert_eq!(PolicyCondition::Lt.as_str(), "lt");
        assert_eq!(PolicyCondition::Eq.as_str(), "eq");
    }
}

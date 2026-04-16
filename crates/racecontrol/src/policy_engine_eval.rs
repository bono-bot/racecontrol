//! Policy engine background evaluation task — periodic rule evaluation + action dispatch.

use std::collections::HashMap;
use std::sync::Arc;
use crate::state::AppState;

use super::{
    PolicyCondition, PolicyRule, LOG_TARGET,
    append_eval_log, get_active_rules,
};

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
            if let Some(&last) = last_fired.get(&rule.id)
                && now.duration_since(last) < cooldown {
                    tracing::debug!(target: LOG_TARGET, "rule '{}' in cooldown, suppressing", rule.name);
                    continue;
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

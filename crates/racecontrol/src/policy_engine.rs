//! Phase 299: Policy Rules Engine — types, DB helpers, REST handlers.
//!
//! Tables: policy_rules, policy_eval_log
//! Endpoints:
//!   GET    /api/v1/policy/rules         — list all rules
//!   POST   /api/v1/policy/rules         — create rule
//!   PUT    /api/v1/policy/rules/{id}    — update rule
//!   DELETE /api/v1/policy/rules/{id}    — delete rule
//!   GET    /api/v1/policy/eval-log      — list evaluation log (last 500)

use serde::{Deserialize, Serialize};

#[path = "policy_engine_handlers.rs"]
mod handlers;

#[path = "policy_engine_eval.rs"]
mod eval;

pub use handlers::{
    list_rules_handler, create_rule_handler, update_rule_handler,
    delete_rule_handler, list_eval_log_handler,
};
pub use eval::policy_engine_task;

pub(crate) const LOG_TARGET: &str = "policy_engine";

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

pub(crate) fn validate_condition(s: &str) -> bool {
    matches!(s, "gt" | "lt" | "eq")
}

pub(crate) fn validate_action(s: &str) -> bool {
    matches!(s, "alert" | "config_change" | "flag_toggle" | "budget_adjust")
}

// ─── DB helpers ───────────────────────────────────────────────────────────────

/// Map a raw sqlx row to PolicyRule.
pub(crate) fn row_to_rule(
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

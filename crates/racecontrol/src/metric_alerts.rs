//! Metric Alert Task — Phase 289 (ALRT-01..05)
//!
//! Background task that evaluates configured alert rules against the latest
//! metric snapshot every 60 seconds. Fires WhatsApp alerts when thresholds are
//! exceeded, with a 30-minute cooldown per rule to suppress duplicate alerts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::config::AlertCondition;
use crate::event_archive;
use crate::state::AppState;

const LOG_TARGET: &str = "metric_alerts";

pub async fn metric_alert_task(state: Arc<AppState>) {
    tracing::info!(
        target: LOG_TARGET,
        "metric alert task started ({} rules)",
        state.config.alert_rules.len()
    );
    let mut last_fired: HashMap<String, Instant> = HashMap::new();
    let cooldown = Duration::from_secs(30 * 60);
    let mut first_cycle = true;

    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let snapshot = crate::api::metrics_query::query_snapshot(&state.db, None).await;

        if first_cycle {
            tracing::info!(
                target: LOG_TARGET,
                "first evaluation cycle: {} metrics in snapshot",
                snapshot.len()
            );
            first_cycle = false;
        }

        // Build lookup — if same metric for multiple pods, collect all values
        let mut latest: HashMap<String, Vec<f64>> = HashMap::new();
        for entry in snapshot {
            latest.entry(entry.name).or_default().push(entry.value);
        }

        for rule in &state.config.alert_rules {
            let Some(values) = latest.get(&rule.metric) else {
                tracing::debug!(
                    target: LOG_TARGET,
                    "metric '{}' not in snapshot, skipping rule '{}'",
                    rule.metric,
                    rule.name
                );
                continue;
            };

            // Fire if ANY pod's value triggers the condition
            let triggered = values.iter().any(|&value| check_condition(&rule.condition, value, rule.threshold));

            if !triggered {
                continue;
            }

            let now = Instant::now();
            if let Some(&last) = last_fired.get(&rule.name) {
                if now.duration_since(last) < cooldown {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "rule '{}' cooldown active, suppressing alert",
                        rule.name
                    );
                    continue;
                }
            }
            last_fired.insert(rule.name.clone(), now);

            // Use the most significant value for the message
            let display_value = match rule.condition {
                AlertCondition::Gt => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                AlertCondition::Lt => values.iter().cloned().fold(f64::INFINITY, f64::min),
                AlertCondition::Eq => values.first().cloned().unwrap_or(0.0),
            };

            let msg_body = rule
                .message_template
                .replace("{value}", &format!("{:.2}", display_value))
                .replace("{threshold}", &format!("{:.2}", rule.threshold));
            let message = format!(
                "[{}] {}: {}",
                rule.severity.to_uppercase(),
                rule.name,
                msg_body
            );

            tracing::warn!(target: LOG_TARGET, "metric alert fired: {}", message);
            event_archive::append_event(&state.db, "alert.fired", "metric_alerts", None, serde_json::json!({
                "rule_name": rule.name,
                "metric": rule.metric,
                "value": display_value,
                "threshold": rule.threshold,
                "severity": rule.severity,
            }), &state.config.venue.venue_id);
            crate::whatsapp_alerter::send_whatsapp(&state.config, &message).await;
        }
    }
}

/// Check whether a metric value triggers the given condition against the threshold.
fn check_condition(condition: &AlertCondition, value: f64, threshold: f64) -> bool {
    match condition {
        AlertCondition::Gt => value > threshold,
        AlertCondition::Lt => value < threshold,
        AlertCondition::Eq => (value - threshold).abs() < f64::EPSILON,
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlertCondition, MetricAlertRule};

    // ── Helper ──────────────────────────────────────────────────────────────

    fn make_rule(name: &str, condition: AlertCondition, threshold: f64) -> MetricAlertRule {
        MetricAlertRule {
            name: name.to_string(),
            metric: "test_metric".to_string(),
            condition,
            threshold,
            severity: "warn".to_string(),
            message_template: "value={value} threshold={threshold}".to_string(),
        }
    }

    // ── Condition checks ─────────────────────────────────────────────────────

    #[test]
    fn metric_alert_gt_fires_above_threshold() {
        let rule = make_rule("cpu_high", AlertCondition::Gt, 80.0);
        assert!(check_condition(&rule.condition, 90.0, rule.threshold), "should fire when value > threshold");
        assert!(!check_condition(&rule.condition, 80.0, rule.threshold), "should NOT fire when value == threshold");
        assert!(!check_condition(&rule.condition, 70.0, rule.threshold), "should NOT fire when value < threshold");
    }

    #[test]
    fn metric_alert_lt_fires_below_threshold() {
        let rule = make_rule("fps_low", AlertCondition::Lt, 30.0);
        assert!(check_condition(&rule.condition, 20.0, rule.threshold), "should fire when value < threshold");
        assert!(!check_condition(&rule.condition, 30.0, rule.threshold), "should NOT fire when value == threshold");
        assert!(!check_condition(&rule.condition, 40.0, rule.threshold), "should NOT fire when value > threshold");
    }

    #[test]
    fn metric_alert_eq_fires_on_exact_match() {
        let rule = make_rule("pod_count", AlertCondition::Eq, 8.0);
        assert!(check_condition(&rule.condition, 8.0, rule.threshold), "should fire when value == threshold");
        assert!(!check_condition(&rule.condition, 7.0, rule.threshold), "should NOT fire when value != threshold");
        assert!(!check_condition(&rule.condition, 9.0, rule.threshold), "should NOT fire when value != threshold");
    }

    #[test]
    fn metric_alert_eq_uses_epsilon_comparison() {
        // f64::EPSILON is ~2.22e-16; exact equality fires, large delta does not
        assert!(check_condition(&AlertCondition::Eq, 1.0, 1.0), "exact match should fire");
        assert!(!check_condition(&AlertCondition::Eq, 1.001, 1.0), "value outside epsilon should not fire");
    }

    // ── Dedup / cooldown ─────────────────────────────────────────────────────

    #[test]
    fn metric_alert_dedup_suppresses_within_cooldown() {
        let cooldown = Duration::from_secs(30 * 60);
        let mut last_fired: HashMap<String, Instant> = HashMap::new();
        let rule_name = "test_rule";

        // First firing — should NOT be suppressed (no entry yet)
        let now = Instant::now();
        let suppressed_first = last_fired
            .get(rule_name)
            .map(|&last| now.duration_since(last) < cooldown)
            .unwrap_or(false);
        assert!(!suppressed_first, "first alert should not be suppressed");
        last_fired.insert(rule_name.to_string(), now);

        // Second firing immediately after — should be suppressed
        let now2 = Instant::now();
        let suppressed_second = last_fired
            .get(rule_name)
            .map(|&last| now2.duration_since(last) < cooldown)
            .unwrap_or(false);
        assert!(suppressed_second, "second alert within cooldown should be suppressed");
    }

    #[test]
    fn metric_alert_dedup_fires_after_cooldown_expires() {
        let cooldown = Duration::from_millis(1); // Very short for this test
        let mut last_fired: HashMap<String, Instant> = HashMap::new();
        let rule_name = "test_rule";

        // Record a firing that is well in the past (100ms ago, cooldown=1ms)
        let past = Instant::now() - Duration::from_millis(100);
        last_fired.insert(rule_name.to_string(), past);

        // Check: cooldown has expired, should NOT be suppressed
        let now = Instant::now();
        let suppressed = last_fired
            .get(rule_name)
            .map(|&last| now.duration_since(last) < cooldown)
            .unwrap_or(false);
        assert!(!suppressed, "alert should fire again after cooldown expires");
    }

    // ── TOML config deserialization ──────────────────────────────────────────

    #[test]
    fn metric_alert_toml_with_rules_deserializes() {
        let toml = r#"
[venue]
name = "Test Venue"

[server]
host = "127.0.0.1"
port = 8080

[database]
path = "/tmp/test.db"

[[alert_rules]]
name = "cpu_high"
metric = "cpu_usage_pct"
condition = "gt"
threshold = 90.0
message_template = "CPU at {value}% (limit: {threshold}%)"

[[alert_rules]]
name = "fps_low"
metric = "fps"
condition = "lt"
threshold = 30.0
severity = "critical"
message_template = "FPS dropped to {value} (min: {threshold})"
"#;
        let config: crate::config::Config =
            toml::from_str(toml).expect("TOML with alert_rules should parse");
        assert_eq!(config.alert_rules.len(), 2);
        assert_eq!(config.alert_rules[0].name, "cpu_high");
        assert_eq!(config.alert_rules[1].severity, "critical");
    }

    #[test]
    fn metric_alert_toml_without_rules_deserializes() {
        let toml = r#"
[venue]
name = "Test Venue"

[server]
host = "127.0.0.1"
port = 8080

[database]
path = "/tmp/test.db"
"#;
        let config: crate::config::Config =
            toml::from_str(toml).expect("TOML without alert_rules should parse");
        assert!(config.alert_rules.is_empty(), "default should be empty vec");
    }
}

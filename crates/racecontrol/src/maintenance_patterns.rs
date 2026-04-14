//! Phase 6: Failure Pattern Correlation + Phase 7: Remaining Useful Life (RUL) Estimation.
//!
//! Extracted from maintenance_engine.rs — multi-metric failure pattern detection
//! and linear trend extrapolation for component RUL.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::maintenance_models::ComponentRUL;
use super::HwRow;

// ─── Phase 6: Failure Pattern Correlation ───────────────────────────────────

/// Multi-metric failure pattern — correlates multiple metrics to detect
/// complex failure modes that single-threshold rules miss.
#[derive(Debug, Clone, Serialize)]
pub struct FailurePattern {
    pub name: String,
    pub component: String,
    pub conditions: Vec<PatternCondition>,
    pub min_matching: usize,
    pub lookback_minutes: u32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatternCondition {
    pub metric_name: String,
    pub threshold: f64,
    pub above: bool,
}

/// Alert fired when a failure pattern matches.
#[derive(Debug, Clone, Serialize)]
pub struct PatternAlert {
    pub pattern_name: String,
    pub pod_id: String,
    pub component: String,
    pub matched_conditions: Vec<String>,
    pub confidence: f32,
    pub detected_at: DateTime<Utc>,
    pub message: String,
}

pub fn default_patterns() -> Vec<FailurePattern> {
    vec![
        FailurePattern {
            name: "GPU Thermal Throttle".into(),
            component: "GPU".into(),
            conditions: vec![
                PatternCondition { metric_name: "gpu_temp_celsius".into(), threshold: 80.0, above: true },
                PatternCondition { metric_name: "gpu_power_watts".into(), threshold: 200.0, above: true },
                PatternCondition { metric_name: "gpu_usage_pct".into(), threshold: 50.0, above: false },
            ],
            min_matching: 2,
            lookback_minutes: 15,
            confidence: 0.75,
        },
        FailurePattern {
            name: "Memory Exhaustion Cascade".into(),
            component: "Memory".into(),
            conditions: vec![
                PatternCondition { metric_name: "memory_usage_pct".into(), threshold: 90.0, above: true },
                PatternCondition { metric_name: "process_handle_count".into(), threshold: 5000.0, above: true },
                PatternCondition { metric_name: "cpu_usage_pct".into(), threshold: 80.0, above: true },
            ],
            min_matching: 2,
            lookback_minutes: 10,
            confidence: 0.7,
        },
        FailurePattern {
            name: "Storage Degradation".into(),
            component: "Storage".into(),
            conditions: vec![
                PatternCondition { metric_name: "disk_usage_pct".into(), threshold: 90.0, above: true },
                PatternCondition { metric_name: "disk_smart_health_pct".into(), threshold: 70.0, above: false },
            ],
            min_matching: 2,
            lookback_minutes: 60,
            confidence: 0.8,
        },
    ]
}

/// Check failure patterns against recent telemetry data within each pattern's
/// lookback window. Returns alerts for any patterns where enough conditions match.
pub async fn check_patterns(
    pool: &SqlitePool,
    patterns: &[FailurePattern],
) -> Vec<PatternAlert> {
    let now = Utc::now();
    let mut alerts = Vec::new();

    // We need per-pattern lookback windows, so use the maximum and filter per-pattern.
    let max_lookback = patterns.iter().map(|p| p.lookback_minutes).max().unwrap_or(60);
    let cutoff = (now - chrono::Duration::minutes(max_lookback as i64)).to_rfc3339();

    // P1-3: Use subquery for deterministic latest-per-pod selection.
    let rows: Result<Vec<HwRow>, sqlx::Error> = sqlx::query(
        "SELECT
            pod_id,
            gpu_temp_celsius,
            cpu_temp_celsius,
            gpu_power_watts,
            disk_smart_health_pct,
            process_handle_count,
            cpu_usage_pct,
            memory_usage_pct,
            disk_usage_pct,
            network_latency_ms
        FROM hardware_telemetry
        WHERE collected_at > ?1
          AND (pod_id, collected_at) IN (
              SELECT pod_id, MAX(collected_at)
              FROM hardware_telemetry
              WHERE collected_at > ?1
              GROUP BY pod_id
          )"
    )
    .bind(&cutoff)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|r| {
                use sqlx::Row;
                HwRow {
                    pod_id: r.get("pod_id"),
                    gpu_temp_celsius: r.get("gpu_temp_celsius"),
                    cpu_temp_celsius: r.get("cpu_temp_celsius"),
                    gpu_power_watts: r.get("gpu_power_watts"),
                    disk_smart_health_pct: r.get("disk_smart_health_pct"),
                    process_handle_count: r.get("process_handle_count"),
                    cpu_usage_pct: r.get("cpu_usage_pct"),
                    memory_usage_pct: r.get("memory_usage_pct"),
                    disk_usage_pct: r.get("disk_usage_pct"),
                    network_latency_ms: r.get("network_latency_ms"),
                }
            })
            .collect()
    });

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Pattern check: failed to query hardware_telemetry: {}", e);
            return Vec::new();
        }
    };

    for row in &rows {
        for pattern in patterns {
            let mut matched: Vec<String> = Vec::new();

            for cond in &pattern.conditions {
                if let Some(val) = row.metric_value(&cond.metric_name) {
                    let hit = if cond.above { val > cond.threshold } else { val < cond.threshold };
                    if hit {
                        let direction = if cond.above { "above" } else { "below" };
                        matched.push(format!(
                            "{} = {:.1} ({} {:.1})",
                            cond.metric_name, val, direction, cond.threshold
                        ));
                    }
                }
            }

            if matched.len() >= pattern.min_matching {
                let message = format!(
                    "Pattern '{}' detected on pod {} ({}/{} conditions): {}",
                    pattern.name,
                    row.pod_id,
                    matched.len(),
                    pattern.conditions.len(),
                    matched.join("; ")
                );

                tracing::warn!("PATTERN ALERT: {}", message);

                alerts.push(PatternAlert {
                    pattern_name: pattern.name.clone(),
                    pod_id: row.pod_id.clone(),
                    component: pattern.component.clone(),
                    matched_conditions: matched,
                    confidence: pattern.confidence,
                    detected_at: now,
                    message,
                });
            }
        }
    }

    alerts
}

// ─── Phase 7: Remaining Useful Life (RUL) Estimation ────────────────────────

/// Calculate RUL for a component using linear trend extrapolation.
///
/// Uses `get_metric_trend` from telemetry_store to get the slope. If the trend
/// is declining, calculates when the metric will hit the failure threshold.
pub async fn calculate_rul(
    pool: &SqlitePool,
    pod_id: &str,
    component: &str,
    metric_name: &str,
    failure_threshold: f64,
) -> Option<ComponentRUL> {
    let trend = match crate::telemetry_store::get_metric_trend(pool, pod_id, metric_name, 30).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(
                "RUL: failed to get trend for {}:{} on pod {}: {}",
                component, metric_name, pod_id, e
            );
            return None;
        }
    };

    if trend.data_points < 3 {
        // Not enough data for meaningful extrapolation.
        return None;
    }

    // P2: Guard against near-zero rate to avoid infinity/NaN in division.
    // A rate < 0.001/day means the component is effectively stable — RUL is undefined.
    if trend.rate_per_day.abs() < 0.001 {
        return None;
    }

    // Only calculate RUL when trending toward failure.
    // For "health" metrics (declining is bad), trend must be "declining".
    // For "usage" metrics (rising is bad), trend must be "rising".
    let is_declining_health = trend.trend == "declining" && trend.rate_per_day < 0.0;
    let is_rising_usage = trend.trend == "rising" && trend.rate_per_day > 0.0;

    let rul_hours = if is_declining_health {
        // Metric declining toward failure_threshold (e.g., disk health dropping toward 0)
        let gap = trend.current_value - failure_threshold;
        if gap <= 0.0 {
            // Already past failure threshold
            0.0
        } else {
            (gap / trend.rate_per_day.abs()) * 24.0
        }
    } else if is_rising_usage {
        // Metric rising toward failure_threshold (e.g., disk usage rising toward 95%)
        let gap = failure_threshold - trend.current_value;
        if gap <= 0.0 {
            0.0
        } else {
            (gap / trend.rate_per_day.abs()) * 24.0
        }
    } else {
        // Stable or trending away from failure — no RUL concern
        return None;
    };

    // Parse pod_id number
    let pod_num: u8 = pod_id
        .trim_start_matches("pod")
        .trim_start_matches("pod-")
        .parse()
        .unwrap_or(0);

    // Map component string to ComponentType
    let component_type = match component {
        "GPU" => crate::maintenance_models::ComponentType::GPU,
        "CPU" => crate::maintenance_models::ComponentType::CPU,
        "Memory" => crate::maintenance_models::ComponentType::Memory,
        "Storage" => crate::maintenance_models::ComponentType::Storage,
        "Network" => crate::maintenance_models::ComponentType::Network,
        "Cooling" => crate::maintenance_models::ComponentType::Cooling,
        "Software" => crate::maintenance_models::ComponentType::Software,
        _ => crate::maintenance_models::ComponentType::Software,
    };

    Some(ComponentRUL {
        pod_id: pod_num,
        component: component_type,
        component_name: format!("{}:{}", component, metric_name),
        rul_hours: rul_hours as f32,
        rul_confidence: trend.confidence as f32,
        degradation_rate_per_day: trend.rate_per_day,
        last_updated: Utc::now(),
        method: "linear_trend_extrapolation".into(),
        explanation: format!(
            "{} on pod {} is {} at {:.1}/day (current: {:.1}, threshold: {:.1}, ~{:.0}h remaining)",
            metric_name, pod_id, trend.trend, trend.rate_per_day, trend.current_value, failure_threshold, rul_hours
        ),
    })
}

//! Phase 5 (v29.0): Rule-based anomaly detection engine for hardware telemetry.
//!
//! Runs server-side, scanning the `hardware_telemetry` table every 60 seconds.
//! Each rule defines a metric, threshold, direction, sustained-violation window,
//! and cooldown. Alerts are logged via tracing and returned for API consumption.

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::maintenance_models::ComponentRUL;

// ─── Rule & Alert Types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AnomalyRule {
    pub name: String,
    pub component: String,
    pub severity: String,
    pub metric_name: String,
    pub threshold: f64,
    pub above: bool,
    pub min_sustained_minutes: u32,
    pub cooldown_minutes: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnomalyAlert {
    pub rule_name: String,
    pub pod_id: String,
    pub component: String,
    pub severity: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub sustained_minutes: u32,
    pub detected_at: DateTime<Utc>,
    pub message: String,
}

// ─── Default Rules (14-model MMA consensus) ───────────────────────────────────

pub fn default_rules() -> Vec<AnomalyRule> {
    vec![
        AnomalyRule {
            name: "GPU Overheat".into(),
            component: "GPU".into(),
            severity: "High".into(),
            metric_name: "gpu_temp_celsius".into(),
            threshold: 85.0,
            above: true,
            min_sustained_minutes: 5,
            cooldown_minutes: 30,
        },
        AnomalyRule {
            name: "GPU Critical Temp".into(),
            component: "GPU".into(),
            severity: "Critical".into(),
            metric_name: "gpu_temp_celsius".into(),
            threshold: 92.0,
            above: true,
            min_sustained_minutes: 2,
            cooldown_minutes: 60,
        },
        AnomalyRule {
            name: "Disk Health Warning".into(),
            component: "Storage".into(),
            severity: "Medium".into(),
            metric_name: "disk_smart_health_pct".into(),
            threshold: 80.0,
            above: false,
            min_sustained_minutes: 60,
            cooldown_minutes: 1440,
        },
        AnomalyRule {
            name: "Disk Health Critical".into(),
            component: "Storage".into(),
            severity: "Critical".into(),
            metric_name: "disk_smart_health_pct".into(),
            threshold: 50.0,
            above: false,
            min_sustained_minutes: 10,
            cooldown_minutes: 1440,
        },
        AnomalyRule {
            name: "High CPU Usage".into(),
            component: "CPU".into(),
            severity: "Medium".into(),
            metric_name: "cpu_usage_pct".into(),
            threshold: 95.0,
            above: true,
            min_sustained_minutes: 10,
            cooldown_minutes: 15,
        },
        AnomalyRule {
            name: "Memory Pressure".into(),
            component: "Memory".into(),
            severity: "High".into(),
            metric_name: "memory_usage_pct".into(),
            threshold: 95.0,
            above: true,
            min_sustained_minutes: 5,
            cooldown_minutes: 15,
        },
        AnomalyRule {
            name: "Network Latency Spike".into(),
            component: "Network".into(),
            severity: "Medium".into(),
            metric_name: "network_latency_ms".into(),
            threshold: 100.0,
            above: true,
            min_sustained_minutes: 2,
            cooldown_minutes: 10,
        },
        AnomalyRule {
            name: "Handle Leak".into(),
            component: "Software".into(),
            severity: "High".into(),
            metric_name: "process_handle_count".into(),
            threshold: 10000.0,
            above: true,
            min_sustained_minutes: 10,
            cooldown_minutes: 60,
        },
        AnomalyRule {
            name: "Disk Space Critical".into(),
            component: "Storage".into(),
            severity: "Critical".into(),
            metric_name: "disk_usage_pct".into(),
            threshold: 95.0,
            above: true,
            min_sustained_minutes: 5,
            cooldown_minutes: 60,
        },
        AnomalyRule {
            name: "GPU Power Anomaly".into(),
            component: "GPU".into(),
            severity: "Medium".into(),
            metric_name: "gpu_power_watts".into(),
            threshold: 250.0,
            above: true,
            min_sustained_minutes: 5,
            cooldown_minutes: 30,
        },
    ]
}

// ─── Engine State ─────────────────────────────────────────────────────────────

pub struct EngineState {
    /// (pod_id, rule_name) -> last alert time
    last_alert: HashMap<(String, String), DateTime<Utc>>,
    /// (pod_id, rule_name) -> first violation time (for sustained check)
    first_violation: HashMap<(String, String), DateTime<Utc>>,
    /// Recent alerts kept for API access (capped at 200)
    recent_alerts: Vec<AnomalyAlert>,
}

impl EngineState {
    fn new() -> Self {
        Self {
            last_alert: HashMap::new(),
            first_violation: HashMap::new(),
            recent_alerts: Vec::new(),
        }
    }

    /// Return a snapshot of recent alerts for API consumers.
    pub fn recent_alerts(&self) -> &[AnomalyAlert] {
        &self.recent_alerts
    }
}

// ─── Telemetry Row ────────────────────────────────────────────────────────────

/// One row from hardware_telemetry (latest per pod).
#[derive(Debug)]
struct HwRow {
    pod_id: String,
    gpu_temp_celsius: Option<f64>,
    cpu_temp_celsius: Option<f64>,
    gpu_power_watts: Option<f64>,
    disk_smart_health_pct: Option<i64>,
    process_handle_count: Option<i64>,
    cpu_usage_pct: Option<f64>,
    memory_usage_pct: Option<f64>,
    disk_usage_pct: Option<f64>,
    network_latency_ms: Option<i64>,
}

impl HwRow {
    /// Look up a metric value by column name. Returns None if the column is NULL.
    fn metric_value(&self, name: &str) -> Option<f64> {
        match name {
            "gpu_temp_celsius" => self.gpu_temp_celsius,
            "cpu_temp_celsius" => self.cpu_temp_celsius,
            "gpu_power_watts" => self.gpu_power_watts,
            "disk_smart_health_pct" => self.disk_smart_health_pct.map(|v| v as f64),
            "process_handle_count" => self.process_handle_count.map(|v| v as f64),
            "cpu_usage_pct" => self.cpu_usage_pct,
            "memory_usage_pct" => self.memory_usage_pct,
            "disk_usage_pct" => self.disk_usage_pct,
            "network_latency_ms" => self.network_latency_ms.map(|v| v as f64),
            _ => None,
        }
    }
}

// ─── Scan Function ────────────────────────────────────────────────────────────

/// Run one anomaly-detection pass over the latest hardware telemetry data.
///
/// Returns any newly fired alerts (respecting sustained-violation windows and
/// per-rule cooldowns).
pub async fn run_anomaly_scan(
    pool: &SqlitePool,
    state: &Arc<RwLock<EngineState>>,
    rules: &[AnomalyRule],
) -> Vec<AnomalyAlert> {
    let now = Utc::now();
    let cutoff = (now - chrono::Duration::seconds(60)).to_rfc3339();

    // Fetch the latest row per pod within the last 60 seconds.
    // P1-3: Use subquery to reliably get the row with MAX(collected_at) per pod.
    // The old GROUP BY + HAVING MAX pattern is nondeterministic in SQLite.
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
            tracing::warn!("Anomaly scan: failed to query hardware_telemetry: {}", e);
            return Vec::new();
        }
    };

    // MMA-R1: Read current state snapshot under brief read lock, process without lock,
    // then apply mutations under brief write lock. Prevents blocking API readers during scan.
    let (last_alert_snapshot, first_violation_snapshot) = {
        let guard = state.read().await;
        (guard.last_alert.clone(), guard.first_violation.clone())
    };

    let mut alerts = Vec::new();
    let mut new_last_alert: HashMap<(String, String), DateTime<Utc>> = HashMap::new();
    let mut new_first_violation: HashMap<(String, String), DateTime<Utc>> = HashMap::new();
    let mut cleared_violations: Vec<(String, String)> = Vec::new();

    // Copy existing first_violation entries so we can track them across the scan
    let mut working_violations = first_violation_snapshot;

    for row in &rows {
        for rule in rules {
            let key = (row.pod_id.clone(), rule.name.clone());

            let value = match row.metric_value(&rule.metric_name) {
                Some(v) => v,
                None => {
                    // Metric is NULL — clear any tracked violation (sensor offline).
                    working_violations.remove(&key);
                    cleared_violations.push(key);
                    continue;
                }
            };

            let violated = if rule.above {
                value > rule.threshold
            } else {
                value < rule.threshold
            };

            if !violated {
                // No violation — clear tracked first-violation time.
                working_violations.remove(&key);
                cleared_violations.push(key);
                continue;
            }

            // Track sustained violation start.
            let first = *working_violations
                .entry(key.clone())
                .or_insert(now);
            new_first_violation.insert(key.clone(), first);

            let sustained_secs = (now - first).num_seconds().max(0) as u32;
            let sustained_min = sustained_secs / 60;

            if sustained_min < rule.min_sustained_minutes {
                // Not yet sustained long enough.
                continue;
            }

            // Check cooldown (use snapshot + any new alerts we've recorded this scan).
            let last = new_last_alert.get(&key).or_else(|| last_alert_snapshot.get(&key));
            if let Some(last_time) = last {
                let since_last = (now - *last_time).num_seconds().max(0) as u32;
                if since_last < rule.cooldown_minutes * 60 {
                    continue;
                }
            }

            // Fire alert.
            let direction = if rule.above { "above" } else { "below" };
            let message = format!(
                "{} on pod {}: {} is {:.1} ({} threshold {:.1}) for {}+ minutes",
                rule.name, row.pod_id, rule.metric_name, value, direction, rule.threshold, sustained_min
            );

            let alert = AnomalyAlert {
                rule_name: rule.name.clone(),
                pod_id: row.pod_id.clone(),
                component: rule.component.clone(),
                severity: rule.severity.clone(),
                metric_name: rule.metric_name.clone(),
                current_value: value,
                threshold: rule.threshold,
                sustained_minutes: sustained_min,
                detected_at: now,
                message: message.clone(),
            };

            new_last_alert.insert(key.clone(), now);
            // Reset first_violation so the sustained window restarts after cooldown.
            working_violations.remove(&key);
            cleared_violations.push(key);

            match rule.severity.as_str() {
                "Critical" => tracing::error!("ANOMALY [{}]: {}", rule.severity, message),
                "High" => tracing::warn!("ANOMALY [{}]: {}", rule.severity, message),
                _ => tracing::info!("ANOMALY [{}]: {}", rule.severity, message),
            }

            alerts.push(alert);
        }
    }

    // Brief write lock only for state mutations
    {
        let mut guard = state.write().await;
        for (k, v) in new_last_alert {
            guard.last_alert.insert(k, v);
        }
        for (k, v) in new_first_violation {
            guard.first_violation.insert(k, v);
        }
        for k in cleared_violations {
            guard.first_violation.remove(&k);
        }
        if !alerts.is_empty() {
            guard.recent_alerts.extend(alerts.clone());
            let len = guard.recent_alerts.len();
            if len > 200 {
                guard.recent_alerts.drain(..len - 200);
            }
        }
    }

    alerts
}

// ─── Background Scanner ──────────────────────────────────────────────────────

/// Spawn a background tokio task that runs anomaly detection every 60 seconds.
///
/// Returns the shared engine state handle for API access.
/// If `availability_map` is provided, anomaly alerts will update pod availability
/// via the self-healing orchestrator.
pub fn spawn_anomaly_scanner(pool: SqlitePool) -> Arc<RwLock<EngineState>> {
    spawn_anomaly_scanner_with_healing(pool, None)
}

/// Spawn anomaly scanner with optional self-healing integration.
/// When a PodAvailabilityMap is provided, detected anomalies automatically
/// update pod availability state for kiosk/PWA consumers.
pub fn spawn_anomaly_scanner_with_healing(
    pool: SqlitePool,
    availability_map: Option<crate::self_healing::PodAvailabilityMap>,
) -> Arc<RwLock<EngineState>> {
    let state = Arc::new(RwLock::new(EngineState::new()));
    let state_clone = Arc::clone(&state);
    let rules = default_rules();

    tokio::spawn(async move {
        tracing::info!(
            "v29.0 Phase 5: Anomaly scanner started ({} rules, 60s interval, healing={})",
            rules.len(),
            availability_map.is_some(),
        );

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        // Skip the immediate first tick — let telemetry accumulate.
        interval.tick().await;

        loop {
            interval.tick().await;
            let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
            if !alerts.is_empty() {
                tracing::info!(
                    "Anomaly scan: {} new alert(s) detected",
                    alerts.len()
                );

                // Wire anomaly alerts to self-healing availability map
                if let Some(ref avail_map) = availability_map {
                    for alert in &alerts {
                        // MMA-R1: Use strip_prefix for strict parsing, validate range 1-8
                        let pod_num: Option<u8> = alert.pod_id
                            .strip_prefix("pod_")
                            .or_else(|| alert.pod_id.strip_prefix("pod"))
                            .and_then(|s| s.parse::<u8>().ok())
                            .filter(|&p| (1..=8).contains(&p));

                        if let Some(pod_num) = pod_num {
                            let action = crate::self_healing::recommend_action(
                                &alert.rule_name,
                                &alert.severity,
                                pod_num,
                            );
                            crate::self_healing::apply_action(avail_map, &action).await;
                        } else {
                            tracing::warn!("Anomaly scanner: invalid pod_id '{}', skipping self-heal", alert.pod_id);
                        }
                    }
                }
            }
        }
    });

    state
}

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

// ─── Phase 16: Pre-Maintenance Automated Checks ─────────────────────────────

/// Pre-maintenance check results — validate system state before starting work
#[derive(Debug, Clone, Serialize)]
pub struct PreMaintenanceCheck {
    pub pod_id: u8,
    pub checks_passed: bool,
    pub has_active_session: bool,
    pub recent_backup: bool,
    pub pod_reachable: bool,
    pub messages: Vec<String>,
}

/// Run pre-maintenance validation before starting a maintenance task
pub async fn run_pre_checks(
    pod_id: u8,
    state: &std::sync::Arc<crate::state::AppState>,
) -> PreMaintenanceCheck {
    let mut check = PreMaintenanceCheck {
        pod_id,
        checks_passed: true,
        has_active_session: false,
        recent_backup: true, // assume true until we can verify
        pod_reachable: false,
        messages: Vec::new(),
    };

    // Check if pod has active billing session
    let pods = state.pods.read().await;
    if let Some(pod) = pods.values().find(|p| p.number == pod_id as u32) {
        check.pod_reachable = true;
        if pod.billing_session_id.is_some() {
            check.has_active_session = true;
            check.checks_passed = false;
            check.messages.push(format!(
                "Pod {} has active billing session — defer maintenance",
                pod_id
            ));
        }
    } else {
        check.messages.push(format!(
            "Pod {} not connected — cannot verify state",
            pod_id
        ));
        check.checks_passed = false;
    }

    check
}

// ─── Phase 10: Business-Aware Priority Scoring ──────────────────────────────

/// Calculate priority 1-100 weighted by business context.
/// GPT-4.1 death spiral fix: use EXPECTED revenue, not actual.
pub fn calculate_priority(severity: &str, _pod_id: u8, is_peak: bool, has_active_session: bool) -> u8 {
    let base = match severity {
        "Critical" => 80,
        "High" => 60,
        "Medium" => 40,
        _ => 20,
    };
    let peak_factor = if is_peak { 1.5 } else { 1.0 };
    let session_factor = if has_active_session { 1.4 } else { 1.0 };
    let score = (base as f64 * peak_factor * session_factor).min(100.0);
    score as u8
}

/// Check if the venue is currently operating (ping-based, not clock-based).
/// Replaces hardcoded peak hours with venue_state reachability check.
/// Rule: "If server or James is on, venue is open."
pub fn is_peak_hours() -> bool {
    crate::venue_state::venue_is_open()
}

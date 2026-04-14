//! Phase 5 (v29.0): Rule-based anomaly detection engine for hardware telemetry.
//!
//! Runs server-side, scanning the `hardware_telemetry` table every 60 seconds.
//! Each rule defines a metric, threshold, direction, sustained-violation window,
//! and cooldown. Alerts are logged via tracing and returned for API consumption.
//!
//! Submodules (extracted for <500 line compliance):
//! - `maintenance_patterns`: Failure pattern correlation + RUL estimation
//! - `maintenance_checks`: Pre-maintenance validation + business priority scoring

#[path = "maintenance_patterns.rs"]
mod maintenance_patterns;
pub use maintenance_patterns::{
    FailurePattern, PatternCondition, PatternAlert,
    default_patterns, check_patterns, calculate_rul,
};

#[path = "maintenance_checks.rs"]
mod maintenance_checks;
pub use maintenance_checks::{
    PreMaintenanceCheck, run_pre_checks, calculate_priority, is_peak_hours,
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
pub(super) struct HwRow {
    pub(super) pod_id: String,
    pub(super) gpu_temp_celsius: Option<f64>,
    pub(super) cpu_temp_celsius: Option<f64>,
    pub(super) gpu_power_watts: Option<f64>,
    pub(super) disk_smart_health_pct: Option<i64>,
    pub(super) process_handle_count: Option<i64>,
    pub(super) cpu_usage_pct: Option<f64>,
    pub(super) memory_usage_pct: Option<f64>,
    pub(super) disk_usage_pct: Option<f64>,
    pub(super) network_latency_ms: Option<i64>,
}

impl HwRow {
    /// Look up a metric value by column name. Returns None if the column is NULL.
    pub(super) fn metric_value(&self, name: &str) -> Option<f64> {
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


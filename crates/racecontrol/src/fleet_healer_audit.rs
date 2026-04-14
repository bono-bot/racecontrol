//! Fleet Healer — Audit & safety subsystem: repair audit trail, survival report
//! ingestion, and billing safety checks.
//!
//! Contains FH-09 (Repair Audit Trail), FH-10 (Survival Report Ingestion),
//! and FH-11 (Billing Safety Check).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::diagnosis::SshCommandResult;
use super::LOG_TARGET;
use crate::state::AppState;

// ─── FH-09: Repair Audit Trail ─────────────────────────────────────────────

/// Logs every SSH command + response to the `incident_log` table.
pub struct AuditTrail;

impl AuditTrail {
    /// Ensure the incident_log table exists. Called from db::migrate().
    pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS incident_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action_id TEXT NOT NULL,
                pod_id TEXT NOT NULL,
                action_type TEXT NOT NULL,
                command TEXT,
                stdout TEXT,
                stderr TEXT,
                exit_code INTEGER,
                duration_ms INTEGER,
                success INTEGER NOT NULL DEFAULT 0,
                metadata TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_incident_log_action_id ON incident_log(action_id)",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_incident_log_pod_id ON incident_log(pod_id)",
        )
        .execute(pool)
        .await?;

        tracing::info!(target: LOG_TARGET, "incident_log table initialized");
        Ok(())
    }

    /// Log an SSH command execution to the audit trail.
    pub async fn log_ssh_command(
        pool: &SqlitePool,
        action_id: &str,
        result: &SshCommandResult,
        action_type: &str,
    ) {
        let success = result.exit_code == Some(0);
        if let Err(e) = sqlx::query(
            "INSERT INTO incident_log (action_id, pod_id, action_type, command, stdout, stderr, exit_code, duration_ms, success)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(action_id)
        .bind(&result.pod_id)
        .bind(action_type)
        .bind(&result.command)
        .bind(&result.stdout)
        .bind(&result.stderr)
        .bind(result.exit_code)
        .bind(result.duration_ms as i64)
        .bind(success)
        .execute(pool)
        .await
        {
            tracing::error!(
                target: LOG_TARGET,
                action_id = action_id,
                error = %e,
                "Failed to write audit trail entry"
            );
        }
    }

    /// Log a repair result to the audit trail.
    pub async fn log_repair(
        pool: &SqlitePool,
        action_id: &str,
        pod_id: &str,
        action_type: &str,
        success: bool,
        metadata: Option<&str>,
    ) {
        if let Err(e) = sqlx::query(
            "INSERT INTO incident_log (action_id, pod_id, action_type, success, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(action_id)
        .bind(pod_id)
        .bind(action_type)
        .bind(success)
        .bind(metadata)
        .execute(pool)
        .await
        {
            tracing::error!(
                target: LOG_TARGET,
                action_id = action_id,
                error = %e,
                "Failed to write repair audit entry"
            );
        }
    }

    /// Query recent audit entries for a pod.
    pub async fn recent_entries(
        pool: &SqlitePool,
        pod_id: &str,
        limit: u32,
    ) -> Vec<Value> {
        match sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, Option<String>, Option<i32>, Option<i64>, bool, Option<String>, String)>(
            "SELECT id, action_id, pod_id, action_type, command, stdout, stderr, exit_code, duration_ms, success, metadata, created_at
             FROM incident_log WHERE pod_id = ?1 ORDER BY id DESC LIMIT ?2",
        )
        .bind(pod_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|r| {
                    json!({
                        "id": r.0,
                        "action_id": r.1,
                        "pod_id": r.2,
                        "action_type": r.3,
                        "command": r.4,
                        "stdout": r.5,
                        "stderr": r.6,
                        "exit_code": r.7,
                        "duration_ms": r.8,
                        "success": r.9,
                        "metadata": r.10,
                        "created_at": r.11,
                    })
                })
                .collect(),
            Err(e) => {
                tracing::error!(target: LOG_TARGET, error = %e, "Failed to query audit trail");
                Vec::new()
            }
        }
    }
}

// ─── FH-10: Layer 1 Report Ingestion ───────────────────────────────────────

/// A survival report sent by a watchdog or Layer 1 component on a pod.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurvivalReport {
    /// Pod identifier (e.g., "pod_1").
    pub pod_id: String,
    /// Layer that generated the report (e.g., "watchdog", "rc-sentry").
    pub source_layer: String,
    /// Current status of the pod from the reporter's perspective.
    pub status: String,
    /// Timestamp of the report.
    pub timestamp: DateTime<Utc>,
    /// Free-form diagnostics.
    pub diagnostics: Option<Value>,
    /// Uptime in seconds.
    pub uptime_secs: Option<u64>,
    /// Build ID of the running agent binary.
    pub build_id: Option<String>,
}

/// Ingests survival reports from Layer 1 watchdogs.
/// The fleet healer uses these to decide which pods need SSH intervention.
pub struct SurvivalReportIngester {
    /// Most recent report per pod.
    reports: HashMap<String, SurvivalReport>,
}

impl SurvivalReportIngester {
    pub fn new() -> Self {
        Self {
            reports: HashMap::new(),
        }
    }

    /// Ingest a new survival report. Returns true if the report indicates trouble.
    pub fn ingest(&mut self, report: SurvivalReport) -> bool {
        let is_troubled = report.status != "healthy" && report.status != "ok";

        if is_troubled {
            tracing::info!(
                target: LOG_TARGET,
                pod_id = %report.pod_id,
                source = %report.source_layer,
                status = %report.status,
                "Survival report indicates trouble"
            );
        }

        self.reports.insert(report.pod_id.clone(), report);
        is_troubled
    }

    /// Get the most recent report for a pod.
    pub fn get_report(&self, pod_id: &str) -> Option<&SurvivalReport> {
        self.reports.get(pod_id)
    }

    /// Get all current reports.
    pub fn all_reports(&self) -> &HashMap<String, SurvivalReport> {
        &self.reports
    }
}

// ─── FH-11: Billing Safety Check ───────────────────────────────────────────

/// Checks billing state before allowing repair actions on a pod.
/// Never restart or repair a pod with an active billing session.
pub struct BillingSafetyCheck;

impl BillingSafetyCheck {
    /// Returns true if the pod has NO active billing session (safe to repair).
    pub async fn is_safe_to_repair(state: &AppState, pod_id: &str) -> bool {
        let timers = state.billing.active_timers.read().await;
        let has_active = timers.contains_key(pod_id);

        if has_active {
            tracing::warn!(
                target: LOG_TARGET,
                pod_id = pod_id,
                "BILLING SAFETY: Pod has an active billing session — repair BLOCKED"
            );
        }

        !has_active
    }

    /// Returns a list of pods that are safe to repair (no active billing).
    pub async fn safe_pods(state: &AppState, pod_ids: &[String]) -> Vec<String> {
        let timers = state.billing.active_timers.read().await;
        pod_ids
            .iter()
            .filter(|pid| !timers.contains_key(pid.as_str()))
            .cloned()
            .collect()
    }
}

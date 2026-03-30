//! Fleet Healer — Layer 2: SSH-based remote pod healing via Tailscale.
//!
//! Phase 270 — v31.0. The server SSHes into dark/broken pods, runs diagnostics,
//! fingerprints symptoms, detects fleet-wide patterns, and applies deterministic
//! fixes autonomously — with billing safety, canary rollout, and full audit trail.
//!
//! Requirements: FH-01 through FH-12.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::state::AppState;

const LOG_TARGET: &str = "fleet-healer";

// ─── Pod Tailscale IP map ───────────────────────────────────────────────────

/// Map pod number (1–8) to its Tailscale IP for SSH access.
fn tailscale_ip(pod_number: u32) -> Option<&'static str> {
    match pod_number {
        1 => Some("100.92.122.89"),
        2 => Some("100.105.93.108"),
        3 => Some("100.69.231.26"),
        4 => Some("100.75.45.10"),
        5 => Some("100.110.133.87"),
        6 => Some("100.127.149.17"),
        7 => Some("100.82.196.28"),
        8 => Some("100.98.67.67"),
        _ => None,
    }
}

/// SSH user for pod connections.
const SSH_USER: &str = "User";
/// SSH connection timeout in seconds.
const SSH_TIMEOUT_SECS: u64 = 10;
/// SSH command execution timeout in seconds.
const SSH_CMD_TIMEOUT_SECS: u64 = 30;

// ─── FH-01: SSH Diagnostic Runner ──────────────────────────────────────────

/// Result of running a single SSH command on a pod.
#[derive(Debug, Clone, Serialize)]
pub struct SshCommandResult {
    pub pod_id: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Runs commands on remote pods via SSH over Tailscale.
pub struct SshDiagnosticRunner;

impl SshDiagnosticRunner {
    /// Run a single command on a pod via SSH.
    ///
    /// Uses `tokio::process::Command` to invoke the ssh binary with
    /// StrictHostKeyChecking=no (Tailscale manages trust).
    ///
    /// SECURITY: Commands are validated against an allowlist of safe characters
    /// to prevent shell injection via KB-sourced fix actions (MMA audit P0 fix).
    pub async fn run_command(pod_number: u32, command: &str) -> Result<SshCommandResult, FleetHealerError> {
        // MMA audit P0 fix: validate command against injection attacks.
        // KB-sourced fix_action could contain shell metacharacters (;, |, &&, $()).
        // Only allow: alphanumeric, spaces, slashes, dots, hyphens, underscores, colons, equals.
        if !command.chars().all(|c| c.is_alphanumeric() || " /\\.-_:=,\"'".contains(c)) {
            tracing::warn!(
                target: LOG_TARGET,
                pod = pod_number,
                command = %command,
                "SSH command BLOCKED — contains unsafe characters (potential injection)"
            );
            return Err(FleetHealerError::CommandBlocked {
                pod_id: format!("pod_{}", pod_number),
                reason: "Command contains unsafe characters".to_string(),
            });
        }

        let ip = tailscale_ip(pod_number)
            .ok_or(FleetHealerError::UnknownPod(pod_number))?;

        let pod_id = format!("pod_{}", pod_number);
        let start = Instant::now();

        let result = tokio::time::timeout(
            Duration::from_secs(SSH_CMD_TIMEOUT_SECS),
            tokio::process::Command::new("ssh")
                .arg("-o").arg("StrictHostKeyChecking=no")
                .arg("-o").arg("BatchMode=yes")
                .arg("-o").arg(format!("ConnectTimeout={}", SSH_TIMEOUT_SECS))
                .arg(format!("{}@{}", SSH_USER, ip))
                .arg(command)
                .output(),
        )
        .await
        .map_err(|_| FleetHealerError::SshTimeout {
            pod_id: pod_id.clone(),
            timeout_secs: SSH_CMD_TIMEOUT_SECS,
        })?
        .map_err(|e| FleetHealerError::SshExecFailed {
            pod_id: pod_id.clone(),
            error: e.to_string(),
        })?;

        let elapsed = start.elapsed();

        Ok(SshCommandResult {
            pod_id,
            command: command.to_string(),
            exit_code: result.status.code(),
            stdout: String::from_utf8_lossy(&result.stdout).to_string(),
            stderr: String::from_utf8_lossy(&result.stderr).to_string(),
            duration_ms: elapsed.as_millis() as u64,
            timestamp: Utc::now(),
        })
    }

    /// Run a suite of diagnostic commands on a pod and return structured results.
    pub async fn run_diagnostics(pod_number: u32) -> Result<Vec<SshCommandResult>, FleetHealerError> {
        let commands = vec![
            "tasklist /FO CSV /NH",
            "netstat -an | findstr LISTEN",
            "wevtutil qe Application /c:20 /f:text /rd:true",
            r#"powershell -NoProfile -Command "Get-Process rc-agent -ErrorAction SilentlyContinue | Select-Object Id,SessionId,CPU,WorkingSet64 | ConvertTo-Json""#,
            r#"if exist C:\RacingPoint\MAINTENANCE_MODE (echo MAINTENANCE_MODE_PRESENT) else (echo MAINTENANCE_MODE_ABSENT)"#,
        ];

        let mut results = Vec::with_capacity(commands.len());
        for cmd in commands {
            match Self::run_command(pod_number, cmd).await {
                Ok(r) => results.push(r),
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        pod = pod_number,
                        command = cmd,
                        error = %e,
                        "SSH diagnostic command failed"
                    );
                    // Continue with remaining commands even if one fails.
                }
            }
        }

        Ok(results)
    }
}

// ─── FH-02: Diagnostic Fingerprinting ──────────────────────────────────────

/// A structured symptom derived from diagnostic command output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Symptom {
    /// Category of the symptom (e.g., "process_missing", "port_not_listening").
    pub category: String,
    /// Specific detail (e.g., "rc-agent.exe", "8090").
    pub detail: String,
    /// Severity: "critical", "high", "medium", "low".
    pub severity: String,
}

/// Maps raw diagnostic output to structured symptoms.
pub struct DiagnosticFingerprinter;

impl DiagnosticFingerprinter {
    /// Fingerprint a set of diagnostic results into a list of symptoms.
    pub fn fingerprint(results: &[SshCommandResult]) -> Vec<Symptom> {
        let mut symptoms = Vec::new();

        for result in results {
            // Fingerprint tasklist output
            if result.command.starts_with("tasklist") {
                Self::fingerprint_tasklist(&result.stdout, &mut symptoms);
            }

            // Fingerprint netstat output
            if result.command.starts_with("netstat") {
                Self::fingerprint_netstat(&result.stdout, &mut symptoms);
            }

            // Fingerprint Windows Event Log
            if result.command.starts_with("wevtutil") {
                Self::fingerprint_event_log(&result.stdout, &mut symptoms);
            }

            // Fingerprint MAINTENANCE_MODE check
            if result.stdout.contains("MAINTENANCE_MODE_PRESENT") {
                symptoms.push(Symptom {
                    category: "sentinel".to_string(),
                    detail: "MAINTENANCE_MODE active".to_string(),
                    severity: "high".to_string(),
                });
            }

            // Fingerprint rc-agent process info
            if result.command.contains("Get-Process rc-agent") {
                Self::fingerprint_rcagent_process(&result.stdout, &mut symptoms);
            }
        }

        symptoms
    }

    fn fingerprint_tasklist(stdout: &str, symptoms: &mut Vec<Symptom>) {
        let critical_processes = ["rc-agent.exe", "msedge.exe"];
        let expected_processes = ["conspitlink2.0.exe"];

        for proc in &critical_processes {
            if !stdout.to_lowercase().contains(&proc.to_lowercase()) {
                symptoms.push(Symptom {
                    category: "process_missing".to_string(),
                    detail: proc.to_string(),
                    severity: "critical".to_string(),
                });
            }
        }

        for proc in &expected_processes {
            if !stdout.to_lowercase().contains(&proc.to_lowercase()) {
                symptoms.push(Symptom {
                    category: "process_missing".to_string(),
                    detail: proc.to_string(),
                    severity: "medium".to_string(),
                });
            }
        }
    }

    fn fingerprint_netstat(stdout: &str, symptoms: &mut Vec<Symptom>) {
        let expected_ports = [("8090", "rc-agent"), ("18923", "lock_screen")];

        for (port, service) in &expected_ports {
            let pattern = format!(":{}", port);
            if !stdout.contains(&pattern) {
                symptoms.push(Symptom {
                    category: "port_not_listening".to_string(),
                    detail: format!("{} (port {})", service, port),
                    severity: "high".to_string(),
                });
            }
        }
    }

    fn fingerprint_event_log(stdout: &str, symptoms: &mut Vec<Symptom>) {
        let error_patterns = [
            ("Application Error", "app_crash"),
            ("Faulting application", "app_crash"),
            (".NET Runtime", "dotnet_error"),
            ("0xc0000005", "access_violation"),
        ];

        for (pattern, category) in &error_patterns {
            if stdout.contains(pattern) {
                symptoms.push(Symptom {
                    category: category.to_string(),
                    detail: format!("Event log contains: {}", pattern),
                    severity: "high".to_string(),
                });
            }
        }
    }

    fn fingerprint_rcagent_process(stdout: &str, symptoms: &mut Vec<Symptom>) {
        // If empty or error, rc-agent is not running (already caught by tasklist)
        if stdout.trim().is_empty() {
            return;
        }

        // Try to parse JSON to check session ID
        if let Ok(info) = serde_json::from_str::<Value>(stdout) {
            // Check if running in Session 0 (services) — should be Session 1 (console)
            if let Some(session_id) = info.get("SessionId").and_then(|v| v.as_i64()) {
                if session_id == 0 {
                    symptoms.push(Symptom {
                        category: "wrong_session".to_string(),
                        detail: "rc-agent running in Session 0 (should be Session 1)".to_string(),
                        severity: "critical".to_string(),
                    });
                }
            }
        }
    }
}

// ─── FH-03: Fleet Pattern Detection ────────────────────────────────────────

/// Tracks failures per pod with timestamps for fleet-wide pattern detection.
/// Same failure on 3+ pods within 5 minutes triggers a single coordinated
/// response instead of 8 parallel sessions.
pub struct FleetPatternDetector {
    /// Map of symptom_key -> Vec<(pod_id, timestamp)>
    recent_failures: HashMap<String, Vec<(String, Instant)>>,
    /// Sliding window for pattern detection.
    window: Duration,
    /// Minimum pod count to declare a fleet-wide pattern.
    min_pods: usize,
}

/// A detected fleet-wide pattern.
#[derive(Debug, Clone, Serialize)]
pub struct FleetPattern {
    /// The symptom that triggered the pattern.
    pub symptom_key: String,
    /// Pods affected.
    pub affected_pods: Vec<String>,
    /// When the pattern was detected.
    pub detected_at: DateTime<Utc>,
}

impl FleetPatternDetector {
    pub fn new() -> Self {
        Self {
            recent_failures: HashMap::new(),
            window: Duration::from_secs(300), // 5 minutes
            min_pods: 3,
        }
    }

    /// Record a failure for a pod. Returns `Some(FleetPattern)` if a fleet-wide
    /// pattern is now detected (3+ pods with the same symptom within 5 min).
    pub fn record_failure(&mut self, pod_id: &str, symptom: &Symptom) -> Option<FleetPattern> {
        let key = format!("{}:{}", symptom.category, symptom.detail);
        let now = Instant::now();

        let entries = self.recent_failures.entry(key.clone()).or_default();

        // Evict old entries outside the window
        entries.retain(|(_, ts)| now.duration_since(*ts) < self.window);

        // Don't add duplicate pod entries within the same window
        if !entries.iter().any(|(pid, _)| pid == pod_id) {
            entries.push((pod_id.to_string(), now));
        }

        // Check for fleet-wide pattern
        if entries.len() >= self.min_pods {
            let affected: Vec<String> = entries.iter().map(|(pid, _)| pid.clone()).collect();
            tracing::warn!(
                target: LOG_TARGET,
                symptom_key = %key,
                affected_count = affected.len(),
                "Fleet-wide pattern detected: {} pods have the same failure",
                affected.len()
            );
            Some(FleetPattern {
                symptom_key: key,
                affected_pods: affected,
                detected_at: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Clear all recorded failures (e.g., after handling a fleet pattern).
    pub fn clear_pattern(&mut self, symptom_key: &str) {
        self.recent_failures.remove(symptom_key);
    }
}

// ─── FH-04 / FH-05: Repair Confidence Gate & Dispatch ──────────────────────

/// Confidence threshold for autonomous fix dispatch.
const CONFIDENCE_GATE: f64 = 0.8;

/// A repair action to be dispatched to a pod via SSH.
#[derive(Debug, Clone, Serialize)]
pub struct RepairAction {
    /// Unique ID for audit trail.
    pub action_id: String,
    /// Pod target.
    pub pod_id: String,
    /// SSH command to execute.
    pub ssh_command: String,
    /// Description for humans and audit.
    pub description: String,
    /// Fix type from fleet KB.
    pub fix_type: String,
    /// Confidence score from fleet KB.
    pub confidence: f64,
}

/// Result of a repair attempt.
#[derive(Debug, Clone, Serialize)]
pub struct RepairResult {
    pub action_id: String,
    pub pod_id: String,
    pub success: bool,
    pub ssh_result: Option<SshCommandResult>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Dispatches repairs to pods via SSH, gated by confidence and fix_type.
pub struct RepairDispatcher;

impl RepairDispatcher {
    /// Check whether a repair should be autonomously dispatched.
    /// FH-04: Only dispatch if confidence >= 0.8 AND fix_type is Deterministic or Config.
    pub fn should_dispatch(confidence: f64, fix_type: &str) -> bool {
        if confidence < CONFIDENCE_GATE {
            tracing::info!(
                target: LOG_TARGET,
                confidence = confidence,
                fix_type = fix_type,
                "Repair blocked: confidence {:.2} < gate {:.2}",
                confidence,
                CONFIDENCE_GATE
            );
            return false;
        }

        match fix_type {
            "Deterministic" | "Config" => true,
            _ => {
                tracing::info!(
                    target: LOG_TARGET,
                    fix_type = fix_type,
                    "Repair blocked: fix_type '{}' not eligible for autonomous dispatch",
                    fix_type
                );
                false
            }
        }
    }

    /// Dispatch a repair action to a pod via SSH.
    /// FH-05: Apply deterministic fixes from fleet KB remotely.
    pub async fn dispatch(
        pod_number: u32,
        action: &RepairAction,
    ) -> RepairResult {
        let action_id = action.action_id.clone();
        let pod_id = action.pod_id.clone();

        tracing::info!(
            target: LOG_TARGET,
            action_id = %action_id,
            pod = pod_number,
            description = %action.description,
            "Dispatching repair via SSH"
        );

        match SshDiagnosticRunner::run_command(pod_number, &action.ssh_command).await {
            Ok(result) => {
                let success = result.exit_code == Some(0);
                if success {
                    tracing::info!(
                        target: LOG_TARGET,
                        action_id = %action_id,
                        pod = pod_number,
                        "Repair executed successfully"
                    );
                } else {
                    tracing::warn!(
                        target: LOG_TARGET,
                        action_id = %action_id,
                        pod = pod_number,
                        exit_code = ?result.exit_code,
                        stderr = %result.stderr,
                        "Repair command returned non-zero exit code"
                    );
                }
                RepairResult {
                    action_id,
                    pod_id,
                    success,
                    ssh_result: Some(result),
                    error: None,
                    timestamp: Utc::now(),
                }
            }
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    action_id = %action_id,
                    pod = pod_number,
                    error = %e,
                    "Repair dispatch failed"
                );
                RepairResult {
                    action_id,
                    pod_id,
                    success: false,
                    ssh_result: None,
                    error: Some(e.to_string()),
                    timestamp: Utc::now(),
                }
            }
        }
    }
}

// ─── FH-06: Post-Fix Behavioral Verification ──────────────────────────────

/// Verifies that a fix actually worked by polling the pod's health and debug
/// endpoints for build_id match and edge_process_count > 0.
pub struct PostFixVerifier;

impl PostFixVerifier {
    /// Poll a pod's /health endpoint for the expected build_id, and its /debug
    /// endpoint for edge_process_count > 0.
    ///
    /// Retries up to `max_retries` times with `interval` between each attempt.
    pub async fn verify(
        http_client: &reqwest::Client,
        pod_ip: &str,
        expected_build_id: Option<&str>,
        max_retries: u32,
        interval: Duration,
    ) -> PostFixVerification {
        let health_url = format!("http://{}:8090/health", pod_ip);
        let debug_url = format!("http://{}:18924/debug", pod_ip);

        for attempt in 1..=max_retries {
            tracing::debug!(
                target: LOG_TARGET,
                pod_ip = pod_ip,
                attempt = attempt,
                "Post-fix verification attempt"
            );

            // Check /health for build_id
            let build_id_ok = match http_client
                .get(&health_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(body) = resp.json::<Value>().await {
                        match expected_build_id {
                            Some(expected) => {
                                body.get("build_id")
                                    .and_then(|v| v.as_str())
                                    .map(|id| id == expected)
                                    .unwrap_or(false)
                            }
                            None => true, // No expected build_id — skip check
                        }
                    } else {
                        false
                    }
                }
                Err(_) => false,
            };

            // Check /debug for edge_process_count > 0
            let edge_ok = match http_client
                .get(&debug_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(body) = resp.json::<Value>().await {
                        body.get("edge_process_count")
                            .and_then(|v| v.as_u64())
                            .map(|c| c > 0)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
                Err(_) => false,
            };

            if build_id_ok && edge_ok {
                tracing::info!(
                    target: LOG_TARGET,
                    pod_ip = pod_ip,
                    attempt = attempt,
                    "Post-fix verification PASSED"
                );
                return PostFixVerification {
                    passed: true,
                    build_id_match: build_id_ok,
                    edge_process_ok: edge_ok,
                    attempts: attempt,
                    verified_at: Utc::now(),
                };
            }

            if attempt < max_retries {
                tokio::time::sleep(interval).await;
            }
        }

        tracing::warn!(
            target: LOG_TARGET,
            pod_ip = pod_ip,
            max_retries = max_retries,
            "Post-fix verification FAILED after all retries"
        );
        PostFixVerification {
            passed: false,
            build_id_match: false,
            edge_process_ok: false,
            attempts: max_retries,
            verified_at: Utc::now(),
        }
    }
}

/// Result of post-fix verification.
#[derive(Debug, Clone, Serialize)]
pub struct PostFixVerification {
    pub passed: bool,
    pub build_id_match: bool,
    pub edge_process_ok: bool,
    pub attempts: u32,
    pub verified_at: DateTime<Utc>,
}

// ─── FH-07: Canary Rollout ─────────────────────────────────────────────────

/// Canary rollout strategy: Pod 8 first, verify, then gradual.
///
/// Stages:
/// 1. Pod 8 (canary)
/// 2. Pods 1, 2, 3 (first wave)
/// 3. Pods 4, 5, 6, 7 (remaining)
pub struct CanaryRollout;

impl CanaryRollout {
    /// Return rollout waves in order. Each wave is a Vec of pod numbers.
    /// Excludes pods not in `target_pods`.
    pub fn waves(target_pods: &[u32]) -> Vec<Vec<u32>> {
        let canary: Vec<u32> = vec![8].into_iter().filter(|p| target_pods.contains(p)).collect();
        let wave1: Vec<u32> = vec![1, 2, 3].into_iter().filter(|p| target_pods.contains(p)).collect();
        let wave2: Vec<u32> = vec![4, 5, 6, 7].into_iter().filter(|p| target_pods.contains(p)).collect();

        let mut waves = Vec::new();
        if !canary.is_empty() {
            waves.push(canary);
        }
        if !wave1.is_empty() {
            waves.push(wave1);
        }
        if !wave2.is_empty() {
            waves.push(wave2);
        }
        waves
    }
}

// ─── FH-08: Pod Isolation Before Risky Repair ──────────────────────────────

/// Writes/clears MAINTENANCE_MODE sentinel via SSH before/after risky repairs.
pub struct PodIsolation;

impl PodIsolation {
    /// Write MAINTENANCE_MODE sentinel on a pod before a risky repair.
    pub async fn isolate(pod_number: u32) -> Result<(), FleetHealerError> {
        tracing::info!(
            target: LOG_TARGET,
            pod = pod_number,
            "Isolating pod: writing MAINTENANCE_MODE sentinel"
        );
        let result = SshDiagnosticRunner::run_command(
            pod_number,
            r#"echo fleet_healer_isolation > C:\RacingPoint\MAINTENANCE_MODE"#,
        )
        .await?;

        if result.exit_code != Some(0) {
            return Err(FleetHealerError::IsolationFailed {
                pod_id: format!("pod_{}", pod_number),
                error: format!("exit_code={:?}, stderr={}", result.exit_code, result.stderr),
            });
        }
        Ok(())
    }

    /// Clear MAINTENANCE_MODE sentinel on a pod after successful repair verification.
    pub async fn clear_isolation(pod_number: u32) -> Result<(), FleetHealerError> {
        tracing::info!(
            target: LOG_TARGET,
            pod = pod_number,
            "Clearing pod isolation: removing MAINTENANCE_MODE sentinel"
        );
        let result = SshDiagnosticRunner::run_command(
            pod_number,
            r#"del C:\RacingPoint\MAINTENANCE_MODE 2>nul & echo ok"#,
        )
        .await?;

        if result.exit_code != Some(0) {
            tracing::warn!(
                target: LOG_TARGET,
                pod = pod_number,
                stderr = %result.stderr,
                "Failed to clear MAINTENANCE_MODE (may not have existed)"
            );
        }
        Ok(())
    }
}

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

// ─── FH-12: Survival Report Endpoint ───────────────────────────────────────

/// Axum handler for POST /api/v1/pods/{id}/survival-report.
/// Watchdog processes on pods POST their survival reports here.
pub async fn survival_report_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(pod_id): axum::extract::Path<String>,
    axum::Json(report): axum::Json<SurvivalReport>,
) -> axum::Json<Value> {
    let normalized = rc_common::pod_id::normalize_pod_id(&pod_id)
        .unwrap_or_else(|_| pod_id.clone());

    tracing::info!(
        target: LOG_TARGET,
        pod_id = %normalized,
        source = %report.source_layer,
        status = %report.status,
        "Received survival report"
    );

    // Log to audit trail
    AuditTrail::log_repair(
        &state.db,
        &format!("sr-{}", Utc::now().timestamp_millis()),
        &normalized,
        "survival_report_ingested",
        report.status == "healthy" || report.status == "ok",
        Some(&serde_json::to_string(&report).unwrap_or_default()),
    )
    .await;

    axum::Json(json!({
        "status": "accepted",
        "pod_id": normalized,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Build the sub-router for fleet healer endpoints.
pub fn fleet_healer_routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new().route(
        "/pods/{pod_id}/survival-report",
        axum::routing::post(survival_report_handler),
    )
}

// ─── Error Type ────────────────────────────────────────────────────────────

/// Errors specific to the fleet healer module.
#[derive(Debug, thiserror::Error)]
pub enum FleetHealerError {
    #[error("Unknown pod number: {0}")]
    UnknownPod(u32),

    #[error("SSH timeout on {pod_id} after {timeout_secs}s")]
    SshTimeout { pod_id: String, timeout_secs: u64 },

    #[error("SSH execution failed on {pod_id}: {error}")]
    SshExecFailed { pod_id: String, error: String },

    #[error("Pod isolation failed on {pod_id}: {error}")]
    IsolationFailed { pod_id: String, error: String },

    #[error("Billing session active on {pod_id} — repair not permitted")]
    BillingActive { pod_id: String },

    #[error("SSH command blocked on {pod_id}: {reason}")]
    CommandBlocked { pod_id: String, reason: String },
}

// ─── Orchestrator (ties FH-01 through FH-12 together) ──────────────────────

/// High-level orchestrator that coordinates all fleet healer subsystems.
/// Typically invoked from a background task or API trigger.
pub struct FleetHealerOrchestrator;

impl FleetHealerOrchestrator {
    /// Heal a single pod end-to-end:
    /// 1. Check billing safety (FH-11)
    /// 2. Run SSH diagnostics (FH-01)
    /// 3. Fingerprint symptoms (FH-02)
    /// 4. Look up fixes in fleet KB
    /// 5. Gate on confidence (FH-04)
    /// 6. Isolate pod (FH-08) if risky
    /// 7. Dispatch fix (FH-05)
    /// 8. Verify (FH-06)
    /// 9. Clear isolation
    /// 10. Log everything (FH-09)
    pub async fn heal_pod(
        state: &Arc<AppState>,
        pod_number: u32,
    ) -> Result<HealPodOutcome, FleetHealerError> {
        let pod_id = format!("pod_{}", pod_number);
        let action_id = uuid::Uuid::new_v4().to_string();

        tracing::info!(
            target: LOG_TARGET,
            action_id = %action_id,
            pod = pod_number,
            "Starting fleet heal for pod"
        );

        // FH-11: Billing safety check
        if !BillingSafetyCheck::is_safe_to_repair(state, &pod_id).await {
            AuditTrail::log_repair(
                &state.db,
                &action_id,
                &pod_id,
                "heal_blocked_billing",
                false,
                Some("Active billing session on pod"),
            )
            .await;
            return Ok(HealPodOutcome {
                pod_id,
                action_id,
                stage: "billing_check".to_string(),
                success: false,
                blocked_reason: Some("Active billing session".to_string()),
                symptoms: Vec::new(),
                repair_applied: false,
                verification_passed: false,
            });
        }

        // FH-01: SSH Diagnostics
        let diag_results = SshDiagnosticRunner::run_diagnostics(pod_number).await?;

        // FH-09: Log every diagnostic command
        for result in &diag_results {
            AuditTrail::log_ssh_command(&state.db, &action_id, result, "diagnostic").await;
        }

        // FH-02: Fingerprint
        let symptoms = DiagnosticFingerprinter::fingerprint(&diag_results);

        if symptoms.is_empty() {
            tracing::info!(
                target: LOG_TARGET,
                action_id = %action_id,
                pod = pod_number,
                "No symptoms detected — pod appears healthy via SSH"
            );
            AuditTrail::log_repair(
                &state.db,
                &action_id,
                &pod_id,
                "heal_no_symptoms",
                true,
                None,
            )
            .await;
            return Ok(HealPodOutcome {
                pod_id,
                action_id,
                stage: "fingerprint".to_string(),
                success: true,
                blocked_reason: None,
                symptoms: Vec::new(),
                repair_applied: false,
                verification_passed: true,
            });
        }

        tracing::info!(
            target: LOG_TARGET,
            action_id = %action_id,
            pod = pod_number,
            symptom_count = symptoms.len(),
            "Symptoms detected: {:?}",
            symptoms
        );

        // FH-04/FH-05: Look up KB solutions and attempt repair for each symptom
        let mut repair_applied = false;
        let mut verification_passed = false;

        for symptom in &symptoms {
            let problem_key = format!("{}:{}", symptom.category, symptom.detail);

            // Query fleet KB for matching solutions
            let kb_solution = crate::fleet_kb::get_solution_by_hash(
                &state.db,
                &problem_key,
            )
            .await
            .ok()
            .flatten();

            if let Some(solution) = kb_solution {
                let fix_type_str = serde_json::to_string(&solution.fix_type)
                    .unwrap_or_else(|_| "\"Unknown\"".to_string())
                    .trim_matches('"')
                    .to_string();

                // FH-04: Confidence gate
                if RepairDispatcher::should_dispatch(solution.confidence, &fix_type_str) {
                    let repair = RepairAction {
                        action_id: action_id.clone(),
                        pod_id: pod_id.clone(),
                        ssh_command: solution.fix_action.to_string(),
                        description: format!(
                            "KB fix for {}: {}",
                            problem_key, solution.root_cause
                        ),
                        fix_type: fix_type_str.clone(),
                        confidence: solution.confidence,
                    };

                    // FH-08: Isolate if not deterministic
                    let needs_isolation = fix_type_str != "Deterministic";
                    if needs_isolation {
                        if let Err(e) = PodIsolation::isolate(pod_number).await {
                            tracing::warn!(
                                target: LOG_TARGET,
                                action_id = %action_id,
                                pod = pod_number,
                                error = %e,
                                "Failed to isolate pod — skipping repair"
                            );
                            continue;
                        }
                    }

                    // FH-05: Dispatch
                    let result = RepairDispatcher::dispatch(pod_number, &repair).await;

                    // FH-09: Log repair
                    if let Some(ssh_result) = &result.ssh_result {
                        AuditTrail::log_ssh_command(
                            &state.db,
                            &action_id,
                            ssh_result,
                            "repair",
                        )
                        .await;
                    }

                    repair_applied = true;

                    // FH-06: Post-fix verification
                    if result.success {
                        let ip = tailscale_ip(pod_number).unwrap_or("127.0.0.1");
                        let verification = PostFixVerifier::verify(
                            &state.http_client,
                            ip,
                            None,
                            3,
                            Duration::from_secs(10),
                        )
                        .await;

                        verification_passed = verification.passed;

                        AuditTrail::log_repair(
                            &state.db,
                            &action_id,
                            &pod_id,
                            "post_fix_verification",
                            verification.passed,
                            Some(
                                &serde_json::to_string(&verification).unwrap_or_default(),
                            ),
                        )
                        .await;
                    }

                    // FH-08: Clear isolation after verification
                    if needs_isolation {
                        let _ = PodIsolation::clear_isolation(pod_number).await;
                    }

                    // Break after first successful repair — don't stack fixes
                    if verification_passed {
                        break;
                    }
                }
            }
        }

        let outcome = HealPodOutcome {
            pod_id: pod_id.clone(),
            action_id: action_id.clone(),
            stage: "complete".to_string(),
            success: verification_passed || symptoms.is_empty(),
            blocked_reason: None,
            symptoms: symptoms.clone(),
            repair_applied,
            verification_passed,
        };

        AuditTrail::log_repair(
            &state.db,
            &action_id,
            &pod_id,
            "heal_complete",
            outcome.success,
            Some(&serde_json::to_string(&outcome).unwrap_or_default()),
        )
        .await;

        Ok(outcome)
    }
}

/// Outcome of a pod heal attempt.
#[derive(Debug, Clone, Serialize)]
pub struct HealPodOutcome {
    pub pod_id: String,
    pub action_id: String,
    pub stage: String,
    pub success: bool,
    pub blocked_reason: Option<String>,
    pub symptoms: Vec<Symptom>,
    pub repair_applied: bool,
    pub verification_passed: bool,
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tailscale IP map ─────────────────────────────────────────────────

    #[test]
    fn tailscale_ip_returns_correct_ips_for_all_8_pods() {
        assert_eq!(tailscale_ip(1), Some("100.92.122.89"));
        assert_eq!(tailscale_ip(2), Some("100.105.93.108"));
        assert_eq!(tailscale_ip(3), Some("100.69.231.26"));
        assert_eq!(tailscale_ip(4), Some("100.75.45.10"));
        assert_eq!(tailscale_ip(5), Some("100.110.133.87"));
        assert_eq!(tailscale_ip(6), Some("100.127.149.17"));
        assert_eq!(tailscale_ip(7), Some("100.82.196.28"));
        assert_eq!(tailscale_ip(8), Some("100.98.67.67"));
    }

    #[test]
    fn tailscale_ip_returns_none_for_invalid_pod() {
        assert_eq!(tailscale_ip(0), None);
        assert_eq!(tailscale_ip(9), None);
        assert_eq!(tailscale_ip(100), None);
    }

    // ── Diagnostic Fingerprinting ────────────────────────────────────────

    #[test]
    fn fingerprint_detects_missing_rcagent_in_tasklist() {
        let result = SshCommandResult {
            pod_id: "pod_1".into(),
            command: "tasklist /FO CSV /NH".into(),
            exit_code: Some(0),
            stdout: r#""svchost.exe","1234","Services","0","12,345 K"
"msedge.exe","5678","Console","1","98,765 K""#
                .into(),
            stderr: String::new(),
            duration_ms: 100,
            timestamp: Utc::now(),
        };

        let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
        assert!(
            symptoms.iter().any(|s| s.category == "process_missing" && s.detail == "rc-agent.exe"),
            "Should detect missing rc-agent.exe"
        );
    }

    #[test]
    fn fingerprint_detects_missing_edge() {
        let result = SshCommandResult {
            pod_id: "pod_2".into(),
            command: "tasklist /FO CSV /NH".into(),
            exit_code: Some(0),
            stdout: r#""rc-agent.exe","1234","Console","1","50,000 K""#.into(),
            stderr: String::new(),
            duration_ms: 100,
            timestamp: Utc::now(),
        };

        let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
        assert!(
            symptoms.iter().any(|s| s.category == "process_missing" && s.detail == "msedge.exe"),
            "Should detect missing msedge.exe"
        );
    }

    #[test]
    fn fingerprint_detects_port_not_listening() {
        let result = SshCommandResult {
            pod_id: "pod_3".into(),
            command: "netstat -an | findstr LISTEN".into(),
            exit_code: Some(0),
            stdout: "  TCP    0.0.0.0:445    0.0.0.0:0    LISTENING\n".into(),
            stderr: String::new(),
            duration_ms: 100,
            timestamp: Utc::now(),
        };

        let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
        assert!(
            symptoms.iter().any(|s| s.category == "port_not_listening" && s.detail.contains("8090")),
            "Should detect port 8090 not listening"
        );
    }

    #[test]
    fn fingerprint_detects_maintenance_mode() {
        let result = SshCommandResult {
            pod_id: "pod_4".into(),
            command: "if exist ...".into(),
            exit_code: Some(0),
            stdout: "MAINTENANCE_MODE_PRESENT\n".into(),
            stderr: String::new(),
            duration_ms: 50,
            timestamp: Utc::now(),
        };

        let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
        assert!(
            symptoms.iter().any(|s| s.category == "sentinel"),
            "Should detect MAINTENANCE_MODE sentinel"
        );
    }

    #[test]
    fn fingerprint_detects_session_zero() {
        let result = SshCommandResult {
            pod_id: "pod_5".into(),
            command: "powershell ... Get-Process rc-agent ...".into(),
            exit_code: Some(0),
            stdout: r#"{"Id":1234,"SessionId":0,"CPU":5.2,"WorkingSet64":52428800}"#.into(),
            stderr: String::new(),
            duration_ms: 200,
            timestamp: Utc::now(),
        };

        let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
        assert!(
            symptoms.iter().any(|s| s.category == "wrong_session"),
            "Should detect rc-agent in Session 0"
        );
    }

    #[test]
    fn fingerprint_detects_event_log_crash() {
        let result = SshCommandResult {
            pod_id: "pod_6".into(),
            command: "wevtutil qe Application /c:20".into(),
            exit_code: Some(0),
            stdout: "  Faulting application name: rc-agent.exe, version: 0.0.0.0\n".into(),
            stderr: String::new(),
            duration_ms: 300,
            timestamp: Utc::now(),
        };

        let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
        assert!(
            symptoms.iter().any(|s| s.category == "app_crash"),
            "Should detect application crash in event log"
        );
    }

    // ── Fleet Pattern Detection ──────────────────────────────────────────

    #[test]
    fn fleet_pattern_triggers_on_three_pods() {
        let mut detector = FleetPatternDetector::new();
        let symptom = Symptom {
            category: "process_missing".into(),
            detail: "rc-agent.exe".into(),
            severity: "critical".into(),
        };

        assert!(detector.record_failure("pod_1", &symptom).is_none());
        assert!(detector.record_failure("pod_2", &symptom).is_none());
        let pattern = detector.record_failure("pod_3", &symptom);
        assert!(pattern.is_some(), "Should trigger pattern on 3rd pod");
        let p = pattern.unwrap();
        assert_eq!(p.affected_pods.len(), 3);
    }

    #[test]
    fn fleet_pattern_deduplicates_same_pod() {
        let mut detector = FleetPatternDetector::new();
        let symptom = Symptom {
            category: "process_missing".into(),
            detail: "rc-agent.exe".into(),
            severity: "critical".into(),
        };

        assert!(detector.record_failure("pod_1", &symptom).is_none());
        assert!(detector.record_failure("pod_1", &symptom).is_none());
        assert!(detector.record_failure("pod_1", &symptom).is_none());
        // Same pod 3 times should NOT trigger — needs 3 different pods.
        assert!(detector.record_failure("pod_2", &symptom).is_none());
    }

    // ── Repair Confidence Gate ───────────────────────────────────────────

    #[test]
    fn confidence_gate_allows_high_confidence_deterministic() {
        assert!(RepairDispatcher::should_dispatch(0.9, "Deterministic"));
        assert!(RepairDispatcher::should_dispatch(0.8, "Config"));
        assert!(RepairDispatcher::should_dispatch(1.0, "Deterministic"));
    }

    #[test]
    fn confidence_gate_blocks_low_confidence() {
        assert!(!RepairDispatcher::should_dispatch(0.79, "Deterministic"));
        assert!(!RepairDispatcher::should_dispatch(0.5, "Config"));
        assert!(!RepairDispatcher::should_dispatch(0.0, "Deterministic"));
    }

    #[test]
    fn confidence_gate_blocks_non_deterministic_fix_types() {
        assert!(!RepairDispatcher::should_dispatch(0.95, "Restart"));
        assert!(!RepairDispatcher::should_dispatch(0.99, "CodeChange"));
        assert!(!RepairDispatcher::should_dispatch(1.0, "Manual"));
    }

    // ── Canary Rollout ───────────────────────────────────────────────────

    #[test]
    fn canary_rollout_order() {
        let waves = CanaryRollout::waves(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0], vec![8]); // canary
        assert_eq!(waves[1], vec![1, 2, 3]); // wave 1
        assert_eq!(waves[2], vec![4, 5, 6, 7]); // wave 2
    }

    #[test]
    fn canary_rollout_filters_targets() {
        let waves = CanaryRollout::waves(&[1, 8]);
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0], vec![8]);
        assert_eq!(waves[1], vec![1]);
    }

    #[test]
    fn canary_rollout_no_canary_pod() {
        let waves = CanaryRollout::waves(&[1, 2, 3]);
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0], vec![1, 2, 3]);
    }

    // ── Survival Report Ingester ─────────────────────────────────────────

    #[test]
    fn ingester_detects_trouble() {
        let mut ingester = SurvivalReportIngester::new();
        let report = SurvivalReport {
            pod_id: "pod_1".into(),
            source_layer: "watchdog".into(),
            status: "degraded".into(),
            timestamp: Utc::now(),
            diagnostics: None,
            uptime_secs: Some(120),
            build_id: Some("abc123".into()),
        };
        assert!(ingester.ingest(report));
    }

    #[test]
    fn ingester_healthy_is_not_trouble() {
        let mut ingester = SurvivalReportIngester::new();
        let report = SurvivalReport {
            pod_id: "pod_2".into(),
            source_layer: "watchdog".into(),
            status: "healthy".into(),
            timestamp: Utc::now(),
            diagnostics: None,
            uptime_secs: Some(3600),
            build_id: Some("def456".into()),
        };
        assert!(!ingester.ingest(report));
    }
}

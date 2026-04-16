//! Fleet Healer — Diagnosis subsystem: SSH diagnostics, fingerprinting, fleet patterns.
//!
//! Contains FH-01 (SSH Diagnostic Runner), FH-02 (Diagnostic Fingerprinting),
//! and FH-03 (Fleet Pattern Detection).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{FleetHealerError, LOG_TARGET, SSH_CMD_TIMEOUT_SECS, SSH_TIMEOUT_SECS, SSH_USER, tailscale_ip};

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
            if let Some(session_id) = info.get("SessionId").and_then(|v| v.as_i64())
                && session_id == 0 {
                    symptoms.push(Symptom {
                        category: "wrong_session".to_string(),
                        detail: "rc-agent running in Session 0 (should be Session 1)".to_string(),
                        severity: "critical".to_string(),
                    });
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

impl Default for FleetPatternDetector {
    fn default() -> Self {
        Self::new()
    }
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

//! Pod healer diagnostics: data collection, verification chains, heal actions, and helpers.
//!
//! Extracted from pod_healer.rs (Phase 385, v49.0 Architecture Completion).
//! Contains: collect_diagnostics, check_stale_sockets, check_disk_space, check_memory,
//! check_rc_agent_health (with ColdVerificationChain), check_processes, execute_heal_action,
//! exec_on_pod, is_protected_pid, has_active_billing.

use std::sync::Arc;

use serde_json::json;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::verification::{ColdVerificationChain, VerifyStep, VerificationError};

use crate::pod_healer::{
    PodDiagnostics, HealAction, PROTECTED_PROCESSES, MONITORED_PORTS,
    POD_AGENT_PORT, POD_AGENT_TIMEOUT,
};

// --- Diagnostics Collection --------------------------------------------------

pub(crate) async fn collect_diagnostics(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<PodDiagnostics> {
    // Run all diagnostic commands concurrently
    let (sockets_res, disk_res, memory_res, health_res, procs_res) = tokio::join!(
        check_stale_sockets(state, pod_ip),
        check_disk_space(state, pod_ip),
        check_memory(state, pod_ip),
        check_rc_agent_health(state, pod_ip),
        check_processes(state, pod_ip),
    );

    let stale_sockets = sockets_res.unwrap_or_default();
    let (disk_free_pct,) = disk_res.unwrap_or((100.0,));
    let (memory_free_mb, memory_total_mb) = memory_res.unwrap_or((8192, 32768));
    let rc_agent_healthy = health_res.unwrap_or(true); // assume healthy on error
    let suspicious_processes = procs_res.unwrap_or_default();

    Ok(PodDiagnostics {
        stale_sockets,
        disk_free_pct,
        memory_free_mb,
        memory_total_mb,
        rc_agent_healthy,
        suspicious_processes,
    })
}

/// Check for CLOSE_WAIT / TIME_WAIT sockets on monitored ports.
async fn check_stale_sockets(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<Vec<(u32, String)>> {
    let cmd = format!(
        "netstat -ano | findstr /C:\"CLOSE_WAIT\" /C:\"TIME_WAIT\" | findstr {}",
        MONITORED_PORTS
            .iter()
            .map(|p| format!("/C:\"{}\"", p))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let output = exec_on_pod(state, pod_ip, &cmd).await?;
    let mut results = Vec::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // netstat output: Proto LocalAddr ForeignAddr State PID
        if parts.len() >= 5 {
            let state_str = parts[3].to_string();
            if let Ok(pid) = parts[4].parse::<u32>() {
                if pid > 0 && (state_str == "CLOSE_WAIT" || state_str == "TIME_WAIT") {
                    // Deduplicate by PID
                    if !results.iter().any(|(p, _): &(u32, String)| *p == pid) {
                        results.push((pid, state_str));
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Check disk free space percentage on C: drive.
async fn check_disk_space(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<(f64,)> {
    let cmd = "wmic logicaldisk where \"DeviceID='C:'\" get size,freespace /format:csv";
    let output = exec_on_pod(state, pod_ip, cmd).await?;

    // CSV output: Node,FreeSpace,Size
    for line in output.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            if let (Ok(free), Ok(total)) = (
                parts[1].trim().parse::<f64>(),
                parts[2].trim().parse::<f64>(),
            ) {
                if total > 0.0 {
                    let pct_free = (free / total) * 100.0;
                    return Ok((pct_free,));
                }
            }
        }
    }

    Ok((100.0,)) // assume OK if parse fails
}

/// Check free physical memory.
async fn check_memory(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<(u64, u64)> {
    let cmd = "wmic OS get FreePhysicalMemory,TotalVisibleMemorySize /format:csv";
    let output = exec_on_pod(state, pod_ip, cmd).await?;

    // CSV: Node,FreePhysicalMemory,TotalVisibleMemorySize (in KB)
    for line in output.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            if let (Ok(free_kb), Ok(total_kb)) = (
                parts[1].trim().parse::<u64>(),
                parts[2].trim().parse::<u64>(),
            ) {
                return Ok((free_kb / 1024, total_kb / 1024)); // convert to MB
            }
        }
    }

    Ok((8192, 32768)) // default: assume 8GB free / 32GB total
}

// ─── Verification chain steps for curl parse (COV-02) ────────────────────────

struct StepRawStdout;
impl VerifyStep for StepRawStdout {
    type Input = String;  // raw exec output
    type Output = String; // same, but verified non-empty
    fn name(&self) -> &str { "raw_stdout_check" }
    fn run(&self, input: String) -> Result<String, VerificationError> {
        if input.trim().is_empty() {
            return Err(VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("(empty, len={})", input.len()),
            });
        }
        Ok(input)
    }
}

struct StepTrimQuotes;
impl VerifyStep for StepTrimQuotes {
    type Input = String;
    type Output = String;
    fn name(&self) -> &str { "trim_quotes" }
    fn run(&self, input: String) -> Result<String, VerificationError> {
        let trimmed = input.trim().trim_matches('"').to_string();
        Ok(trimmed)
    }
}

struct StepParseU32;
impl VerifyStep for StepParseU32 {
    type Input = String;
    type Output = u32;
    fn name(&self) -> &str { "parse_http_code" }
    fn run(&self, input: String) -> Result<u32, VerificationError> {
        input.parse::<u32>().map_err(|_| VerificationError::InputParseError {
            step: self.name().to_string(),
            raw_value: input,
        })
    }
}

struct StepCheckHttp200;
impl VerifyStep for StepCheckHttp200 {
    type Input = u32;
    type Output = bool;
    fn name(&self) -> &str { "check_http_200" }
    fn run(&self, input: u32) -> Result<bool, VerificationError> {
        Ok(input == 200)
    }
}

/// Check if rc-agent lock screen is responsive.
/// The lock screen binds to 127.0.0.1:18923, so we must check from the pod
/// itself via pod-agent exec rather than connecting directly to the pod's network IP.
pub(crate) async fn check_rc_agent_health(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<bool> {
    // Use curl.exe instead of PowerShell — cmd.exe strips $ variables from
    // PowerShell commands, causing $r to disappear and the check to always return 0.
    // curl.exe -s -o NUL -w %{http_code} is cmd.exe-safe (no $ variables).
    // Retry-once before declaring unhealthy (standing rule: never conclude offline from single probe).
    let cmd = r#"curl.exe -s -o NUL -w %{http_code} http://127.0.0.1:18923/ --max-time 3"#;
    let first = exec_on_pod(state, pod_ip, cmd).await;
    let exec_result = match &first {
        Ok(output) if output.trim().ends_with("200") || output.trim() == "200" => first,
        _ => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            exec_on_pod(state, pod_ip, cmd).await
        }
    };
    match exec_result {
        Ok(output) => {
            let chain = ColdVerificationChain::new("pod_healer_curl");
            match chain.execute_step(&StepRawStdout, output.clone()) {
                Ok(raw) => match chain.execute_step(&StepTrimQuotes, raw) {
                    Ok(trimmed) => match chain.execute_step(&StepParseU32, trimmed) {
                        Ok(code) => match chain.execute_step(&StepCheckHttp200, code) {
                            Ok(healthy) => Ok(healthy),
                            Err(e) => {
                                tracing::warn!(target: "pod_healer", error = %e, "verification chain failed");
                                Ok(false)
                            }
                        },
                        Err(e) => {
                            tracing::warn!(target: "pod_healer", error = %e, "curl output parse failed — raw value logged in chain step");
                            Ok(false)
                        }
                    },
                    Err(e) => {
                        tracing::warn!(target: "pod_healer", error = %e, "trim step failed");
                        Ok(false)
                    }
                },
                Err(e) => {
                    tracing::warn!(target: "pod_healer", error = %e, "raw stdout check failed");
                    Ok(false)
                }
            }
        }
        Err(_) => Ok(true), // if pod-agent exec fails, assume healthy (safe default)
    }
}

/// List running processes and flag suspicious ones (high memory, not in protected list).
async fn check_processes(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<Vec<(String, u32, u64)>> {
    let cmd = "tasklist /FO CSV /NH";
    let output = exec_on_pod(state, pod_ip, cmd).await?;

    let mut suspicious = Vec::new();
    let high_mem_threshold_kb: u64 = 500_000; // 500MB

    for line in output.lines() {
        // CSV: "Image Name","PID","Session Name","Session#","Mem Usage"
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 5 {
            let name = parts[0].trim_matches('"').to_lowercase();
            let pid: u32 = parts[1].trim_matches('"').parse().unwrap_or(0);
            // Mem Usage like "123,456 K" -- handle the comma in the number
            let mem_str = parts[4..]
                .join(",")
                .replace('"', "")
                .replace(" K", "")
                .replace(',', "")
                .trim()
                .to_string();
            let mem_kb: u64 = mem_str.parse().unwrap_or(0);

            if pid == 0 {
                continue;
            }

            // Flag if high memory AND not in protected list
            let is_protected = PROTECTED_PROCESSES
                .iter()
                .any(|p| name == *p || name.contains(p.trim_end_matches(".exe")));

            if !is_protected && mem_kb > high_mem_threshold_kb {
                suspicious.push((name, pid, mem_kb));
            }
        }
    }

    Ok(suspicious)
}

// --- Auto-Heal Actions -------------------------------------------------------

pub(crate) async fn execute_heal_action(state: &Arc<AppState>, pod_ip: &str, action: &HealAction) {
    // Relaunch lock screen: send ForceRelaunchBrowser over WS — no shell exec needed
    if action.action == "relaunch_lock_screen" {
        let senders = state.agent_senders.read().await;
        if let Some(sender) = senders.get(&action.pod_id) {
            let msg = CoreToAgentMessage::ForceRelaunchBrowser {
                pod_id: action.pod_id.clone(),
            };
            match sender.send(CoreMessage::wrap(msg)).await {
                Ok(_) => tracing::info!(
                    "Pod healer: ForceRelaunchBrowser sent to {} (lock screen recovery)",
                    action.pod_id
                ),
                Err(e) => tracing::warn!(
                    "Pod healer: ForceRelaunchBrowser send to {} failed: {}",
                    action.pod_id, e
                ),
            }
        } else {
            tracing::warn!(
                "Pod healer: ForceRelaunchBrowser -- no WS sender for {} (pod disconnected?)",
                action.pod_id
            );
        }
        return;
    }

    let cmd = match action.action.as_str() {
        "kill_zombie" => {
            // Extract PID from target like "PID 1234"
            let pid = action
                .target
                .strip_prefix("PID ")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            if pid == 0 {
                tracing::warn!("Pod healer: invalid PID in kill_zombie action");
                return;
            }
            format!("taskkill /F /PID {}", pid)
        }
        "clear_temp" => {
            r#"del /q /s C:\Users\*\AppData\Local\Temp\* >nul 2>&1"#.to_string()
        }
        _ => {
            tracing::warn!("Pod healer: unknown action type: {}", action.action);
            return;
        }
    };

    match exec_on_pod(state, pod_ip, &cmd).await {
        Ok(output) => {
            tracing::info!(
                "Pod healer: action '{}' on {} completed: {}",
                action.action,
                action.pod_id,
                output.chars().take(200).collect::<String>()
            );
        }
        Err(e) => {
            tracing::warn!(
                "Pod healer: action '{}' on {} failed: {}",
                action.action,
                action.pod_id,
                e
            );
        }
    }
}

// --- Helpers -----------------------------------------------------------------

/// Execute a command on a pod via pod-agent POST /exec.
pub(crate) async fn exec_on_pod(
    state: &Arc<AppState>,
    pod_ip: &str,
    command: &str,
) -> anyhow::Result<String> {
    let url = format!("http://{}:{}/exec", pod_ip, POD_AGENT_PORT);
    let resp = state
        .http_client
        .post(&url)
        .json(&json!({
            "cmd": command,
            "timeout_ms": 10000
        }))
        .timeout(POD_AGENT_TIMEOUT)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Pod exec returned HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    Ok(body["stdout"].as_str().unwrap_or("").to_string())
}

/// Check if a PID belongs to a protected process on the pod.
pub(crate) async fn is_protected_pid(state: &Arc<AppState>, pod_ip: &str, pid: u32) -> bool {
    let cmd = format!(
        "wmic process where ProcessId={} get Name /format:csv",
        pid
    );
    match exec_on_pod(state, pod_ip, &cmd).await {
        Ok(output) => {
            let name = output
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.contains("Node"))
                .next()
                .map(|l| {
                    l.split(',')
                        .last()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase()
                })
                .unwrap_or_default();

            PROTECTED_PROCESSES.iter().any(|p| name == *p)
        }
        Err(_) => true, // if we can't check, treat as protected (safe default)
    }
}

/// Check if a pod has an active billing session.
pub(crate) async fn has_active_billing(state: &Arc<AppState>, pod_id: &str) -> bool {
    let timers = state.billing.active_timers.read().await;
    timers.contains_key(pod_id)
}

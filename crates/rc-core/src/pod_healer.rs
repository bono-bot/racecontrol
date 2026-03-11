//! Pod Healer: Self-healing daemon with AI diagnostics.
//!
//! Runs every 2 minutes (configurable). For each connected pod, collects deep
//! diagnostics via pod-agent `/exec`, applies safe rule-based fixes (kill zombie
//! sockets, restart unresponsive rc-agent, clear temp files), and escalates
//! complex/unfamiliar issues to AI (Claude CLI -> Ollama -> Anthropic).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::activity_log::log_pod_activity;
use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::{AiDebugSuggestion, PodInfo, PodStatus, SimType};

const POD_AGENT_PORT: u16 = 8090;
const POD_AGENT_TIMEOUT: Duration = Duration::from_secs(10);
const HEAL_COOLDOWN_SECS: i64 = 600; // 10 min cooldown per pod

/// Processes that must NEVER be killed by the healer.
const PROTECTED_PROCESSES: &[&str] = &[
    "rc-agent.exe",
    "pod-agent.exe",
    "acs.exe",
    "conspitlink2.0.exe",
    "msedge.exe",
    "explorer.exe",
    "system",
    "svchost.exe",
    "csrss.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "dwm.exe",
    "taskhostw.exe",
    "conhost.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "vmsdesktop.exe",
    // James's machine runs as Pod 1 — these are infrastructure, not suspicious
    "claude.exe",
    "ollama.exe",
    "ollama_llama_server.exe",
    "deskin.exe",
];

/// Ports we monitor for stale sockets.
const MONITORED_PORTS: &[&str] = &["18923", "18924"];

/// Disk usage threshold (percent used) to trigger temp cleanup.
const DISK_THRESHOLD_PCT: f64 = 90.0;

/// Memory threshold (MB free) to flag as low memory.
const MEMORY_LOW_MB: u64 = 2048;

// ─── Types ──────────────────────────────────────────────────────────────────

struct PodDiagnostics {
    stale_sockets: Vec<(u32, String)>, // (PID, state like CLOSE_WAIT)
    disk_free_pct: f64,
    memory_free_mb: u64,
    memory_total_mb: u64,
    rc_agent_healthy: bool,
    suspicious_processes: Vec<(String, u32, u64)>, // (name, PID, mem_kb)
}

struct HealAction {
    pod_id: String,
    action: String,
    target: String,
    reason: String,
}

struct HealCooldown {
    last_action: DateTime<Utc>,
}

// ─── Spawn ──────────────────────────────────────────────────────────────────

/// Spawn the pod healer background task.
pub fn spawn(state: Arc<AppState>) {
    if !state.config.pods.healer_enabled {
        tracing::info!("Pod healer disabled");
        return;
    }

    let interval_secs = state.config.pods.healer_interval_secs as u64;

    tracing::info!(
        "Pod healer starting (interval: {}s, cooldown: {}s)",
        interval_secs,
        HEAL_COOLDOWN_SECS,
    );

    tokio::spawn(async move {
        // Wait for pods to connect before first scan
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut cooldowns: HashMap<String, HealCooldown> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;
            heal_all_pods(&state, &mut cooldowns).await;
        }
    });
}

// ─── Main Loop ──────────────────────────────────────────────────────────────

async fn heal_all_pods(
    state: &Arc<AppState>,
    cooldowns: &mut HashMap<String, HealCooldown>,
) {
    // Snapshot connected pods
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();

    let active_pods: Vec<&PodInfo> = pods
        .iter()
        .filter(|p| p.status != PodStatus::Disabled && p.last_seen.is_some())
        .collect();

    if active_pods.is_empty() {
        return;
    }

    tracing::info!("Pod healer: checking {} pods", active_pods.len());

    for pod in active_pods {
        if let Err(e) = heal_pod(state, pod, cooldowns).await {
            tracing::warn!("Pod healer: error checking pod {}: {}", pod.id, e);
        }
    }
}

async fn heal_pod(
    state: &Arc<AppState>,
    pod: &PodInfo,
    cooldowns: &mut HashMap<String, HealCooldown>,
) -> anyhow::Result<()> {
    // First verify pod-agent is reachable
    let ping_url = format!("http://{}:{}/ping", pod.ip_address, POD_AGENT_PORT);
    let ping = state
        .http_client
        .get(&ping_url)
        .timeout(Duration::from_millis(3000))
        .send()
        .await;

    let is_reachable = match &ping {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };
    if !is_reachable {
        // Pod-agent unreachable — pod_monitor handles this case
        return Ok(());
    }

    // Collect diagnostics
    let diag = collect_diagnostics(state, &pod.ip_address).await?;

    // Build issue list for potential AI escalation
    let mut issues: Vec<String> = Vec::new();
    let mut actions: Vec<HealAction> = Vec::new();

    // ─── Rule 1: Stale sockets ──────────────────────────────────────────
    if !diag.stale_sockets.is_empty() {
        for (pid, sock_state) in &diag.stale_sockets {
            if is_protected_pid(state, &pod.ip_address, *pid).await {
                issues.push(format!(
                    "Protected process PID {} has {} socket on monitored port",
                    pid, sock_state
                ));
            } else {
                actions.push(HealAction {
                    pod_id: pod.id.clone(),
                    action: "kill_zombie".to_string(),
                    target: format!("PID {}", pid),
                    reason: format!("{} socket on lock screen port", sock_state),
                });
            }
        }
    }

    // ─── Rule 2: rc-agent lock screen unresponsive ──────────────────────
    if !diag.rc_agent_healthy {
        let has_active_ws = state.agent_senders.read().await.contains_key(&pod.id);
        if has_active_ws {
            // Pod has an active WebSocket connection → rc-agent IS running.
            // Lock screen port check is a false positive (PowerShell flakiness,
            // antivirus, transient TCP issue). Do NOT restart — that would kill
            // the WebSocket and cause offline/online flapping.
            tracing::debug!(
                "Pod healer: {} lock screen unresponsive but WebSocket connected — skipping restart",
                pod.id
            );
        } else {
            let has_active_billing = has_active_billing(state, &pod.id).await;
            if has_active_billing {
                issues.push(format!(
                    "rc-agent lock screen unresponsive but pod has active billing — NOT restarting"
                ));
            } else {
                actions.push(HealAction {
                    pod_id: pod.id.clone(),
                    action: "restart_rc_agent".to_string(),
                    target: "rc-agent.exe".to_string(),
                    reason: "Lock screen (port 18923) unresponsive, no active billing, no WebSocket".to_string(),
                });
            }
        }
    }

    // ─── Rule 3: Disk space low ─────────────────────────────────────────
    if diag.disk_free_pct < (100.0 - DISK_THRESHOLD_PCT) {
        actions.push(HealAction {
            pod_id: pod.id.clone(),
            action: "clear_temp".to_string(),
            target: "C:\\Users\\*\\AppData\\Local\\Temp\\*".to_string(),
            reason: format!("Disk only {:.1}% free", diag.disk_free_pct),
        });
    }

    // ─── Rule 4: Low memory (alert only) ────────────────────────────────
    if diag.memory_free_mb < MEMORY_LOW_MB {
        issues.push(format!(
            "Low memory: {}MB free / {}MB total",
            diag.memory_free_mb, diag.memory_total_mb
        ));
    }

    // ─── Rule 5: Suspicious processes (alert only) ──────────────────────
    if !diag.suspicious_processes.is_empty() {
        for (name, pid, mem_kb) in &diag.suspicious_processes {
            issues.push(format!(
                "Suspicious process: {} (PID {}, {}MB RAM)",
                name, pid, mem_kb / 1024
            ));
        }
    }

    // Nothing to do
    if actions.is_empty() && issues.is_empty() {
        return Ok(());
    }

    // Check cooldown before executing actions
    let now = Utc::now();
    let cooldown_ok = match cooldowns.get(&pod.id) {
        Some(cd) => (now - cd.last_action).num_seconds() > HEAL_COOLDOWN_SECS,
        None => true,
    };

    // Execute auto-heal actions (if cooldown allows)
    if cooldown_ok && !actions.is_empty() {
        for action in &actions {
            tracing::info!(
                "Pod healer: [{}] {} -> {} ({})",
                action.pod_id, action.action, action.target, action.reason
            );
            let activity_action = match action.action.as_str() {
                "kill_zombie" => "Zombie Socket Killed",
                "restart_rc_agent" => "Lock Screen Fixed",
                "clear_temp" => "Disk Cleaned",
                _ => "Auto-Fix Applied",
            };
            log_pod_activity(state, &action.pod_id, "race_engineer", activity_action, &action.reason, "race_engineer");
            execute_heal_action(state, &pod.ip_address, action).await;
        }
        cooldowns.insert(
            pod.id.clone(),
            HealCooldown { last_action: now },
        );
    } else if !actions.is_empty() {
        tracing::info!(
            "Pod healer: {} has {} pending actions but cooldown not elapsed",
            pod.id,
            actions.len()
        );
    }

    // Escalate to AI if there are complex issues that rules can't handle
    // (respects same cooldown as heal actions to prevent spamming)
    if !issues.is_empty() && state.config.ai_debugger.enabled && cooldown_ok {
        log_pod_activity(state, &pod.id, "race_engineer", "AI Analysis Requested", &issues.join("; "), "race_engineer");
        escalate_to_ai(state, pod, &issues, &actions).await;
        cooldowns.insert(
            pod.id.clone(),
            HealCooldown { last_action: now },
        );
    }

    Ok(())
}

// ─── Diagnostics Collection ─────────────────────────────────────────────────

async fn collect_diagnostics(
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

/// Check if rc-agent lock screen is responsive.
/// The lock screen binds to 127.0.0.1:18923, so we must check from the pod
/// itself via pod-agent exec rather than connecting directly to the pod's network IP.
async fn check_rc_agent_health(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<bool> {
    let cmd = r#"powershell -NoProfile -Command "try { $r = Invoke-WebRequest -Uri 'http://127.0.0.1:18923/' -TimeoutSec 3 -UseBasicParsing; $r.StatusCode } catch { 0 }""#;
    match exec_on_pod(state, pod_ip, cmd).await {
        Ok(output) => {
            let code: u32 = output.trim().parse().unwrap_or(0);
            Ok(code == 200)
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
            // Mem Usage like "123,456 K" — handle the comma in the number
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

// ─── Auto-Heal Actions ─────────────────────────────────────────────────────

async fn execute_heal_action(
    state: &Arc<AppState>,
    pod_ip: &str,
    action: &HealAction,
) {
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
        "restart_rc_agent" => {
            r#"cd /d C:\RacingPoint & taskkill /F /IM rc-agent.exe >nul 2>&1 & timeout /t 3 /nobreak >nul & start /b rc-agent.exe"#.to_string()
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

// ─── AI Escalation ──────────────────────────────────────────────────────────

async fn escalate_to_ai(
    state: &Arc<AppState>,
    pod: &PodInfo,
    issues: &[String],
    actions_taken: &[HealAction],
) {
    let actions_desc = if actions_taken.is_empty() {
        "No auto-heal actions taken.".to_string()
    } else {
        actions_taken
            .iter()
            .map(|a| format!("  - {} on {} ({})", a.action, a.target, a.reason))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let context = format!(
        "POD HEALTH ALERT — Pod {} (#{}, IP: {})\n\n\
         Issues detected:\n{}\n\n\
         Auto-heal actions taken:\n{}\n\n\
         Pod status: {:?}, Last seen: {:?}, Current game: {:?}",
        pod.id,
        pod.number,
        pod.ip_address,
        issues
            .iter()
            .map(|i| format!("  - {}", i))
            .collect::<Vec<_>>()
            .join("\n"),
        actions_desc,
        pod.status,
        pod.last_seen,
        pod.current_game,
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are an expert Windows systems administrator and sim racing venue technician. \
                        Analyze the pod health issues below. Provide a brief root cause hypothesis \
                        and specific remediation steps. Focus on actionable fixes. Keep under 150 words."
        }),
        json!({
            "role": "user",
            "content": context.clone()
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("healer")).await {
        Ok((suggestion, model)) => {
            tracing::info!(
                "Pod healer AI suggestion for {} (via {}): {}",
                pod.id,
                model,
                suggestion.chars().take(100).collect::<String>()
            );

            let debug_suggestion = AiDebugSuggestion {
                pod_id: pod.id.clone(),
                sim_type: pod.current_game.unwrap_or(SimType::AssettoCorsa),
                error_context: context,
                suggestion,
                model,
                created_at: Utc::now(),
            };

            // Persist to DB
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, ?, ?, ?, ?, ?, 'healer')",
            )
            .bind(&id)
            .bind(&debug_suggestion.pod_id)
            .bind(
                serde_json::to_string(&debug_suggestion.sim_type)
                    .unwrap_or_default()
                    .trim_matches('"'),
            )
            .bind(&debug_suggestion.error_context)
            .bind(&debug_suggestion.suggestion)
            .bind(&debug_suggestion.model)
            .execute(&state.db)
            .await;

            // Broadcast to dashboard
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::AiDebugSuggestion(debug_suggestion));
        }
        Err(e) => {
            tracing::warn!("Pod healer AI escalation failed for {}: {}", pod.id, e);
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Execute a command on a pod via pod-agent POST /exec.
async fn exec_on_pod(
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
    Ok(body["stdout"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// Check if a PID belongs to a protected process on the pod.
async fn is_protected_pid(
    state: &Arc<AppState>,
    pod_ip: &str,
    pid: u32,
) -> bool {
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
async fn has_active_billing(state: &Arc<AppState>, pod_id: &str) -> bool {
    let timers = state.billing.active_timers.read().await;
    timers.contains_key(pod_id)
}

//! Deploy executor: orchestrates rc-agent binary deployment to pods.
//!
//! Uses self-swap pattern since rc-agent hosts both game management AND the
//! remote ops HTTP server (port 8090). We can't kill the process handling
//! our deploy commands, so we download alongside and swap via detached script.
//!
//! The deploy lifecycle for a single pod:
//!   1. Validate binary URL is reachable (HEAD request)
//!   2. Download new binary as rc-agent-new.exe (curl via /exec)
//!   3. Verify binary size >= 5MB threshold
//!   4. Write config from template (via /write on port 8090)
//!   5. Trigger self-swap: create do-swap.bat and run it detached
//!      (waits 3s → kills rc-agent → renames new→current → starts)
//!   6. Verify health: process alive + WebSocket connected + lock screen responsive
//!
//! Each step updates DeployState in AppState and broadcasts DashboardEvent::DeployProgress.
//! On failure at any step, state is set to Failed and an email alert is sent.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::DeployState;

const POD_AGENT_PORT: u16 = 8090;
const MIN_BINARY_SIZE: u64 = 5_000_000; // 5MB minimum for a valid rc-agent.exe
const VERIFY_DELAYS: &[u64] = &[5, 15, 30, 60];

/// Shorter health-check delays for rollback verification (50s total vs 110s for deploy).
/// Rollback restores a known-good binary — we expect faster recovery.
const ROLLBACK_VERIFY_DELAYS: &[u64] = &[5, 15, 30];

/// Batch script written to do-swap.bat on the pod before self-swap.
///
/// Sequence: wait 3s (for /exec to return) → kill rc-agent → preserve current as
/// rc-agent-prev.exe → move new binary to rc-agent.exe (with AV retry loop) → start.
/// CRLF line endings required — Windows batch files reject LF-only scripts.
const SWAP_SCRIPT_CONTENT: &str = "@echo off\r\n\
    cd /d C:\\RacingPoint\r\n\
    timeout /t 3 /nobreak >nul\r\n\
    taskkill /F /IM rc-agent.exe >nul 2>&1\r\n\
    timeout /t 2 /nobreak >nul\r\n\
    if exist rc-agent-prev.exe del /Q rc-agent-prev.exe >nul 2>&1\r\n\
    if exist rc-agent.exe move /Y rc-agent.exe rc-agent-prev.exe >nul 2>&1\r\n\
    set RETRIES=0\r\n\
    :RETRY\r\n\
    move /Y rc-agent-new.exe rc-agent.exe >nul 2>&1\r\n\
    if %ERRORLEVEL% NEQ 0 (\r\n\
        timeout /t 2 /nobreak >nul\r\n\
        set /a RETRIES+=1\r\n\
        if %RETRIES% LSS 5 goto RETRY\r\n\
        echo SWAP FAILED > C:\\RacingPoint\\deploy-error.log\r\n\
        exit /b 1\r\n\
    )\r\n\
    start \"\" /D C:\\RacingPoint rc-agent.exe\r\n";

/// Batch script written to do-rollback.bat on the pod when health verification fails.
///
/// Sequence: kill bad binary → restore rc-agent-prev.exe → start.
/// No sleep between kill and del — prevents watchdog from restarting the bad binary.
/// CRLF line endings required — Windows batch files reject LF-only scripts.
pub const ROLLBACK_SCRIPT_CONTENT: &str = "@echo off\r\n\
    cd /d C:\\RacingPoint\r\n\
    taskkill /F /IM rc-agent.exe >nul 2>&1\r\n\
    if exist rc-agent.exe del /Q rc-agent.exe >nul 2>&1\r\n\
    move /Y rc-agent-prev.exe rc-agent.exe\r\n\
    start \"\" /D C:\\RacingPoint rc-agent.exe\r\n";

/// Validate that a binary size meets the minimum threshold.
/// Returns Ok(()) if valid, Err with descriptive message if too small.
pub fn validate_binary_size(size_bytes: u64) -> Result<(), String> {
    if size_bytes < MIN_BINARY_SIZE {
        Err(format!(
            "Binary too small: {} bytes (minimum {} bytes / {}MB). \
             Possible corrupted download or HTML error page.",
            size_bytes, MIN_BINARY_SIZE, MIN_BINARY_SIZE / 1_000_000
        ))
    } else {
        Ok(())
    }
}

/// Parse the file size from Windows `dir` command output.
/// Example input line: "03/13/2026  10:30 AM        15,234,567 rc-agent.exe"
/// Returns the size in bytes, or None if parsing fails.
///
/// The `filename` parameter specifies which file to look for (e.g. "rc-agent.exe"
/// or "rc-agent-new.exe").
pub fn parse_file_size_from_dir(dir_output: &str, filename: &str) -> Option<u64> {
    for line in dir_output.lines() {
        if line.contains(filename)
            && !line.contains("File Not Found")
            && !line.contains("DIR")
        {
            // Split on whitespace, find the field before the filename
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if part.contains(filename) && i > 0 {
                    // The field before the filename is the size (may have commas)
                    let size_str = parts[i - 1].replace(',', "");
                    return size_str.parse::<u64>().ok();
                }
            }
        }
    }
    None
}

/// Human-readable label for a deploy state (used in progress messages and logs).
pub fn deploy_step_label(state: &DeployState) -> String {
    match state {
        DeployState::Idle => "Idle".to_string(),
        DeployState::Killing => "Killing old rc-agent process".to_string(),
        DeployState::WaitingDead => "Waiting for old process to exit".to_string(),
        DeployState::Downloading { progress_pct } => {
            format!("Downloading new binary ({}%)", progress_pct)
        }
        DeployState::SizeCheck => "Verifying binary size".to_string(),
        DeployState::Starting => "Starting new rc-agent process".to_string(),
        DeployState::VerifyingHealth => {
            "Verifying pod health (process + WS + lock screen)".to_string()
        }
        DeployState::Complete => "Deploy completed successfully".to_string(),
        DeployState::Failed { reason } => format!("Deploy failed: {}", reason),
        DeployState::WaitingSession => "Waiting for active billing session to end".to_string(),
        DeployState::RollingBack => "Rolling back to previous binary".to_string(),
    }
}

/// Generate pod config content from a hardcoded template.
/// Mirrors the template at deploy-staging/rc-agent.template.toml.
pub fn generate_pod_config(pod_number: u32) -> String {
    let pod_name = format!("Pod {:02}", pod_number);
    format!(
        r#"# rc-agent Configuration (generated by deploy executor)
# Pod {pod_number} — {pod_name}

[pod]
number = {pod_number}
name = "{pod_name}"
sim = "assetto_corsa"
sim_ip = "127.0.0.1"
sim_port = 9996

[core]
url = "ws://192.168.31.23:8080/ws/agent"

[games.assetto_corsa]
steam_app_id = 244210
use_steam = true

[games.f1_25]
steam_app_id = 3059520
use_steam = true

[games.assetto_corsa_evo]
steam_app_id = 3058630
use_steam = true

[games.assetto_corsa_rally]
steam_app_id = 3917090
use_steam = true

[games.le_mans_ultimate]
steam_app_id = 1564310
use_steam = true

[games.forza]
steam_app_id = 2440510
use_steam = true

[games.iracing]
steam_app_id = 266410
use_steam = true

[ai_debugger]
enabled = true
ollama_url = "http://192.168.31.27:11434"
ollama_model = "qwen2.5:3b"
"#,
        pod_number = pod_number,
        pod_name = pod_name
    )
}

/// Update deploy state for a pod, broadcast progress to dashboard, and log.
async fn set_deploy_state(state: &Arc<AppState>, pod_id: &str, deploy_state: DeployState) {
    let message = deploy_step_label(&deploy_state);
    let timestamp = Utc::now().to_rfc3339();

    // Update AppState
    {
        let mut deploy_states = state.pod_deploy_states.write().await;
        deploy_states.insert(pod_id.to_string(), deploy_state.clone());
    }

    // Broadcast to dashboard
    let _ = state.dashboard_tx.send(DashboardEvent::DeployProgress {
        pod_id: pod_id.to_string(),
        state: deploy_state.clone(),
        message: message.clone(),
        timestamp,
    });

    // Log significant state changes
    match &deploy_state {
        DeployState::Idle => {}
        _ => {
            tracing::info!("Deploy [{}]: {}", pod_id, message);
        }
    }
}

/// Execute a command on a pod via pod-agent HTTP POST /exec.
/// Returns (success, stdout, stderr).
async fn http_exec_on_pod(
    state: &Arc<AppState>,
    pod_ip: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String> {
    let url = format!("http://{}:{}/exec", pod_ip, POD_AGENT_PORT);
    let result = state
        .http_client
        .post(&url)
        .json(&serde_json::json!({
            "cmd": cmd,
            "timeout_ms": timeout_ms
        }))
        .timeout(Duration::from_millis(timeout_ms + 5000))
        .send()
        .await;

    match result {
        Ok(resp) => {
            let status_ok = resp.status().is_success();
            match resp.json::<serde_json::Value>().await {
                Ok(body) => {
                    let success = body["success"].as_bool().unwrap_or(status_ok);
                    let stdout = body["stdout"].as_str().unwrap_or("").to_string();
                    let stderr = body["stderr"].as_str().unwrap_or("").to_string();
                    Ok((success, stdout, stderr))
                }
                Err(e) => Err(format!("Failed to parse exec response: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to reach pod-agent: {}", e)),
    }
}

/// Execute a command on a pod: try HTTP first, fall back to WebSocket.
///
/// HTTP is preferred (lower latency, direct). If HTTP fails (firewall, pod-agent down),
/// falls back to WebSocket which is always reachable (outbound connection from pod).
async fn exec_on_pod(
    state: &Arc<AppState>,
    pod_id: &str,
    pod_ip: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String> {
    match http_exec_on_pod(state, pod_ip, cmd, timeout_ms).await {
        Ok(result) => Ok(result),
        Err(http_err) => {
            tracing::warn!(
                "HTTP command failed for {} ({}): {}. Trying WS fallback.",
                pod_id, pod_ip, http_err
            );
            crate::ws::ws_exec_on_pod(state, pod_id, cmd, timeout_ms).await
        }
    }
}

/// Check if rc-agent.exe is running on the pod.
async fn is_process_alive(state: &Arc<AppState>, pod_id: &str, pod_ip: &str) -> bool {
    match exec_on_pod(state, pod_id, pod_ip, "tasklist /NH | findstr rc-agent", 5000).await {
        Ok((_, stdout, _)) => stdout.contains("rc-agent"),
        Err(_) => false,
    }
}

/// Check if WebSocket channel is open for a pod.
async fn is_ws_connected(state: &Arc<AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    match senders.get(pod_id) {
        Some(sender) => !sender.is_closed(),
        None => false,
    }
}

/// Check if lock screen HTTP server is responsive on the pod.
async fn is_lock_screen_healthy(state: &Arc<AppState>, pod_id: &str, pod_ip: &str) -> bool {
    let cmd = r#"powershell -NoProfile -Command "try { $r = Invoke-WebRequest -Uri 'http://127.0.0.1:18923/health' -TimeoutSec 3 -UseBasicParsing; $r.StatusCode } catch { 0 }""#;
    match exec_on_pod(state, pod_id, pod_ip, cmd, 8000).await {
        Ok((_, stdout, _)) => {
            let code: u32 = stdout.trim().parse().unwrap_or(0);
            code == 200
        }
        Err(_) => false,
    }
}

/// Check if a deploy has been cancelled (state set to Failed by CancelDeploy command).
async fn is_cancelled(state: &Arc<AppState>, pod_id: &str) -> bool {
    let deploy_states = state.pod_deploy_states.read().await;
    matches!(deploy_states.get(pod_id), Some(DeployState::Failed { .. }))
}

/// Deploy rc-agent to a single pod using self-swap pattern.
///
/// This is the main deploy executor. It runs as a tokio::spawn'd background task.
/// The caller (API endpoint or rolling deploy) should spawn this and return immediately.
///
/// Steps:
/// 1. Validate binary URL is reachable
/// 2. Download new binary as rc-agent-new.exe
/// 3. Size check
/// 4. Write config
/// 5. Trigger detached self-swap (bat script kills → renames → starts)
/// 9. Verify health (process + WS + lock screen)
pub async fn deploy_pod(
    state: Arc<AppState>,
    pod_id: String,
    pod_ip: String,
    binary_url: String,
) {
    // Bug #12: Global 5-minute timeout — if deploy hasn't completed, mark as failed.
    const DEPLOY_GLOBAL_TIMEOUT_SECS: u64 = 300;
    let state_timeout = state.clone();
    let pod_id_timeout = pod_id.clone();
    if tokio::time::timeout(
        Duration::from_secs(DEPLOY_GLOBAL_TIMEOUT_SECS),
        deploy_pod_inner(state.clone(), pod_id.clone(), pod_ip, binary_url),
    )
    .await
    .is_err()
    {
        let reason = format!("Deploy timed out after {}s — marking as failed", DEPLOY_GLOBAL_TIMEOUT_SECS);
        tracing::error!("Deploy [{}]: {}", pod_id_timeout, reason);
        set_deploy_state(&state_timeout, &pod_id_timeout, DeployState::Failed { reason }).await;
    }
}

/// Inner deploy function wrapped by the global timeout in deploy_pod.
async fn deploy_pod_inner(
    state: Arc<AppState>,
    pod_id: String,
    pod_ip: String,
    binary_url: String,
) {
    // Step 0: Validate binary URL is reachable BEFORE killing old process
    tracing::info!("Deploy [{}]: validating binary URL: {}", pod_id, binary_url);
    match state
        .http_client
        .head(&binary_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Deploy [{}]: binary URL reachable", pod_id);
        }
        Ok(resp) => {
            let reason = format!("Binary URL returned HTTP {}: {}", resp.status(), binary_url);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            return;
        }
        Err(e) => {
            let reason = format!("Binary URL unreachable: {} ({})", binary_url, e);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            return;
        }
    }

    // Step 1: Download new binary as rc-agent-new.exe (self-swap pattern)
    // rc-agent hosts both game management AND remote ops on port 8090.
    // We can't kill it to replace it — instead download alongside, then swap.
    set_deploy_state(&state, &pod_id, DeployState::Downloading { progress_pct: 0 }).await;
    log_pod_activity(
        &state,
        &pod_id,
        "deploy",
        "Deploy Started",
        &format!("Binary: {} (self-swap)", binary_url),
        "deploy",
    );

    // Clean any stale staging binary first
    let _ = exec_on_pod(
        &state,
        &pod_id,
        &pod_ip,
        "del /F C:\\RacingPoint\\rc-agent-new.exe",
        5000,
    )
    .await;

    let download_cmd = format!(
        "curl.exe -s -f -o C:\\RacingPoint\\rc-agent-new.exe {}",
        binary_url
    );
    match exec_on_pod(&state, &pod_id, &pod_ip, &download_cmd, 120_000).await {
        Ok((success, _stdout, stderr)) => {
            if !success {
                let reason = format!(
                    "Binary download failed: {}",
                    stderr.chars().take(200).collect::<String>()
                );
                set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() })
                    .await;
                send_deploy_failure_alert(&state, &pod_id, &reason).await;
                log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy");
                return;
            }
        }
        Err(e) => {
            let reason = format!("Download command failed: {}", e);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy");
            return;
        }
    }
    set_deploy_state(
        &state,
        &pod_id,
        DeployState::Downloading { progress_pct: 100 },
    )
    .await;

    // Step 2: Size check on rc-agent-new.exe
    set_deploy_state(&state, &pod_id, DeployState::SizeCheck).await;
    let dir_result = exec_on_pod(
        &state,
        &pod_id,
        &pod_ip,
        "dir C:\\RacingPoint\\rc-agent-new.exe",
        5000,
    )
    .await;
    match dir_result {
        Ok((_, stdout, _)) => match parse_file_size_from_dir(&stdout, "rc-agent-new.exe") {
            Some(size) => {
                if let Err(reason) = validate_binary_size(size) {
                    set_deploy_state(
                        &state,
                        &pod_id,
                        DeployState::Failed { reason: reason.clone() },
                    )
                    .await;
                    send_deploy_failure_alert(&state, &pod_id, &reason).await;
                    log_pod_activity(
                        &state,
                        &pod_id,
                        "deploy",
                        "Deploy Failed",
                        &reason,
                        "deploy",
                    );
                    return;
                }
                tracing::info!(
                    "Deploy [{}]: binary size OK ({} bytes)",
                    pod_id, size
                );
            }
            None => {
                let reason = format!(
                    "Could not parse binary size from dir output: {}",
                    stdout.chars().take(200).collect::<String>()
                );
                set_deploy_state(
                    &state,
                    &pod_id,
                    DeployState::Failed { reason: reason.clone() },
                )
                .await;
                send_deploy_failure_alert(&state, &pod_id, &reason).await;
                log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy");
                return;
            }
        },
        Err(e) => {
            let reason = format!("Dir command failed: {}", e);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy");
            return;
        }
    }

    // Cancellation check before writing config
    if is_cancelled(&state, &pod_id).await {
        return;
    }

    // Step 6: Write config (generate from template based on pod number)
    // Extract pod_number from pod_id ("pod_3" -> 3)
    let pod_number: u32 = pod_id
        .strip_prefix("pod_")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);

    if pod_number >= 1 && pod_number <= 8 {
        let config_content = generate_pod_config(pod_number);
        let write_url = format!("http://{}:{}/write", pod_ip, POD_AGENT_PORT);
        let write_result = state
            .http_client
            .post(&write_url)
            .json(&serde_json::json!({
                "path": "C:\\RacingPoint\\rc-agent.toml",
                "content": config_content
            }))
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match write_result {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Deploy [{}]: config written", pod_id);
            }
            Ok(resp) => {
                tracing::warn!(
                    "Deploy [{}]: config write returned HTTP {} -- proceeding with existing config",
                    pod_id,
                    resp.status()
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Deploy [{}]: config write failed: {} -- proceeding with existing config",
                    pod_id,
                    e
                );
            }
        }
    }

    // Step 5: Trigger self-swap via detached batch script.
    // Write do-swap.bat via /write endpoint (cleaner than echo pipeline), then run detached.
    // The script: waits 3s → kills rc-agent → preserves current as rc-agent-prev.exe →
    // moves new→current (with AV retry) → starts new binary.
    set_deploy_state(&state, &pod_id, DeployState::Starting).await;

    // Write do-swap.bat via /write endpoint (clean, no echo pipeline)
    let write_url = format!("http://{}:{}/write", pod_ip, POD_AGENT_PORT);
    let write_result = state
        .http_client
        .post(&write_url)
        .json(&serde_json::json!({
            "path": "C:\\RacingPoint\\do-swap.bat",
            "content": SWAP_SCRIPT_CONTENT
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await;

    match write_result {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Deploy [{}]: do-swap.bat written via /write", pod_id);
        }
        Ok(resp) => {
            let reason = format!("Failed to write do-swap.bat: HTTP {}", resp.status());
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            return;
        }
        Err(e) => {
            let reason = format!("Failed to write do-swap.bat: {}", e);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            return;
        }
    }

    // Run do-swap.bat detached (returns immediately; bat takes ~5s to run)
    let _ = exec_on_pod(
        &state,
        &pod_id,
        &pod_ip,
        r#"start /min cmd /c C:\RacingPoint\do-swap.bat"#,
        5000,
    )
    .await;

    // Step 6: Verify health (process + WS + lock screen)
    // Self-swap takes ~5s (3s wait + 2s kill/rename/start), so first check at 5s is expected to find nothing.
    set_deploy_state(&state, &pod_id, DeployState::VerifyingHealth).await;

    for delay in VERIFY_DELAYS {
        tokio::time::sleep(Duration::from_secs(*delay)).await;

        if is_cancelled(&state, &pod_id).await {
            return;
        }

        let process_ok = is_process_alive(&state, &pod_id, &pod_ip).await;
        if !process_ok {
            continue; // process not yet started -- wait for next check
        }

        let ws_ok = is_ws_connected(&state, &pod_id).await;
        let lock_ok = is_lock_screen_healthy(&state, &pod_id, &pod_ip).await;

        if ws_ok && lock_ok {
            // Full health verified
            set_deploy_state(&state, &pod_id, DeployState::Complete).await;
            log_pod_activity(
                &state,
                &pod_id,
                "deploy",
                "Deploy Completed",
                &format!(
                    "Binary deployed and verified healthy after {}s",
                    delay
                ),
                "deploy",
            );
            // Reset to Idle after a brief delay so dashboard can show Complete
            tokio::time::sleep(Duration::from_secs(10)).await;
            set_deploy_state(&state, &pod_id, DeployState::Idle).await;
            return;
        }
    }

    // All verify delays exhausted without full health — determine failure reason
    let process_ok = is_process_alive(&state, &pod_id, &pod_ip).await;
    let ws_ok = is_ws_connected(&state, &pod_id).await;
    let lock_ok = is_lock_screen_healthy(&state, &pod_id, &pod_ip).await;

    let failure_reason = if !process_ok {
        "Process not running after start".to_string()
    } else if !ws_ok {
        "WebSocket not connected after 60s".to_string()
    } else if !lock_ok {
        "Lock screen not responsive after 60s".to_string()
    } else {
        "Health verification failed (unknown reason)".to_string()
    };

    tracing::warn!("Deploy [{}]: health check failed: {}", pod_id, failure_reason);

    // Check if rc-agent-prev.exe exists for rollback
    let prev_check = exec_on_pod(
        &state,
        &pod_id,
        &pod_ip,
        "if exist C:\\RacingPoint\\rc-agent-prev.exe (echo EXISTS) else (echo MISSING)",
        5000,
    )
    .await;

    let prev_exists = match &prev_check {
        Ok((_, stdout, _)) => stdout.contains("EXISTS"),
        Err(_) => false,
    };

    if prev_exists {
        tracing::info!("Deploy [{}]: rc-agent-prev.exe found, triggering rollback", pod_id);
        set_deploy_state(&state, &pod_id, DeployState::RollingBack).await;

        // Write do-rollback.bat via /write endpoint
        let write_url = format!("http://{}:{}/write", pod_ip, POD_AGENT_PORT);
        let rollback_written = match state
            .http_client
            .post(&write_url)
            .json(&serde_json::json!({
                "path": "C:\\RacingPoint\\do-rollback.bat",
                "content": ROLLBACK_SCRIPT_CONTENT
            }))
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => true,
            _ => false,
        };

        if !rollback_written {
            let reason = format!(
                "Health failed ({}), rollback script write also failed",
                failure_reason
            );
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy");
            return;
        }

        // Execute rollback script detached
        let _ = exec_on_pod(
            &state,
            &pod_id,
            &pod_ip,
            r#"start /min cmd /c C:\RacingPoint\do-rollback.bat"#,
            5000,
        )
        .await;

        // Verify rollback health with shorter delays
        let mut rollback_healthy = false;
        for delay in ROLLBACK_VERIFY_DELAYS {
            tokio::time::sleep(Duration::from_secs(*delay)).await;

            if is_cancelled(&state, &pod_id).await {
                return;
            }

            let proc_ok = is_process_alive(&state, &pod_id, &pod_ip).await;
            if !proc_ok {
                continue;
            }

            let ws_ok = is_ws_connected(&state, &pod_id).await;
            let lock_ok = is_lock_screen_healthy(&state, &pod_id, &pod_ip).await;

            if ws_ok && lock_ok {
                rollback_healthy = true;
                break;
            }
        }

        if rollback_healthy {
            tracing::info!("Deploy [{}]: rollback succeeded, previous binary restored", pod_id);
            set_deploy_state(
                &state,
                &pod_id,
                DeployState::Failed {
                    reason: format!(
                        "Deploy failed ({}), rolled back to previous binary",
                        failure_reason
                    ),
                },
            )
            .await;
            log_pod_activity(
                &state,
                &pod_id,
                "deploy",
                "Deploy Rolled Back",
                &format!(
                    "Health failed: {}. Rolled back to rc-agent-prev.exe.",
                    failure_reason
                ),
                "deploy",
            );
            // Note: state stays Failed (with rollback context in reason) — pod is alive.
            // No separate RolledBack variant; the reason string tells the dashboard.
        } else {
            let reason = format!(
                "Deploy failed ({}) AND rollback failed -- pod may need manual intervention",
                failure_reason
            );
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            log_pod_activity(
                &state,
                &pod_id,
                "deploy",
                "Deploy + Rollback Failed",
                &reason,
                "deploy",
            );
        }
    } else {
        // No previous binary available — cannot rollback (first deploy, or prev was deleted)
        tracing::warn!("Deploy [{}]: no rc-agent-prev.exe found, cannot rollback", pod_id);
        set_deploy_state(
            &state,
            &pod_id,
            DeployState::Failed { reason: failure_reason.clone() },
        )
        .await;
        send_deploy_failure_alert(&state, &pod_id, &failure_reason).await;
        log_pod_activity(
            &state,
            &pod_id,
            "deploy",
            "Deploy Failed",
            &failure_reason,
            "deploy",
        );
    }
}

/// Rolling deploy: Pod 8 first (canary), then remaining pods.
///
/// Pods with active billing sessions are queued (WaitingSession state).
/// Their binary URL is stored in `pending_deploys` — when that pod's session
/// ends, `check_and_trigger_pending_deploy()` fires the deploy automatically.
///
/// If the canary (Pod 8) fails, the rolling deploy halts: no other pods are touched.
/// Non-canary failures are logged but do not halt the rolling deploy.
///
/// This function resolves pod IPs from AppState.pods at call time.
pub async fn deploy_rolling(
    state: Arc<AppState>,
    binary_url: String,
) -> Result<(), String> {
    let canary_id = "pod_8".to_string();

    // Build ordered pod list: canary first, then 1-7 ascending.
    // Resolve IPs from AppState.pods (only deploy to known/connected pods).
    let ordered_pods: Vec<(String, String)> = {
        let pods = state.pods.read().await;
        let mut entries: Vec<(String, String)> = pods
            .iter()
            .map(|(id, p)| (id.clone(), p.ip_address.clone()))
            .collect();
        // Sort: pod_8 canary first (key=0), then ascending by pod number
        entries.sort_by_key(|(id, _)| {
            if id == "pod_8" {
                0u32
            } else {
                id.strip_prefix("pod_")
                    .and_then(|n| n.parse::<u32>().ok())
                    .unwrap_or(99)
            }
        });
        entries
    };

    tracing::info!(
        "Rolling deploy: {} pods found, canary=pod_8, binary={}",
        ordered_pods.len(),
        binary_url
    );

    // Phase 1: Deploy canary (Pod 8) synchronously — must succeed before continuing.
    let canary_ip = {
        let pods = state.pods.read().await;
        pods.get(&canary_id).map(|p| p.ip_address.clone())
    };

    let canary_ip = match canary_ip {
        Some(ip) => ip,
        None => {
            return Err("Canary pod_8 not found in AppState.pods — cannot start rolling deploy".to_string());
        }
    };

    tracing::info!("Rolling deploy: starting canary on pod_8 ({})", canary_ip);
    deploy_pod(state.clone(), canary_id.clone(), canary_ip, binary_url.clone()).await;

    // Check canary result
    {
        let deploy_states = state.pod_deploy_states.read().await;
        match deploy_states.get(&canary_id) {
            Some(DeployState::Complete) | Some(DeployState::Idle) => {
                // Idle means it reset after completion (deploy_pod resets to Idle after 10s)
                tracing::info!("Rolling deploy: canary pod_8 succeeded, proceeding to remaining pods");
            }
            Some(DeployState::Failed { reason }) => {
                return Err(format!(
                    "Canary pod_8 failed: {}. Rolling deploy halted — no other pods touched.",
                    reason
                ));
            }
            other => {
                return Err(format!(
                    "Canary pod_8 in unexpected state: {:?}. Rolling deploy halted.",
                    other
                ));
            }
        }
    }

    // Phase 2: Deploy remaining pods (1-7), respecting active billing sessions.
    for (pod_id, pod_ip) in ordered_pods.iter().filter(|(id, _)| id != &canary_id) {
        let has_active_session = {
            let timers = state.billing.active_timers.read().await;
            timers.contains_key(pod_id)
        };

        if has_active_session {
            tracing::info!(
                "Rolling deploy: {} has active billing session, queuing for session-end",
                pod_id
            );
            // Set WaitingSession state and broadcast
            set_deploy_state(&state, pod_id, DeployState::WaitingSession).await;
            // Store URL for session-end hook
            {
                let mut pending = state.pending_deploys.write().await;
                pending.insert(pod_id.clone(), binary_url.clone());
            }
            continue;
        }

        // No active session — deploy immediately
        tracing::info!("Rolling deploy: deploying to {}", pod_id);
        if let Err(e) = {
            // deploy_pod is infallible (returns ()), so we just spawn inline
            deploy_pod(state.clone(), pod_id.clone(), pod_ip.clone(), binary_url.clone()).await;
            Ok::<(), String>(())
        } {
            tracing::error!("Rolling deploy: {} failed: {} (continuing)", pod_id, e);
        }

        // Brief delay between pods to avoid concurrent disk/network load
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    // ── Fleet summary and retry (DEP-04) ────────────────────────────────────
    // NOTE: ordered_pods includes pod_8 (canary). The canary is always Idle/Complete
    // at this point — if it had failed, deploy_rolling() returned Err early and never
    // reached this block. No special canary exclusion needed in the retry loop.
    let initial_states = deploy_status(&state).await;
    let mut succeeded: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let mut waiting: Vec<String> = Vec::new();

    for (pod_id, _) in &ordered_pods {
        match initial_states.get(pod_id) {
            Some(DeployState::Complete) | Some(DeployState::Idle) => {
                succeeded.push(pod_id.clone());
            }
            Some(DeployState::Failed { .. }) => {
                failed.push(pod_id.clone());
            }
            Some(DeployState::WaitingSession) => {
                waiting.push(pod_id.clone());
            }
            _ => {
                // Still in progress or unknown -- treat as waiting
                waiting.push(pod_id.clone());
            }
        }
    }

    // Retry failed pods once (excluding canary -- if canary failed, we already halted above)
    if !failed.is_empty() {
        tracing::warn!(
            "Rolling deploy: retrying {} failed pod(s): {:?}",
            failed.len(),
            failed
        );

        let retry_pod_ids: Vec<String> = failed.drain(..).collect();
        for retry_id in &retry_pod_ids {
            let retry_ip = {
                let pods = state.pods.read().await;
                pods.get(retry_id).map(|p| p.ip_address.clone())
            };
            if let Some(ip) = retry_ip {
                tracing::info!("Rolling deploy: retry deploying to {}", retry_id);
                deploy_pod(state.clone(), retry_id.clone(), ip, binary_url.clone()).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        // Recheck retry results
        let retry_states = deploy_status(&state).await;
        for retry_id in &retry_pod_ids {
            match retry_states.get(retry_id) {
                Some(DeployState::Complete) | Some(DeployState::Idle) => {
                    succeeded.push(retry_id.clone());
                }
                _ => {
                    failed.push(retry_id.clone());
                }
            }
        }
    }

    // Log structured summary
    tracing::info!(
        "Rolling deploy COMPLETE: succeeded={:?} failed={:?} waiting_session={:?}",
        succeeded,
        failed,
        waiting
    );

    // Broadcast fleet summary event to dashboard
    let _ = state.dashboard_tx.send(DashboardEvent::FleetDeploySummary {
        succeeded: succeeded.clone(),
        failed: failed.clone(),
        waiting: waiting.clone(),
        timestamp: Utc::now().to_rfc3339(),
    });

    Ok(())
}

/// Called when a billing session ends on a pod.
///
/// If a pending deploy is waiting for this pod (queued during a rolling deploy
/// while the pod had an active session), this function fires `deploy_pod` immediately.
///
/// Called from billing.rs after removing the timer from `active_timers`.
pub async fn check_and_trigger_pending_deploy(state: &Arc<AppState>, pod_id: &str) {
    // Check for a pending deploy URL for this pod
    let binary_url = {
        let mut pending = state.pending_deploys.write().await;
        pending.remove(pod_id)
    };

    let binary_url = match binary_url {
        Some(url) => url,
        None => return, // No pending deploy for this pod
    };

    // Resolve pod IP
    let pod_ip = {
        let pods = state.pods.read().await;
        pods.get(pod_id).map(|p| p.ip_address.clone())
    };

    let pod_ip = match pod_ip {
        Some(ip) => ip,
        None => {
            tracing::warn!(
                "Pending deploy for {} triggered at session-end but pod not found",
                pod_id
            );
            return;
        }
    };

    tracing::info!(
        "Session ended on {} — triggering deferred rolling deploy: {}",
        pod_id,
        binary_url
    );

    let state = Arc::clone(state);
    let pod_id = pod_id.to_string();
    tokio::spawn(async move {
        deploy_pod(state, pod_id, pod_ip, binary_url).await;
    });
}

/// Get the current deploy state for all 8 pods.
/// Returns a HashMap of pod_id -> DeployState.
pub async fn deploy_status(state: &Arc<AppState>) -> HashMap<String, DeployState> {
    state.pod_deploy_states.read().await.clone()
}

/// Send an email alert for a deploy failure.
async fn send_deploy_failure_alert(state: &Arc<AppState>, pod_id: &str, reason: &str) {
    let subject = format!("[RaceControl] Deploy FAILED on {}", pod_id);
    let body = format!(
        "Deploy to {} failed.\n\nReason: {}\n\nTime: {}\n\nAction: Check pod manually or retry deploy.",
        pod_id,
        reason,
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    );
    state
        .email_alerter
        .write()
        .await
        .send_alert(pod_id, &subject, &body)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_binary_size tests ──────────────────────────────────────────

    #[test]
    fn validate_binary_size_at_threshold_ok() {
        assert!(validate_binary_size(5_000_000).is_ok());
    }

    #[test]
    fn validate_binary_size_above_threshold_ok() {
        assert!(validate_binary_size(5_000_001).is_ok());
        assert!(validate_binary_size(15_000_000).is_ok());
    }

    #[test]
    fn validate_binary_size_below_threshold_err() {
        assert!(validate_binary_size(4_999_999).is_err());
    }

    #[test]
    fn validate_binary_size_zero_err() {
        let result = validate_binary_size(0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too small"));
    }

    #[test]
    fn validate_binary_size_small_file_err() {
        // HTML error page saved as .exe
        assert!(validate_binary_size(1024).is_err());
    }

    // ── parse_file_size_from_dir tests ──────────────────────────────────────

    #[test]
    fn parse_file_size_normal_dir_output() {
        let output = " Volume in drive C is Windows\n Directory of C:\\RacingPoint\n\n03/13/2026  10:30 AM        15,234,567 rc-agent.exe\n               1 File(s)     15,234,567 bytes\n";
        assert_eq!(parse_file_size_from_dir(output, "rc-agent.exe"), Some(15234567));
    }

    #[test]
    fn parse_file_size_file_not_found() {
        let output = "File Not Found\n";
        assert_eq!(parse_file_size_from_dir(output, "rc-agent.exe"), None);
    }

    #[test]
    fn parse_file_size_empty() {
        assert_eq!(parse_file_size_from_dir("", "rc-agent.exe"), None);
    }

    #[test]
    fn parse_file_size_no_commas() {
        let output = "03/13/2026  10:30 AM        15234567 rc-agent.exe\n";
        assert_eq!(parse_file_size_from_dir(output, "rc-agent.exe"), Some(15234567));
    }

    // ── deploy_step_label tests ─────────────────────────────────────────────

    #[test]
    fn deploy_step_label_killing() {
        let label = deploy_step_label(&DeployState::Killing);
        assert_eq!(label, "Killing old rc-agent process");
    }

    #[test]
    fn deploy_step_label_failed() {
        let label = deploy_step_label(&DeployState::Failed {
            reason: "binary too small".to_string(),
        });
        assert_eq!(label, "Deploy failed: binary too small");
    }

    #[test]
    fn deploy_step_label_downloading() {
        let label = deploy_step_label(&DeployState::Downloading { progress_pct: 75 });
        assert_eq!(label, "Downloading new binary (75%)");
    }

    #[test]
    fn deploy_step_label_complete() {
        let label = deploy_step_label(&DeployState::Complete);
        assert_eq!(label, "Deploy completed successfully");
    }

    // ── RollingBack + script constant tests (Phase 20-deploy-resilience Plan 01) ──

    #[test]
    fn deploy_step_label_rolling_back() {
        let label = deploy_step_label(&DeployState::RollingBack);
        assert_eq!(label, "Rolling back to previous binary");
    }

    #[test]
    fn swap_script_preserves_prev() {
        assert!(
            SWAP_SCRIPT_CONTENT.contains("rc-agent-prev.exe"),
            "SWAP_SCRIPT_CONTENT must reference rc-agent-prev.exe"
        );
    }

    #[test]
    fn swap_script_crlf() {
        assert!(
            SWAP_SCRIPT_CONTENT.contains("\r\n"),
            "SWAP_SCRIPT_CONTENT must use CRLF line endings for Windows batch files"
        );
    }

    #[test]
    fn swap_script_has_av_retry() {
        assert!(
            SWAP_SCRIPT_CONTENT.contains(":RETRY"),
            "SWAP_SCRIPT_CONTENT must have a :RETRY label for AV retry loop"
        );
        assert!(
            SWAP_SCRIPT_CONTENT.contains("LSS 5"),
            "SWAP_SCRIPT_CONTENT must limit AV retries with LSS 5"
        );
    }

    #[test]
    fn swap_script_preserves_move() {
        assert!(
            SWAP_SCRIPT_CONTENT.contains("move /Y rc-agent.exe rc-agent-prev.exe"),
            "SWAP_SCRIPT_CONTENT must move current binary to rc-agent-prev.exe before swap"
        );
    }

    #[test]
    fn rollback_script_contains_prev() {
        assert!(
            ROLLBACK_SCRIPT_CONTENT.contains("rc-agent-prev.exe"),
            "ROLLBACK_SCRIPT_CONTENT must reference rc-agent-prev.exe"
        );
    }

    #[test]
    fn rollback_script_crlf() {
        assert!(
            ROLLBACK_SCRIPT_CONTENT.contains("\r\n"),
            "ROLLBACK_SCRIPT_CONTENT must use CRLF line endings for Windows batch files"
        );
    }

    #[test]
    fn rollback_script_restores_prev() {
        assert!(
            ROLLBACK_SCRIPT_CONTENT.contains("move /Y rc-agent-prev.exe rc-agent.exe"),
            "ROLLBACK_SCRIPT_CONTENT must restore rc-agent-prev.exe to rc-agent.exe"
        );
    }

    #[test]
    fn rollback_verify_delays_shorter() {
        assert_eq!(
            ROLLBACK_VERIFY_DELAYS.len(),
            3,
            "ROLLBACK_VERIFY_DELAYS must have 3 entries (shorter than deploy's 4)"
        );
        let sum: u64 = ROLLBACK_VERIFY_DELAYS.iter().sum();
        assert_eq!(sum, 50, "ROLLBACK_VERIFY_DELAYS sum must be 50s (5+15+30)");
    }

    // ── generate_pod_config tests ───────────────────────────────────────────

    #[test]
    fn generate_pod_config_contains_correct_pod_number() {
        let config = generate_pod_config(3);
        assert!(config.contains("number = 3"));
        assert!(config.contains("\"Pod 03\""));
    }

    #[test]
    fn generate_pod_config_contains_core_url() {
        let config = generate_pod_config(1);
        assert!(config.contains("ws://192.168.31.23:8080/ws/agent"));
    }

    #[test]
    fn generate_pod_config_contains_games() {
        let config = generate_pod_config(8);
        assert!(config.contains("[games.assetto_corsa]"));
        assert!(config.contains("[games.f1_25]"));
        assert!(config.contains("[ai_debugger]"));
    }
}

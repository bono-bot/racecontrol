//! Deploy executor: single-pod deploy logic (download, size-check, swap, verify, rollback).
use std::sync::Arc;
use std::time::Duration;
use crate::{activity_log::log_pod_activity, event_archive, state::AppState};
use rc_common::types::DeployState;
use super::{
    exec_on_pod, is_cancelled, is_lock_screen_healthy, is_process_alive, is_ws_connected,
    generate_pod_config, parse_file_size_from_dir, send_deploy_failure_alert, set_deploy_state,
    validate_binary_size, POD_AGENT_PORT, ROLLBACK_SCRIPT_CONTENT, ROLLBACK_VERIFY_DELAYS,
    SWAP_SCRIPT_CONTENT, VERIFY_DELAYS,
};

/// Deploy rc-agent to a single pod using self-swap pattern.
/// Runs as a tokio::spawn'd background task. Steps: validate URL, download,
/// size-check, write config, trigger self-swap, verify health.
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
        None,
    );
    event_archive::append_event(&state.db, "deploy.started", "deploy", Some(&pod_id), serde_json::json!({
        "binary_url": binary_url,
    }), &state.config.venue.venue_id);

    // Hash-based versioning: download as rc-agent-<hash>.exe
    // The bat file swap logic finds any rc-agent-????????*.exe and renames it
    let build_hash: &str = env!("GIT_HASH");
    let staged_name = format!("rc-agent-{}.exe", build_hash);

    // Clean any stale staging binary first
    let clean_cmd = format!("del /F C:\\RacingPoint\\{}", staged_name);
    let _ = exec_on_pod(&state, &pod_id, &pod_ip, &clean_cmd, 5000).await;

    let download_cmd = format!(
        "curl.exe -s -f -o C:\\RacingPoint\\{} {}",
        staged_name, binary_url
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
                log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy", None);
                event_archive::append_event(&state.db, "deploy.failed", "deploy", Some(&pod_id), serde_json::json!({ "reason": reason }), &state.config.venue.venue_id);
                return;
            }
        }
        Err(e) => {
            let reason = format!("Download command failed: {}", e);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy", None);
            event_archive::append_event(&state.db, "deploy.failed", "deploy", Some(&pod_id), serde_json::json!({ "reason": reason }), &state.config.venue.venue_id);
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
        &format!("dir C:\\RacingPoint\\{}", staged_name),
        5000,
    )
    .await;
    match dir_result {
        Ok((_, stdout, _)) => match parse_file_size_from_dir(&stdout, &staged_name) {
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
                        None,
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
                log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy", None);
                return;
            }
        },
        Err(e) => {
            let reason = format!("Dir command failed: {}", e);
            set_deploy_state(&state, &pod_id, DeployState::Failed { reason: reason.clone() }).await;
            send_deploy_failure_alert(&state, &pod_id, &reason).await;
            log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy", None);
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

    if (1..=8).contains(&pod_number) {
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
                None,
            );
            event_archive::append_event(&state.db, "deploy.completed", "deploy", Some(&pod_id), serde_json::json!({
                "verify_delay_secs": delay,
            }), &state.config.venue.venue_id);
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
            log_pod_activity(&state, &pod_id, "deploy", "Deploy Failed", &reason, "deploy", None);
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
                None,
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
                None,
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
            None,
        );
        event_archive::append_event(&state.db, "deploy.failed", "deploy", Some(&pod_id), serde_json::json!({ "reason": failure_reason }), &state.config.venue.venue_id);
    }
}

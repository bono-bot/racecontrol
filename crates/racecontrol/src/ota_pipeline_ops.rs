//! OTA Pipeline — async operations: sentinel files, kill switches, rollback, billing gates.

use serde_json;

/// Sentinel file path on each pod — prevents recovery systems from fighting the OTA.
pub(crate) const OTA_SENTINEL_PATH: &str = r"C:\RacingPoint\ota-in-progress.flag";
pub(crate) const OTA_SENTINEL_CONTENT: &str = "ota_pipeline_in_progress\n";
/// rc-sentry reads this file at each watchdog tick to decide whether to restart rc-agent.
pub(crate) const SENTRY_FLAGS_PATH: &str = r"C:\RacingPoint\sentry-flags.json";

/// Session wait timeout — how long to wait for active billing sessions to end.
#[allow(dead_code)]
pub(crate) const SESSION_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Write OTA sentinel to each pod via rc-agent /write endpoint.
/// Called at pipeline start to prevent WoL and watchdog from fighting the deploy.
pub async fn set_ota_sentinel(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)], // (pod_id, ip)
) {
    for (pod_id, ip) in pod_ips {
        let url = format!("http://{ip}:8090/write");
        let result = http_client
            .post(&url)
            .json(&serde_json::json!({
                "path": OTA_SENTINEL_PATH,
                "content": OTA_SENTINEL_CONTENT,
            }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        match result {
            Ok(_) => tracing::debug!("OTA sentinel set on {pod_id}"),
            Err(e) => tracing::warn!("Failed to set OTA sentinel on {pod_id}: {e}"),
        }
    }
}

/// Remove OTA sentinel from each pod via rc-agent /exec endpoint.
/// Called at pipeline end (success or failure).
pub async fn clear_ota_sentinel(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)],
) {
    for (pod_id, ip) in pod_ips {
        let url = format!("http://{ip}:8090/exec");
        let cmd = format!(r#"del /Q "{OTA_SENTINEL_PATH}""#);
        let result = http_client
            .post(&url)
            .json(&serde_json::json!({ "cmd": cmd, "timeout_ms": 5000 }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
        match result {
            Ok(_) => tracing::debug!("OTA sentinel cleared on {pod_id}"),
            Err(e) => tracing::warn!("Failed to clear OTA sentinel on {pod_id}: {e}"),
        }
    }
}

/// Set/clear kill_watchdog_restart flag on all connected pods via rc-agent /write.
/// When active=true, rc-sentry's watchdog skips rc-agent restart attempts
/// during the deploy window.
pub async fn set_kill_switch(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)],
    active: bool,
) {
    let flags_json = serde_json::json!({
        "kill_switches": {
            "kill_watchdog_restart": active
        }
    })
    .to_string();

    for (pod_id, ip) in pod_ips {
        let url = format!("http://{ip}:8090/write");
        let result = http_client
            .post(&url)
            .json(&serde_json::json!({
                "path": SENTRY_FLAGS_PATH,
                "content": flags_json,
            }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        match result {
            Ok(_) => tracing::debug!("kill_watchdog_restart={active} written to {pod_id}"),
            Err(e) => tracing::warn!("Failed to write sentry-flags.json to {pod_id}: {e}"),
        }
    }
}

// ── Rollback (OTA-04, OTA-07) ──────────────────────────────────────────────

/// Rollback a wave of pods to rc-agent-prev.exe.
///
/// CRITICAL: Executes rollback via rc-sentry :8091/exec, NOT rc-agent :8090/exec.
/// The rollback bat runs `taskkill /F /IM rc-agent.exe` — executing via rc-agent
/// would kill the process serving the exec endpoint. rc-sentry is a separate binary
/// that survives the kill. (Standing rule: "NEVER use taskkill /F /IM rc-agent.exe
/// followed by start in the same exec chain [via rc-agent].")
pub async fn rollback_wave(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)], // (pod_id, ip)
    sentry_service_key: Option<&str>,
) {
    for (pod_id, ip) in pod_ips {
        tracing::warn!("OTA: Rolling back {pod_id}");

        // Step 1: Write do-rollback.bat via rc-agent /write (agent still alive)
        let write_url = format!("http://{ip}:8090/write");
        let write_result = http_client
            .post(&write_url)
            .json(&serde_json::json!({
                "path": r"C:\RacingPoint\do-rollback.bat",
                "content": crate::deploy::ROLLBACK_SCRIPT_CONTENT,
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        if write_result.is_err() {
            tracing::error!("OTA: Failed to write rollback script to {pod_id}");
            continue;
        }

        // Step 2: Execute rollback via rc-SENTRY :8091/exec (NOT rc-agent :8090)
        let exec_url = format!("http://{ip}:8091/exec");
        let mut req = http_client
            .post(&exec_url);
        if let Some(key) = sentry_service_key {
            req = req.header("X-Service-Key", key);
        }
        let _ = req
            .json(&serde_json::json!({
                "cmd": r#"start /min cmd /c C:\RacingPoint\do-rollback.bat"#,
                "timeout_ms": 5000,
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        tracing::info!("OTA: Rollback triggered for {pod_id} via rc-sentry :8091");
    }

    // Wait for rollback to complete (same delay pattern as deploy.rs)
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
}

/// Check if a pod has an active billing session that should defer its deploy.
pub fn has_active_billing_session(billing_session_id: &Option<String>) -> bool {
    billing_session_id.is_some()
}

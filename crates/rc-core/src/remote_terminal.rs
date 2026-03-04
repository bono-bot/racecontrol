//! Remote terminal: poll cloud for pending commands, execute locally, post results back.
//!
//! Runs as a background task on the local instance.
//! Polls GET /terminal/commands/pending on the cloud every 5 seconds.
//! Executes each command via tokio::process::Command with timeout.
//! Posts results back to POST /terminal/commands/{id}/result.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;
use tokio::time::timeout;

use crate::state::AppState;

const POLL_INTERVAL_SECS: u64 = 5;
const MAX_OUTPUT_BYTES: usize = 100 * 1024; // 100KB max per stdout/stderr
const MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Deserialize)]
struct PendingCommand {
    id: String,
    cmd: String,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct PendingResponse {
    commands: Option<Vec<PendingCommand>>,
}

/// Spawn the remote terminal background task.
/// Only starts if cloud.enabled = true and cloud.api_url is set.
pub fn spawn(state: Arc<AppState>) {
    let cloud = &state.config.cloud;
    if !cloud.enabled {
        return;
    }

    let api_url = match &cloud.api_url {
        Some(url) => url.clone(),
        None => return,
    };

    let secret = cloud.terminal_secret.clone().unwrap_or_default();

    tracing::info!("Remote terminal enabled: polling {}", api_url);

    tokio::spawn(async move {
        // Wait 10s on startup before first poll
        tokio::time::sleep(Duration::from_secs(10)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
        loop {
            interval.tick().await;
            if let Err(e) = poll_and_execute(&state, &api_url, &secret).await {
                tracing::debug!("Remote terminal poll: {}", e);
            }
        }
    });
}

async fn poll_and_execute(
    state: &Arc<AppState>,
    cloud_url: &str,
    secret: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/terminal/commands/pending", cloud_url);

    let resp = state
        .http_client
        .get(&url)
        .header("x-terminal-secret", secret)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Cloud returned status {}", resp.status());
    }

    let body: PendingResponse = resp.json().await?;

    let commands = match body.commands {
        Some(cmds) if !cmds.is_empty() => cmds,
        _ => return Ok(()),
    };

    for cmd in commands {
        tracing::info!("Executing terminal command {}: {}", cmd.id, cmd.cmd);

        // Mark as running
        let _ = state
            .http_client
            .post(&format!("{}/terminal/commands/{}/result", cloud_url, cmd.id))
            .header("x-terminal-secret", secret)
            .json(&json!({
                "exit_code": null,
                "stdout": null,
                "stderr": "Running...",
            }))
            .send()
            .await;

        // Update status to running locally (cloud side handles it in the result post)
        let _ = sqlx::query(
            "UPDATE terminal_commands SET status = 'running', started_at = datetime('now') WHERE id = ?",
        )
        .bind(&cmd.id)
        .execute(&state.db)
        .await;

        let timeout_ms = cmd.timeout_ms.unwrap_or(30_000).min(MAX_TIMEOUT_MS);

        let result = timeout(Duration::from_millis(timeout_ms), async {
            Command::new("cmd")
                .args(["/C", &cmd.cmd])
                .kill_on_drop(true)
                .output()
                .await
        })
        .await;

        let (exit_code, stdout, stderr) = match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                (
                    output.status.code(),
                    truncate_output(stdout),
                    truncate_output(stderr),
                )
            }
            Ok(Err(e)) => (None, String::new(), format!("Failed to execute: {}", e)),
            Err(_) => (
                Some(124),
                String::new(),
                format!("Command timed out after {}ms", timeout_ms),
            ),
        };

        tracing::info!(
            "Terminal command {} finished (exit_code: {:?})",
            cmd.id,
            exit_code
        );

        // Post result back to cloud
        let post_result = state
            .http_client
            .post(&format!("{}/terminal/commands/{}/result", cloud_url, cmd.id))
            .header("x-terminal-secret", secret)
            .json(&json!({
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
            }))
            .timeout(Duration::from_secs(15))
            .send()
            .await;

        if let Err(e) = post_result {
            tracing::error!("Failed to post terminal result for {}: {}", cmd.id, e);
        }
    }

    Ok(())
}

fn truncate_output(s: String) -> String {
    if s.len() > MAX_OUTPUT_BYTES {
        let mut truncated = s[..MAX_OUTPUT_BYTES].to_string();
        truncated.push_str("\n... [output truncated at 100KB]");
        truncated
    } else {
        s
    }
}

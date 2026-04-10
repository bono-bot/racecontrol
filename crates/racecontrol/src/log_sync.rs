//! Log sync: rsyncs JSONL log files to Bono VPS for disaster recovery.
//! Runs as a background tokio task. Syncs once daily during IST 02:00-04:00 window.
//! Adapted from event_archive.rs SCP pattern (same SSH options, same remote host).

use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

use chrono::Timelike;

use crate::state::AppState;

const LOG_TARGET: &str = "log_sync";
const REMOTE_HOST: &str = "root@100.70.177.44"; // Bono VPS via Tailscale
const REMOTE_PATH: &str = "/root/backups/venue-logs";
const LOG_DIR: &str = "logs";
const SYNC_INTERVAL_SECS: u64 = 3600; // Check every hour, sync in IST 02:00-04:00 window

/// Spawn the log sync background task.
pub fn spawn(_state: Arc<AppState>) {
    tokio::spawn(log_sync_task());
    tracing::info!(target: LOG_TARGET, "log_sync task spawned (hourly check, syncs in IST 02:00-04:00)");
}

async fn log_sync_task() {
    let mut interval = tokio::time::interval(Duration::from_secs(SYNC_INTERVAL_SECS));
    let mut last_sync_date: Option<String> = None;

    loop {
        interval.tick().await;

        // Check if we're in the IST 02:00-04:00 sync window
        let now_ist = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata);
        let hour = now_ist.hour();
        let today = now_ist.format("%Y-%m-%d").to_string();

        if hour < 2 || hour >= 4 {
            continue; // Outside sync window
        }

        // Only sync once per day
        if last_sync_date.as_deref() == Some(&today) {
            continue;
        }

        tracing::info!(target: LOG_TARGET, "Starting daily log sync to Bono VPS");

        match sync_logs_to_remote().await {
            Ok(count) => {
                tracing::info!(target: LOG_TARGET, files_synced = count, "Log sync complete");
                last_sync_date = Some(today);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, error = %e, "Log sync failed");
                // Don't update last_sync_date — retry next hour
            }
        }
    }
}

async fn sync_logs_to_remote() -> Result<usize, String> {
    // Ensure remote directory exists
    let mkdir_output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new("ssh")
            .arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("BatchMode=yes")
            .arg("-o").arg("ConnectTimeout=10")
            .arg(REMOTE_HOST)
            .arg(format!("mkdir -p {}", REMOTE_PATH))
            .output(),
    )
    .await
    .map_err(|_| "SSH mkdir timeout".to_string())?
    .map_err(|e| format!("SSH mkdir failed: {}", e))?;

    if !mkdir_output.status.success() {
        return Err(format!(
            "SSH mkdir failed: {}",
            String::from_utf8_lossy(&mkdir_output.stderr)
        ));
    }

    // Find JSONL files in logs directory
    let log_dir = std::path::Path::new(LOG_DIR);
    if !log_dir.exists() {
        return Err("logs/ directory does not exist".to_string());
    }

    let mut count = 0;
    let entries = std::fs::read_dir(log_dir)
        .map_err(|e| format!("Failed to read logs dir: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "Invalid filename".to_string())?;

        let remote_dest = format!("{}:{}/{}", REMOTE_HOST, REMOTE_PATH, filename);

        tracing::debug!(target: LOG_TARGET, file = %filename, "Syncing log file");

        let scp_output = tokio::time::timeout(
            Duration::from_secs(120),
            Command::new("scp")
                .arg("-o").arg("StrictHostKeyChecking=no")
                .arg("-o").arg("BatchMode=yes")
                .arg("-o").arg("ConnectTimeout=10")
                .arg(path.to_str().unwrap_or(""))
                .arg(&remote_dest)
                .output(),
        )
        .await
        .map_err(|_| format!("SCP timeout for {}", filename))?
        .map_err(|e| format!("SCP failed for {}: {}", filename, e))?;

        if scp_output.status.success() {
            count += 1;
        } else {
            tracing::warn!(
                target: LOG_TARGET,
                file = %filename,
                stderr = %String::from_utf8_lossy(&scp_output.stderr),
                "SCP failed for log file"
            );
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(REMOTE_HOST, "root@100.70.177.44");
        assert_eq!(REMOTE_PATH, "/root/backups/venue-logs");
        assert_eq!(LOG_DIR, "logs");
    }
}

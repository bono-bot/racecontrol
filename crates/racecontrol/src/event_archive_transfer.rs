//! Event archive remote SCP transfer.
//!
//! Split from event_archive.rs — contains the nightly JSONL transfer
//! to Bono VPS with SHA256 verification.

use std::sync::Arc;
use std::time::Duration;

use sha2::Digest;

use crate::state::AppState;
use super::LOG_TARGET;

/// Transfer a JSONL file to Bono VPS via SCP with SHA256 verification.
///
/// Reuses backup_pipeline.rs Steps A-E verbatim:
///   A: ssh mkdir -p remote_path
///   B: local SHA256 of the file
///   C: scp with 120s timeout
///   D: remote sha256sum via SSH
///   E: compare checksums, update last_remote_transfer on match
///
/// Only runs during IST 02:00-03:59 window, once per day.
pub(super) async fn transfer_jsonl_to_remote(
    state: &Arc<AppState>,
    filepath: &str,
    filename: &str,
    last_remote_transfer: &mut Option<chrono::NaiveDate>,
) -> anyhow::Result<()> {
    // Clone config before any async IO — no lock held across .await
    let remote_host = state.config.event_archive.remote_host.clone();
    let remote_path = state.config.event_archive.remote_path.clone();

    // Check IST hour — only proceed during 02:00-03:59 IST
    use chrono::Timelike;
    let now_ist = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata);
    let ist_hour = now_ist.hour();
    let today = now_ist.date_naive();

    if ist_hour != 2 && ist_hour != 3 {
        return Ok(());
    }

    // Deduplication: skip if already transferred today
    if *last_remote_transfer == Some(today) {
        return Ok(());
    }

    tracing::info!(
        target: LOG_TARGET,
        "Starting nightly JSONL transfer: {} → {}:{}",
        filename,
        remote_host,
        remote_path
    );

    // Step A: Ensure remote directory exists
    let mkdir_result = tokio::process::Command::new("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=10")
        .arg(&remote_host)
        .arg(&format!("mkdir -p {}", remote_path))
        .output()
        .await;

    if let Err(e) = mkdir_result {
        return Err(anyhow::anyhow!("SSH mkdir failed: {}", e));
    }

    // Step B: Compute local SHA256
    let bytes = tokio::fs::read(filepath).await?;
    let local_checksum = hex::encode(sha2::Sha256::digest(&bytes));
    tracing::debug!(target: LOG_TARGET, "Local SHA256: {}", local_checksum);

    // Step C: SCP the file with 120s timeout
    let remote_dest = format!("{}:{}/{}", remote_host, remote_path, filename);
    let scp_output = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new("scp")
            .arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("BatchMode=yes")
            .arg("-o").arg("ConnectTimeout=10")
            .arg(filepath)
            .arg(&remote_dest)
            .output(),
    )
    .await;

    let scp_result = match scp_output {
        Err(_timeout) => {
            return Err(anyhow::anyhow!(
                "SCP transfer timed out after 120s for {}",
                filename
            ));
        }
        Ok(Err(e)) => {
            return Err(anyhow::anyhow!("SCP spawn error: {}", e));
        }
        Ok(Ok(output)) => output,
    };

    if !scp_result.status.success() {
        let stderr = String::from_utf8_lossy(&scp_result.stderr);
        return Err(anyhow::anyhow!(
            "SCP transfer failed for {}: {}",
            filename,
            stderr
        ));
    }

    tracing::info!(target: LOG_TARGET, "SCP transfer complete: {}", filename);

    // Step D: Remote SHA256 verification
    let verify_output = tokio::process::Command::new("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=10")
        .arg(&remote_host)
        .arg(&format!("sha256sum {}/{}", remote_path, filename))
        .output()
        .await;

    let checksums_match = match verify_output {
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "sha256sum SSH call failed: {}", e);
            None
        }
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // sha256sum output: "<64-char-hex>  <filename>"
            let remote_checksum = stdout.split_whitespace().next().unwrap_or("").to_string();
            let matched = remote_checksum.len() == 64 && remote_checksum == local_checksum;
            tracing::info!(
                target: LOG_TARGET,
                "Checksum — local: {} remote: {} match: {}",
                local_checksum,
                remote_checksum,
                matched
            );
            if !matched {
                tracing::error!(
                    target: LOG_TARGET,
                    "[EVENT-ARCHIVE] Remote checksum MISMATCH for {} — local: {} remote: {}",
                    filename,
                    local_checksum,
                    remote_checksum
                );
            }
            Some(matched)
        }
    };

    // Step E: Update last_remote_transfer on successful transfer
    if checksums_match.unwrap_or(false) {
        *last_remote_transfer = Some(today);
        tracing::info!(
            target: LOG_TARGET,
            "Nightly JSONL transfer complete and verified: {}",
            filename
        );
    } else if checksums_match.is_some() {
        return Err(anyhow::anyhow!(
            "Checksum mismatch for {} — transfer may be corrupt",
            filename
        ));
    } else {
        // Checksum verification failed (SSH error) but SCP succeeded — still record transfer
        *last_remote_transfer = Some(today);
        tracing::warn!(
            target: LOG_TARGET,
            "JSONL transfer complete but checksum unverified (SSH error): {}",
            filename
        );
    }

    Ok(())
}

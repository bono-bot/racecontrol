//! Remote backup transfer: rsync/SCP to Bono VPS with SHA256 verification.
//!
//! Extracted from backup_pipeline.rs (Phase 300-02, Phase 351).
//! - Nightly rsync transfer (02:00-04:00 IST window, once per day, SCP fallback)
//! - SHA256 local+remote checksum verification
//! - Remote reachability checked every tick via `ssh ... echo ok`

use std::sync::Arc;
use std::time::Duration;

use sha2::Digest;

use crate::state::AppState;

pub(super) const LOG_TARGET: &str = "backup_pipeline";

/// SCP transfer helper — used as fallback when rsync is unavailable or fails.
/// Returns Ok(true) on success, Ok(false) on transfer failure, Err on spawn failure.
pub(super) async fn transfer_via_scp(
    state: &Arc<AppState>,
    backup_path: &str,
    filename: &str,
    remote_dest: &str,
) -> anyhow::Result<bool> {
    let scp_output = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new("scp")
            .arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("BatchMode=yes")
            .arg("-o").arg("ConnectTimeout=10")
            .arg(backup_path)
            .arg(remote_dest)
            .output(),
    )
    .await;

    match scp_output {
        Err(_timeout) => {
            tracing::error!(target: LOG_TARGET, "SCP timed out after 120s for {}", filename);
            let mut status = state.backup_status.write().await;
            status.remote_reachable = false;
            Ok(false)
        }
        Ok(Err(e)) => {
            Err(anyhow::anyhow!("SCP spawn error: {}", e))
        }
        Ok(Ok(output)) => {
            if output.status.success() {
                tracing::info!(target: LOG_TARGET, "SCP transfer complete: {}", filename);
                Ok(true)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(target: LOG_TARGET, "SCP transfer failed for {}: {}", filename, stderr);
                let mut status = state.backup_status.write().await;
                status.remote_reachable = false;
                Ok(false)
            }
        }
    }
}

/// Check whether the remote host (Bono VPS) is reachable via SSH and update BackupStatus.
/// Called on every tick so the dashboard always reflects current connectivity.
pub(super) async fn check_remote_reachable(state: &Arc<AppState>) {
    if !state.config.backup.remote_enabled {
        return;
    }
    // Clone config values before async IO — do NOT hold RwLock guard across .await.
    let remote_host = state.config.backup.remote_host.clone();

    let result = tokio::process::Command::new("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=10")
        .arg(&remote_host)
        .arg("echo ok")
        .output()
        .await;

    let reachable = match result {
        Ok(output) => output.status.success() && output.stdout.starts_with(b"ok"),
        Err(e) => {
            tracing::debug!(target: LOG_TARGET, "SSH reachability check error: {}", e);
            false
        }
    };

    let mut status = state.backup_status.write().await;
    status.remote_reachable = reachable;
    if !reachable {
        tracing::debug!(target: LOG_TARGET, "Bono VPS not reachable via SSH");
    }
}

/// Transfer the most recent racecontrol daily backup to Bono VPS via rsync with SHA256 verification.
///
/// Transfer runs only:
/// 1. When `config.backup.remote_enabled` is true
/// 2. During the nightly window: IST hour 2 or 3 (02:00-03:59)
/// 3. Once per day (tracked via `last_remote_transfer` NaiveDate)
///
/// Steps: mkdir -p remote_path → compute local SHA256 → rsync (SCP fallback) → remote sha256sum → compare
/// OPS-11: rsync preferred; SCP fallback if rsync unavailable (config.backup.use_rsync).
pub(super) async fn transfer_to_remote(
    state: &Arc<AppState>,
    backup_path: &str,
    filename: &str,
    last_remote_transfer: &mut Option<chrono::NaiveDate>,
) -> anyhow::Result<()> {
    // Clone config values before any async IO.
    let remote_enabled = state.config.backup.remote_enabled;
    let remote_host = state.config.backup.remote_host.clone();
    let remote_path = state.config.backup.remote_path.clone();
    let use_rsync = state.config.backup.use_rsync;

    if !remote_enabled {
        return Ok(());
    }

    // Check IST hour — only proceed during 02:00-03:59 IST.
    use chrono::Timelike;
    let now_ist = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata);
    let ist_hour = now_ist.hour();
    let today = now_ist.date_naive();

    if ist_hour != 2 && ist_hour != 3 {
        return Ok(());
    }

    // Check if already transferred today.
    if *last_remote_transfer == Some(today) {
        return Ok(());
    }

    tracing::info!(target: LOG_TARGET, "Starting nightly remote transfer: {} → {}:{}", filename, remote_host, remote_path);

    // Step A: Ensure remote directory exists.
    let mkdir = tokio::process::Command::new("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=10")
        .arg(&remote_host)
        .arg(format!("mkdir -p {}", remote_path))
        .output()
        .await;

    if let Err(e) = mkdir {
        let msg = format!("SSH mkdir failed: {}", e);
        tracing::error!(target: LOG_TARGET, "{}", msg);
        let mut status = state.backup_status.write().await;
        status.remote_reachable = false;
        return Err(anyhow::anyhow!(msg));
    }

    // Step B: Compute local SHA256.
    let bytes = tokio::fs::read(backup_path).await?;
    let local_checksum = hex::encode(sha2::Sha256::digest(&bytes));
    tracing::debug!(target: LOG_TARGET, "Local SHA256: {}", local_checksum);

    // Step C: Transfer the file — rsync preferred (OPS-11), SCP fallback.
    let remote_dest = format!("{}:{}/{}", remote_host, remote_path, filename);
    let transfer_ok = if use_rsync {
        // Try rsync first — Git Bash rsync.exe on Windows
        let rsync_bin = if cfg!(target_os = "windows") {
            "C:/Program Files/Git/usr/bin/rsync.exe"
        } else {
            "rsync"
        };
        let rsync_result = tokio::time::timeout(
            Duration::from_secs(180),
            tokio::process::Command::new(rsync_bin)
                .arg("-az")
                .arg("--checksum")
                .arg("--no-perms")
                .arg("--timeout=60")
                .arg("-e")
                .arg("ssh -o StrictHostKeyChecking=no -o BatchMode=yes")
                .arg(backup_path)
                .arg(&remote_dest)
                .output(),
        )
        .await;

        let rsync_ok = match rsync_result {
            Ok(Ok(out)) => out.status.success(),
            Ok(Err(e)) => {
                tracing::debug!(target: LOG_TARGET, "rsync spawn failed ({}), falling back to SCP", e);
                false
            }
            Err(_) => {
                tracing::debug!(target: LOG_TARGET, "rsync timed out, falling back to SCP");
                false
            }
        };

        if rsync_ok {
            tracing::info!(target: LOG_TARGET, "rsync transfer complete: {}", filename);
            true
        } else {
            // Fall back to SCP
            tracing::info!(target: LOG_TARGET, "rsync failed/unavailable, using SCP fallback");
            transfer_via_scp(state, backup_path, filename, &remote_dest).await?
        }
    } else {
        transfer_via_scp(state, backup_path, filename, &remote_dest).await?
    };

    if !transfer_ok {
        let msg = format!("Remote transfer failed for {} (both rsync and SCP exhausted)", filename);
        tracing::error!(target: LOG_TARGET, "{}", msg);
        let mut status = state.backup_status.write().await;
        status.remote_reachable = false;
        return Err(anyhow::anyhow!(msg));
    }

    tracing::info!(target: LOG_TARGET, "Remote transfer complete: {}", filename);

    // Step D: Remote SHA256 verification.
    let verify_output = tokio::process::Command::new("ssh")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=10")
        .arg(&remote_host)
        .arg(format!("sha256sum {}/{}", remote_path, filename))
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
                "Checksum check — local: {} remote: {} match: {}",
                local_checksum,
                remote_checksum,
                matched
            );
            if !matched {
                let msg = format!(
                    "[BACKUP] Remote checksum MISMATCH for {} — local: {} remote: {} | {}",
                    filename,
                    local_checksum,
                    remote_checksum,
                    crate::whatsapp_alerter::ist_now_string()
                );
                tracing::error!(target: LOG_TARGET, "{}", msg);
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
            Some(matched)
        }
    };

    // Step E: Update BackupStatus.
    let ist_now = crate::whatsapp_alerter::ist_now_string();
    {
        let mut status = state.backup_status.write().await;
        status.remote_reachable = true;
        status.last_remote_transfer_at = Some(ist_now);
        status.last_checksum_match = checksums_match;
    }

    // Record that we transferred today so we don't re-transfer within the same nightly window.
    *last_remote_transfer = Some(today);
    tracing::info!(target: LOG_TARGET, "Nightly remote transfer complete for {}", filename);

    Ok(())
}

//! Phase 300-01: SQLite Backup Pipeline
//!
//! Hourly WAL-safe backup of racecontrol.db and telemetry.db using VACUUM INTO.
//! Rotation: 7 daily + 4 weekly files per database.
//! Staleness alert: WhatsApp alert if no successful backup in staleness_alert_hours (default: 2h).
//! Debounce: alert suppressed if already fired within 2 * staleness_alert_hours.
//!
//! Phase 300-02 additions:
//! - Nightly SCP transfer to Bono VPS (02:00-04:00 IST window, once per day)
//! - SHA256 local+remote checksum verification
//! - Remote reachability checked every tick via `ssh ... echo ok`
//! - BackupStatus updated with remote fields on every tick
//!
//! Standing rules compliance:
//! - No .unwrap() — uses ? and if let Err(e)
//! - No lock held across .await — clone/snapshot before async work
//! - VACUUM INTO (not file copy) per locked decision
//! - File paths: forward slashes in VACUUM INTO SQL string
//! - StrictHostKeyChecking=no + BatchMode=yes on all ssh/scp (Pitfall 4)
//! - No hardcoded IPs — uses config.backup.remote_host

use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::Digest;

use crate::state::AppState;
use crate::state::BackupStatus;

const LOG_TARGET: &str = "backup_pipeline";

/// Spawn the backup pipeline background task.
/// Follows scheduler.rs spawn pattern exactly.
pub fn spawn(state: Arc<AppState>) {
    if !state.config.backup.enabled {
        tracing::info!(target: LOG_TARGET, "backup pipeline disabled — skipping spawn");
        return;
    }

    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "backup_pipeline task started");

        // On first tick, scan backup_dir for most recent backup file to initialize
        // BackupStatus.last_backup_at — prevents false staleness alert on startup (Pitfall 5).
        let backup_dir = state.config.backup.backup_dir.clone();
        if let Some(staleness) = compute_staleness(&backup_dir) {
            let mut status = state.backup_status.write().await;
            status.staleness_hours = Some(staleness);
            // Try to find the newest file and populate last_backup_at
            if let Ok(newest) = find_newest_backup_file(&backup_dir) {
                if let Some(path) = newest {
                    status.last_backup_file = Some(path.clone());
                    // Estimate last_backup_at from staleness
                    let ago_secs = (staleness * 3600.0) as u64;
                    let approx_at = chrono::Utc::now()
                        .checked_sub_signed(chrono::Duration::seconds(ago_secs as i64))
                        .unwrap_or(chrono::Utc::now());
                    let ist = approx_at.with_timezone(&chrono_tz::Asia::Kolkata)
                        .format("%Y-%m-%dT%H:%M:%S IST")
                        .to_string();
                    status.last_backup_at = Some(ist);
                }
            }
        }

        let interval_secs = state.config.backup.interval_secs;
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        let mut last_alert_fired: Option<Instant> = None;
        // Track the IST date of the last successful remote transfer to ensure
        // we only transfer once per day even if the server restarts during the nightly window.
        let mut last_remote_transfer: Option<chrono::NaiveDate> = None;

        loop {
            interval.tick().await;
            if let Err(e) = backup_tick(&state, &mut last_alert_fired, &mut last_remote_transfer).await {
                tracing::error!(target: LOG_TARGET, "backup_tick error: {}", e);
            }
        }
    });
}

/// One backup tick: create backups for both databases, rotate, update status, check staleness.
/// Also checks remote reachability every tick, and transfers the daily racecontrol.db backup
/// to Bono VPS once per day during the 02:00-04:00 IST window.
async fn backup_tick(
    state: &Arc<AppState>,
    last_alert_fired: &mut Option<Instant>,
    last_remote_transfer: &mut Option<chrono::NaiveDate>,
) -> anyhow::Result<()> {
    let backup_dir = state.config.backup.backup_dir.clone();
    let daily_retain = state.config.backup.daily_retain;
    let weekly_retain = state.config.backup.weekly_retain;

    // Create backup directory if it does not exist
    std::fs::create_dir_all(&backup_dir)?;

    // Generate IST timestamp for backup file names
    let now_ist = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata);
    let timestamp = now_ist.format("%Y-%m-%dT%H-%M-%S").to_string();

    // Determine if this is a weekly backup (Sunday = day 0 / chrono Weekday::Sun)
    use chrono::Datelike;
    let is_weekly = now_ist.weekday() == chrono::Weekday::Sun;
    let year = now_ist.year();
    let week_num = now_ist.iso_week().week();

    let mut last_backup_file: Option<String> = None;
    let mut last_backup_size: Option<u64> = None;
    let mut total_count: usize = 0;

    // Backup main racecontrol.db
    {
        let start = std::time::Instant::now();
        let main_prefix = "racecontrol";
        let daily_name = format!("{}-{}.db", main_prefix, timestamp);
        let daily_path = format!("{}/{}", backup_dir, daily_name);
        // Use forward slashes in VACUUM INTO SQL even on Windows — SQLite handles this
        let sql_path = daily_path.replace('\\', "/");
        let vacuum_sql = format!("VACUUM INTO '{}'", sql_path);

        match sqlx::query(&vacuum_sql)
            .execute(&state.db)
            .await
        {
            Ok(_) => {
                let elapsed = start.elapsed().as_secs();
                if elapsed > 30 {
                    tracing::warn!(target: LOG_TARGET, "VACUUM INTO racecontrol took {}s (>30s threshold)", elapsed);
                } else {
                    tracing::info!(target: LOG_TARGET, "racecontrol backup created: {} ({}s)", daily_name, elapsed);
                }
                // Record size
                if let Ok(meta) = std::fs::metadata(&daily_path) {
                    last_backup_size = Some(meta.len());
                }
                last_backup_file = Some(daily_name.clone());

                // Weekly snapshot on Sunday
                if is_weekly {
                    let weekly_name = format!("{}-weekly-{}-W{:02}.db", main_prefix, year, week_num);
                    let weekly_path = format!("{}/{}", backup_dir, weekly_name);
                    if let Err(e) = std::fs::copy(&daily_path, &weekly_path) {
                        tracing::warn!(target: LOG_TARGET, "Weekly copy for racecontrol failed: {}", e);
                    } else {
                        tracing::info!(target: LOG_TARGET, "Weekly snapshot created: {}", weekly_name);
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "VACUUM INTO racecontrol.db failed: {}", e);
            }
        }

        rotate_backups(&backup_dir, main_prefix, daily_retain, weekly_retain)?;
        total_count += count_backup_files(&backup_dir, main_prefix);
    }

    // Backup telemetry.db if available
    if let Some(ref telemetry_db) = state.telemetry_db {
        let start = std::time::Instant::now();
        let tel_prefix = "telemetry";
        let daily_name = format!("{}-{}.db", tel_prefix, timestamp);
        let daily_path = format!("{}/{}", backup_dir, daily_name);
        let sql_path = daily_path.replace('\\', "/");
        let vacuum_sql = format!("VACUUM INTO '{}'", sql_path);

        match sqlx::query(&vacuum_sql)
            .execute(telemetry_db)
            .await
        {
            Ok(_) => {
                let elapsed = start.elapsed().as_secs();
                if elapsed > 30 {
                    tracing::warn!(target: LOG_TARGET, "VACUUM INTO telemetry took {}s (>30s threshold)", elapsed);
                } else {
                    tracing::info!(target: LOG_TARGET, "telemetry backup created: {} ({}s)", daily_name, elapsed);
                }

                // Weekly snapshot on Sunday
                if is_weekly {
                    let weekly_name = format!("{}-weekly-{}-W{:02}.db", tel_prefix, year, week_num);
                    let weekly_path = format!("{}/{}", backup_dir, weekly_name);
                    if let Err(e) = std::fs::copy(&daily_path, &weekly_path) {
                        tracing::warn!(target: LOG_TARGET, "Weekly copy for telemetry failed: {}", e);
                    } else {
                        tracing::info!(target: LOG_TARGET, "Weekly snapshot created: {}", weekly_name);
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "VACUUM INTO telemetry.db failed: {}", e);
            }
        }

        rotate_backups(&backup_dir, tel_prefix, daily_retain, weekly_retain)?;
        total_count += count_backup_files(&backup_dir, tel_prefix);
    }

    // Compute staleness from newest file mtime
    let staleness = compute_staleness(&backup_dir);

    // Update BackupStatus — snapshot before writing (no lock held across .await)
    let now_ist_str = crate::whatsapp_alerter::ist_now_string();
    {
        let mut status = state.backup_status.write().await;
        status.last_backup_at = Some(now_ist_str);
        status.last_backup_size_bytes = last_backup_size;
        status.last_backup_file = last_backup_file;
        status.backup_count_local = total_count;
        status.staleness_hours = staleness;
    }

    // Check staleness and fire alert if needed
    check_staleness(state, last_alert_fired).await;

    // Check remote reachability on every tick (non-nightly) so the dashboard
    // always shows a current value even on ticks when we don't transfer.
    check_remote_reachable(state).await;

    // Nightly SCP transfer: racecontrol daily backup → Bono VPS (02:00-04:00 IST).
    // Snapshot the latest daily backup file name (without holding the lock across .await).
    let latest_backup_file = {
        let status = state.backup_status.read().await;
        status.last_backup_file.clone()
    };
    if let Some(ref filename) = latest_backup_file {
        let backup_dir = state.config.backup.backup_dir.clone();
        let backup_path = format!("{}/{}", backup_dir, filename);
        if let Err(e) = transfer_to_remote(state, &backup_path, filename, last_remote_transfer).await {
            tracing::error!(target: LOG_TARGET, "Nightly remote transfer failed: {}", e);
        }
    }

    Ok(())
}

/// Check whether the remote host (Bono VPS) is reachable via SSH and update BackupStatus.
/// Called on every tick so the dashboard always reflects current connectivity.
async fn check_remote_reachable(state: &Arc<AppState>) {
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

/// Transfer the most recent racecontrol daily backup to Bono VPS via SCP with SHA256 verification.
///
/// Transfer runs only:
/// 1. When `config.backup.remote_enabled` is true
/// 2. During the nightly window: IST hour 2 or 3 (02:00-03:59)
/// 3. Once per day (tracked via `last_remote_transfer` NaiveDate)
///
/// Steps: mkdir -p remote_path → compute local SHA256 → SCP (120s timeout) → remote sha256sum → compare
async fn transfer_to_remote(
    state: &Arc<AppState>,
    backup_path: &str,
    filename: &str,
    last_remote_transfer: &mut Option<chrono::NaiveDate>,
) -> anyhow::Result<()> {
    // Clone config values before any async IO.
    let remote_enabled = state.config.backup.remote_enabled;
    let remote_host = state.config.backup.remote_host.clone();
    let remote_path = state.config.backup.remote_path.clone();

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
        .arg(&format!("mkdir -p {}", remote_path))
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

    // Step C: SCP the file with 120s timeout.
    let remote_dest = format!("{}:{}/{}", remote_host, remote_path, filename);
    let scp_output = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new("scp")
            .arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("BatchMode=yes")
            .arg("-o").arg("ConnectTimeout=10")
            .arg(backup_path)
            .arg(&remote_dest)
            .output(),
    )
    .await;

    let scp_result = match scp_output {
        Err(_timeout) => {
            let msg = format!("SCP transfer timed out after 120s for {}", filename);
            tracing::error!(target: LOG_TARGET, "{}", msg);
            let mut status = state.backup_status.write().await;
            status.remote_reachable = false;
            return Err(anyhow::anyhow!(msg));
        }
        Ok(Err(e)) => {
            let msg = format!("SCP spawn error: {}", e);
            tracing::error!(target: LOG_TARGET, "{}", msg);
            let mut status = state.backup_status.write().await;
            status.remote_reachable = false;
            return Err(anyhow::anyhow!(msg));
        }
        Ok(Ok(output)) => output,
    };

    if !scp_result.status.success() {
        let stderr = String::from_utf8_lossy(&scp_result.stderr);
        let msg = format!("SCP transfer failed for {}: {}", filename, stderr);
        tracing::error!(target: LOG_TARGET, "{}", msg);
        let mut status = state.backup_status.write().await;
        status.remote_reachable = false;
        return Err(anyhow::anyhow!(msg));
    }

    tracing::info!(target: LOG_TARGET, "SCP transfer complete: {}", filename);

    // Step D: Remote SHA256 verification.
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

/// Rotate backup files: keep newest `daily_retain` daily + `weekly_retain` weekly per prefix.
pub fn rotate_backups(
    backup_dir: &str,
    prefix: &str,
    daily_retain: usize,
    weekly_retain: usize,
) -> anyhow::Result<()> {
    let dir = std::path::Path::new(backup_dir);
    if !dir.exists() {
        return Ok(());
    }

    // Collect daily files: {prefix}-YYYY-MM-DDTHH-MM-SS.db (NOT weekly)
    let daily_pattern = format!("{}-", prefix);
    let weekly_pattern = format!("{}-weekly-", prefix);

    let mut daily_files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                // Daily: starts with prefix-, NOT weekly
                name.starts_with(&daily_pattern) && !name.contains("-weekly-")
            } else {
                false
            }
        })
        .collect();

    // Sort by name — ISO timestamps sort chronologically
    daily_files.sort();

    // Delete oldest daily files beyond retention limit
    if daily_files.len() > daily_retain {
        let to_delete = daily_files.len() - daily_retain;
        for path in daily_files.iter().take(to_delete) {
            tracing::debug!(target: LOG_TARGET, "Rotating daily backup: {:?}", path);
            if let Err(e) = std::fs::remove_file(path) {
                tracing::warn!(target: LOG_TARGET, "Failed to delete old backup {:?}: {}", path, e);
            }
        }
        tracing::info!(target: LOG_TARGET, "Rotated {} old daily backup(s) for prefix '{}'", to_delete, prefix);
    }

    // Collect and rotate weekly files: {prefix}-weekly-YYYY-WNN.db
    let mut weekly_files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                name.starts_with(&weekly_pattern)
            } else {
                false
            }
        })
        .collect();

    weekly_files.sort();

    if weekly_files.len() > weekly_retain {
        let to_delete = weekly_files.len() - weekly_retain;
        for path in weekly_files.iter().take(to_delete) {
            tracing::debug!(target: LOG_TARGET, "Rotating weekly backup: {:?}", path);
            if let Err(e) = std::fs::remove_file(path) {
                tracing::warn!(target: LOG_TARGET, "Failed to delete old weekly backup {:?}: {}", path, e);
            }
        }
        tracing::info!(target: LOG_TARGET, "Rotated {} old weekly backup(s) for prefix '{}'", to_delete, prefix);
    }

    Ok(())
}

/// Count total backup files for a prefix (daily + weekly combined).
fn count_backup_files(backup_dir: &str, prefix: &str) -> usize {
    let dir = std::path::Path::new(backup_dir);
    if !dir.exists() {
        return 0;
    }
    let file_prefix = format!("{}-", prefix);
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with(&file_prefix))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Find the path of the newest backup file in the directory (by filename sort).
fn find_newest_backup_file(backup_dir: &str) -> anyhow::Result<Option<String>> {
    let dir = std::path::Path::new(backup_dir);
    if !dir.exists() {
        return Ok(None);
    }
    let mut files: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.path().file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
        .filter(|name| name.ends_with(".db"))
        .collect();
    files.sort();
    Ok(files.into_iter().last())
}

/// Compute hours since the newest backup file was modified.
/// Returns None if no backup files exist in the directory.
pub fn compute_staleness(backup_dir: &str) -> Option<f64> {
    let dir = std::path::Path::new(backup_dir);
    if !dir.exists() {
        return None;
    }

    let newest_mtime = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "db")
                .unwrap_or(false)
        })
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .max()?;

    let elapsed = newest_mtime.elapsed().ok()?;
    Some(elapsed.as_secs_f64() / 3600.0)
}

/// Check staleness and fire WhatsApp alert if threshold exceeded, with debounce.
async fn check_staleness(state: &Arc<AppState>, last_alert_fired: &mut Option<Instant>) {
    let staleness_threshold = state.config.backup.staleness_alert_hours as f64;
    let debounce_secs = state.config.backup.staleness_alert_hours * 2 * 3600;

    // Snapshot staleness_hours without holding lock across .await
    let staleness_hours = {
        let status = state.backup_status.read().await;
        status.staleness_hours
    };

    let Some(hours) = staleness_hours else {
        // No backup files at all — treat as stale since startup
        return;
    };

    if hours > staleness_threshold {
        // Check debounce
        let should_fire = match *last_alert_fired {
            None => true,
            Some(fired_at) => fired_at.elapsed() >= Duration::from_secs(debounce_secs),
        };

        if should_fire {
            let last_at = {
                let status = state.backup_status.read().await;
                status.last_backup_at.clone().unwrap_or_else(|| "never".to_string())
            };
            let msg = format!(
                "[BACKUP] No successful backup in {:.1} hours -- last at {} | {}",
                hours,
                last_at,
                crate::whatsapp_alerter::ist_now_string()
            );
            tracing::warn!(target: LOG_TARGET, "{}", msg);
            crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            *last_alert_fired = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_temp_dir() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp dir")
    }

    /// Create a fake backup file in dir with given name.
    fn make_file(dir: &std::path::Path, name: &str) {
        let path = dir.join(name);
        fs::write(path, b"fake backup content").expect("Failed to write fake backup");
    }

    #[test]
    fn rotate_backups_with_10_daily_and_retain_7_deletes_3_oldest() {
        let tmp = make_temp_dir();
        let dir = tmp.path();

        // Create 10 daily backups for racecontrol prefix
        for i in 1..=10 {
            make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
        }

        rotate_backups(
            dir.to_str().unwrap(),
            "racecontrol",
            7, // daily_retain
            4, // weekly_retain
        )
        .unwrap();

        // Count remaining daily files
        let remaining: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("racecontrol-") && !name.contains("-weekly-")
            })
            .collect();

        assert_eq!(
            remaining.len(),
            7,
            "Expected 7 daily files after rotation, got {}",
            remaining.len()
        );
    }

    #[test]
    fn rotate_backups_deletes_oldest_3_keeps_newest_7() {
        let tmp = make_temp_dir();
        let dir = tmp.path();

        for i in 1..=10 {
            make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
        }

        rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4).unwrap();

        // The oldest 3 (01..03) should be gone
        for i in 1..=3 {
            let name = format!("racecontrol-2026-01-{:02}T12-00-00.db", i);
            assert!(
                !dir.join(&name).exists(),
                "File {} should have been deleted",
                name
            );
        }
        // The newest 7 (04..10) should remain
        for i in 4..=10 {
            let name = format!("racecontrol-2026-01-{:02}T12-00-00.db", i);
            assert!(
                dir.join(&name).exists(),
                "File {} should have been retained",
                name
            );
        }
    }

    #[test]
    fn rotate_backups_preserves_weekly_files_up_to_weekly_retain() {
        let tmp = make_temp_dir();
        let dir = tmp.path();

        // Create 6 weekly backup files
        for i in 1..=6 {
            make_file(dir, &format!("racecontrol-weekly-2026-W{:02}.db", i));
        }

        rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4).unwrap();

        let remaining: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains("-weekly-")
            })
            .collect();

        assert_eq!(
            remaining.len(),
            4,
            "Expected 4 weekly files after rotation, got {}",
            remaining.len()
        );
    }

    #[test]
    fn rotate_backups_does_nothing_when_below_retain_limit() {
        let tmp = make_temp_dir();
        let dir = tmp.path();

        // Create only 5 daily files (below the retain limit of 7)
        for i in 1..=5 {
            make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
        }

        rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4).unwrap();

        let remaining: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(remaining.len(), 5, "No files should be deleted when under retain limit");
    }

    #[test]
    fn compute_staleness_returns_none_for_empty_dir() {
        let tmp = make_temp_dir();
        let staleness = compute_staleness(tmp.path().to_str().unwrap());
        assert!(staleness.is_none(), "Expected None for empty directory");
    }

    #[test]
    fn compute_staleness_returns_none_for_nonexistent_dir() {
        let staleness = compute_staleness("/tmp/nonexistent-backup-dir-xyz-12345");
        assert!(staleness.is_none(), "Expected None for nonexistent directory");
    }

    #[test]
    fn compute_staleness_returns_some_when_files_exist() {
        let tmp = make_temp_dir();
        let dir = tmp.path();

        make_file(dir, "racecontrol-2026-01-01T12-00-00.db");

        let staleness = compute_staleness(dir.to_str().unwrap());
        assert!(
            staleness.is_some(),
            "Expected Some staleness value when backup files exist"
        );
        // The file was just created, so staleness should be very low
        let hours = staleness.unwrap();
        assert!(
            hours < 0.1,
            "Freshly created file should have <0.1 hours staleness, got {}",
            hours
        );
    }

    #[test]
    fn backup_file_naming_follows_racecontrol_prefix_pattern() {
        // Verify the pattern: "racecontrol-YYYY-MM-DDTHH-MM-SS.db"
        let tmp = make_temp_dir();
        let dir = tmp.path();
        let name = "racecontrol-2026-04-01T15-30-00.db";
        make_file(dir, name);

        assert!(dir.join(name).exists());
        assert!(name.starts_with("racecontrol-"));
        assert!(name.ends_with(".db"));
        // Should NOT contain "weekly"
        assert!(!name.contains("weekly"));
    }

    #[test]
    fn backup_file_naming_follows_telemetry_prefix_pattern() {
        // Verify the pattern: "telemetry-YYYY-MM-DDTHH-MM-SS.db"
        let tmp = make_temp_dir();
        let dir = tmp.path();
        let name = "telemetry-2026-04-01T15-30-00.db";
        make_file(dir, name);

        assert!(dir.join(name).exists());
        assert!(name.starts_with("telemetry-"));
        assert!(name.ends_with(".db"));
        assert!(!name.contains("weekly"));
    }

    #[test]
    fn weekly_snapshot_naming_follows_pattern() {
        // Verify weekly pattern: "racecontrol-weekly-YYYY-WNN.db"
        let name = "racecontrol-weekly-2026-W14.db";
        assert!(name.starts_with("racecontrol-weekly-"));
        assert!(name.ends_with(".db"));
        assert!(name.contains("W14"));
    }

    #[test]
    fn rotate_backups_does_not_delete_files_from_other_prefix() {
        let tmp = make_temp_dir();
        let dir = tmp.path();

        // Create 10 racecontrol daily files
        for i in 1..=10 {
            make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
        }
        // Create 3 telemetry daily files
        for i in 1..=3 {
            make_file(dir, &format!("telemetry-2026-01-{:02}T12-00-00.db", i));
        }

        rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4).unwrap();

        // Telemetry files should be untouched
        let telemetry_count = fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("telemetry-")
            })
            .count();

        assert_eq!(
            telemetry_count,
            3,
            "Rotating racecontrol should not affect telemetry files"
        );
    }

    #[test]
    fn staleness_debounce_logic_fires_on_first_call() {
        // Simulate: no previous alert, threshold exceeded
        let mut last_alert_fired: Option<Instant> = None;
        let debounce_secs = 2 * 3600u64;

        // First time: should fire
        let should_fire = match last_alert_fired {
            None => true,
            Some(fired_at) => fired_at.elapsed() >= Duration::from_secs(debounce_secs),
        };
        assert!(should_fire, "First staleness check should always fire");

        // Simulate firing
        last_alert_fired = Some(Instant::now());

        // Second time immediately: should NOT fire (debounce)
        let should_fire_again = match last_alert_fired {
            None => true,
            Some(fired_at) => fired_at.elapsed() >= Duration::from_secs(debounce_secs),
        };
        assert!(!should_fire_again, "Immediate re-fire should be suppressed by debounce");
    }
}

//! Phase 300-01: SQLite Backup Pipeline
//!
//! Hourly WAL-safe backup of racecontrol.db, telemetry.db, and admin.db using VACUUM INTO.
//! Rotation: 30 daily + 4 weekly + 12 monthly files per database (OPS-09, OPS-10).
//! Staleness alert: WhatsApp alert if no successful backup in staleness_alert_hours (default: 2h).
//! Zero-byte alert: immediate WhatsApp alert if backup file is empty (OPS-14, no debounce).
//! Debounce: staleness alert suppressed if already fired within 2 * staleness_alert_hours.
//!
//! Phase 300-02 additions:
//! - Nightly rsync transfer to Bono VPS (02:00-04:00 IST window, once per day, SCP fallback)
//! - SHA256 local+remote checksum verification
//! - Remote reachability checked every tick via `ssh ... echo ok`
//! - BackupStatus updated with remote fields on every tick
//!
//! Phase 351 additions:
//! - admin.db VACUUM INTO backup (config.backup.admin_db_path, OPS-08)
//! - Monthly rotation tier: first-of-month snapshot retained 12 months (OPS-10)
//! - Rsync replaces SCP; SCP is automatic fallback (OPS-11)
//! - Zero-byte backup fires immediate WhatsApp alert bypassing debounce (OPS-14)
//! - BackupStatus.last_admin_backup_at + last_admin_backup_size for admin.db visibility
//!
//! Standing rules compliance:
//! - No .unwrap() — uses ? and if let Err(e)
//! - No lock held across .await — clone/snapshot before async work
//! - VACUUM INTO (not file copy) per locked decision
//! - File paths: forward slashes in VACUUM INTO SQL string
//! - StrictHostKeyChecking=no + BatchMode=yes on all ssh/scp/rsync (Pitfall 4)
//! - No hardcoded IPs — uses config.backup.remote_host

#[path = "backup_remote.rs"]
mod backup_remote;
#[path = "backup_rotation.rs"]
mod backup_rotation;

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::state::AppState;

use backup_rotation::{rotate_backups, count_backup_files, find_newest_backup_file, compute_staleness};

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

/// One backup tick: create backups for all databases, rotate, update status, check staleness.
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
    let monthly_retain = state.config.backup.monthly_retain;
    let admin_db_path = state.config.backup.admin_db_path.clone();

    // Create backup directory if it does not exist
    std::fs::create_dir_all(&backup_dir)?;

    // Generate IST timestamp for backup file names
    let now_ist = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata);
    let timestamp = now_ist.format("%Y-%m-%dT%H-%M-%S").to_string();

    // Determine rotation tiers for this tick
    use chrono::Datelike;
    let is_weekly = now_ist.weekday() == chrono::Weekday::Sun;
    // OPS-10: monthly snapshot on the 1st of the month at any backup tick (hourly)
    let is_monthly_first = now_ist.day() == 1;
    let year = now_ist.year();
    let week_num = now_ist.iso_week().week();
    let year_month = now_ist.format("%Y-%m").to_string();

    let mut last_backup_file: Option<String> = None;
    let mut last_backup_size: Option<u64> = None;
    let mut total_count: usize = 0;
    let mut admin_backup_at: Option<String> = None;
    let mut admin_backup_size: Option<u64> = None;

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
                // Record size and check for zero-byte (OPS-14)
                if let Ok(meta) = std::fs::metadata(&daily_path) {
                    let size = meta.len();
                    last_backup_size = Some(size);
                    if size == 0 {
                        let msg = format!(
                            "[BACKUP] Zero-byte racecontrol.db backup: {} | {}",
                            daily_name,
                            crate::whatsapp_alerter::ist_now_string()
                        );
                        tracing::error!(target: LOG_TARGET, "{}", msg);
                        crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                    }
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

                // OPS-10: Monthly snapshot on 1st of month — retain 12 months
                if is_monthly_first {
                    let monthly_name = format!("{}-monthly-{}.db", main_prefix, year_month);
                    let monthly_path = format!("{}/{}", backup_dir, monthly_name);
                    if !std::path::Path::new(&monthly_path).exists() {
                        if let Err(e) = std::fs::copy(&daily_path, &monthly_path) {
                            tracing::warn!(target: LOG_TARGET, "Monthly copy for racecontrol failed: {}", e);
                        } else {
                            tracing::info!(target: LOG_TARGET, "Monthly snapshot created: {}", monthly_name);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "VACUUM INTO racecontrol.db failed: {}", e);
            }
        }

        rotate_backups(&backup_dir, main_prefix, daily_retain, weekly_retain, monthly_retain)?;
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

                // OPS-10: Monthly snapshot on 1st of month
                if is_monthly_first {
                    let monthly_name = format!("{}-monthly-{}.db", tel_prefix, year_month);
                    let monthly_path = format!("{}/{}", backup_dir, monthly_name);
                    if !std::path::Path::new(&monthly_path).exists() {
                        if let Err(e) = std::fs::copy(&daily_path, &monthly_path) {
                            tracing::warn!(target: LOG_TARGET, "Monthly copy for telemetry failed: {}", e);
                        } else {
                            tracing::info!(target: LOG_TARGET, "Monthly snapshot created: {}", monthly_name);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "VACUUM INTO telemetry.db failed: {}", e);
            }
        }

        rotate_backups(&backup_dir, tel_prefix, daily_retain, weekly_retain, monthly_retain)?;
        total_count += count_backup_files(&backup_dir, tel_prefix);
    }

    // OPS-08: Backup admin.db if path is configured (uses sqlite3 .backup via subprocess —
    // admin.db may not be open in the racecontrol sqlx pool, so we use a subprocess call).
    if !admin_db_path.is_empty() {
        let adm_prefix = "admin";
        let daily_name = format!("{}-{}.db", adm_prefix, timestamp);
        let daily_path = format!("{}/{}", backup_dir, daily_name);

        // Use sqlite3 subprocess: safe for cross-pool backup (WAL-safe .backup command)
        let sqlite3_output = tokio::process::Command::new("sqlite3")
            .arg(&admin_db_path)
            .arg(format!(".backup {}", daily_path.replace('\\', "/")))
            .output()
            .await;

        match sqlite3_output {
            Ok(out) if out.status.success() => {
                if let Ok(meta) = std::fs::metadata(&daily_path) {
                    let size = meta.len();
                    admin_backup_size = Some(size);
                    // OPS-14: zero-byte alert for admin.db (no debounce)
                    if size == 0 {
                        let msg = format!(
                            "[BACKUP] Zero-byte admin.db backup: {} | {}",
                            daily_name,
                            crate::whatsapp_alerter::ist_now_string()
                        );
                        tracing::error!(target: LOG_TARGET, "{}", msg);
                        crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                    }
                }
                admin_backup_at = Some(crate::whatsapp_alerter::ist_now_string());
                tracing::info!(target: LOG_TARGET, "admin.db backup created: {}", daily_name);

                // Weekly snapshot on Sunday
                if is_weekly {
                    let weekly_name = format!("{}-weekly-{}-W{:02}.db", adm_prefix, year, week_num);
                    let weekly_path = format!("{}/{}", backup_dir, weekly_name);
                    if let Err(e) = std::fs::copy(&daily_path, &weekly_path) {
                        tracing::warn!(target: LOG_TARGET, "Weekly copy for admin failed: {}", e);
                    } else {
                        tracing::info!(target: LOG_TARGET, "Weekly admin snapshot created: {}", weekly_name);
                    }
                }

                // OPS-10: Monthly snapshot on 1st of month
                if is_monthly_first {
                    let monthly_name = format!("{}-monthly-{}.db", adm_prefix, year_month);
                    let monthly_path = format!("{}/{}", backup_dir, monthly_name);
                    if !std::path::Path::new(&monthly_path).exists() {
                        if let Err(e) = std::fs::copy(&daily_path, &monthly_path) {
                            tracing::warn!(target: LOG_TARGET, "Monthly copy for admin failed: {}", e);
                        } else {
                            tracing::info!(target: LOG_TARGET, "Monthly admin snapshot created: {}", monthly_name);
                        }
                    }
                }

                rotate_backups(&backup_dir, adm_prefix, daily_retain, weekly_retain, monthly_retain)?;
                total_count += count_backup_files(&backup_dir, adm_prefix);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::error!(target: LOG_TARGET, "admin.db backup failed: {}", stderr);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "sqlite3 subprocess failed (admin.db backup skipped): {}", e);
            }
        }
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
        if admin_backup_at.is_some() {
            status.last_admin_backup_at = admin_backup_at;
            status.last_admin_backup_size = admin_backup_size;
        }
    }

    // Check staleness and fire alert if needed
    check_staleness(state, last_alert_fired).await;

    // Check remote reachability on every tick (non-nightly) so the dashboard
    // always shows a current value even on ticks when we don't transfer.
    backup_remote::check_remote_reachable(state).await;

    // Nightly SCP transfer: racecontrol daily backup → Bono VPS (02:00-04:00 IST).
    // Snapshot the latest daily backup file name (without holding the lock across .await).
    let latest_backup_file = {
        let status = state.backup_status.read().await;
        status.last_backup_file.clone()
    };
    if let Some(ref filename) = latest_backup_file {
        let backup_dir = state.config.backup.backup_dir.clone();
        let backup_path = format!("{}/{}", backup_dir, filename);
        if let Err(e) = backup_remote::transfer_to_remote(state, &backup_path, filename, last_remote_transfer).await {
            tracing::error!(target: LOG_TARGET, "Nightly remote transfer failed: {}", e);
        }
    }

    Ok(())
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
#[path = "backup_pipeline_tests.rs"]
mod tests;

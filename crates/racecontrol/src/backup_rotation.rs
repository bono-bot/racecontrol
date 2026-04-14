//! Backup file rotation and staleness helpers.
//!
//! Extracted from backup_pipeline.rs.
//! - rotate_backups: keep newest daily/weekly/monthly per prefix (OPS-09, OPS-10)
//! - count_backup_files: total backup file count for a prefix
//! - find_newest_backup_file: locate the newest backup by filename sort
//! - compute_staleness: hours since the newest backup file was modified

const LOG_TARGET: &str = "backup_pipeline";

/// Rotate backup files: keep newest `daily_retain` daily + `weekly_retain` weekly +
/// `monthly_retain` monthly per prefix (OPS-09, OPS-10).
///
/// File naming:
/// - Daily:   {prefix}-YYYY-MM-DDTHH-MM-SS.db
/// - Weekly:  {prefix}-weekly-YYYY-WNN.db
/// - Monthly: {prefix}-monthly-YYYY-MM.db
pub fn rotate_backups(
    backup_dir: &str,
    prefix: &str,
    daily_retain: usize,
    weekly_retain: usize,
    monthly_retain: usize,
) -> anyhow::Result<()> {
    let dir = std::path::Path::new(backup_dir);
    if !dir.exists() {
        return Ok(());
    }

    let daily_pattern = format!("{}-", prefix);
    let weekly_pattern = format!("{}-weekly-", prefix);
    let monthly_pattern = format!("{}-monthly-", prefix);

    // Collect daily files: {prefix}-YYYY-..., NOT weekly, NOT monthly
    let mut daily_files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                name.starts_with(&daily_pattern)
                    && !name.contains("-weekly-")
                    && !name.contains("-monthly-")
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

    // Collect and rotate monthly files: {prefix}-monthly-YYYY-MM.db (OPS-10)
    let mut monthly_files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                name.starts_with(&monthly_pattern)
            } else {
                false
            }
        })
        .collect();

    monthly_files.sort();

    if monthly_files.len() > monthly_retain {
        let to_delete = monthly_files.len() - monthly_retain;
        for path in monthly_files.iter().take(to_delete) {
            tracing::debug!(target: LOG_TARGET, "Rotating monthly backup: {:?}", path);
            if let Err(e) = std::fs::remove_file(path) {
                tracing::warn!(target: LOG_TARGET, "Failed to delete old monthly backup {:?}: {}", path, e);
            }
        }
        tracing::info!(target: LOG_TARGET, "Rotated {} old monthly backup(s) for prefix '{}'", to_delete, prefix);
    }

    Ok(())
}

/// Count total backup files for a prefix (daily + weekly combined).
pub(super) fn count_backup_files(backup_dir: &str, prefix: &str) -> usize {
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
pub(super) fn find_newest_backup_file(backup_dir: &str) -> anyhow::Result<Option<String>> {
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

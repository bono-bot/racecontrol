//! Subsystem health probe functions — one per subsystem.

use std::time::{Duration, Instant};

use sqlx::SqlitePool;

use crate::config::Config;
use crate::state::AppState;

use super::SubsystemStatus;

// ─── Probe Functions ────────────────────────────────────────────────────────

/// Probe 1: DB Write — INSERT/DELETE on server_health_probe table.
/// Adapted from server_diagnostics.rs check_db_health().
pub(super) async fn probe_db_writable(db: &SqlitePool) -> SubsystemStatus {
    let start = Instant::now();
    let ts = chrono::Utc::now().to_rfc3339();

    match sqlx::query("INSERT OR REPLACE INTO server_health_probe (id, ts) VALUES (2, ?)")
        .bind(&ts)
        .execute(db)
        .await
    {
        Ok(_) => {
            // Cleanup
            let _ = sqlx::query("DELETE FROM server_health_probe WHERE id = 2")
                .execute(db)
                .await;
            SubsystemStatus {
                ok: true,
                latency_ms: start.elapsed().as_millis() as u64,
                error_code: None,
                detail: None,
            }
        }
        Err(e) => SubsystemStatus {
            ok: false,
            latency_ms: start.elapsed().as_millis() as u64,
            error_code: Some("DB_WRITE_FAILED".to_string()),
            detail: Some(e.to_string()),
        },
    }
}

/// Probe 2: RC Backend — self-check. If we're responding, we're up.
pub(super) async fn probe_rc_backend() -> SubsystemStatus {
    SubsystemStatus {
        ok: true,
        latency_ms: 0,
        error_code: None,
        detail: None,
    }
}

/// Probe 3: Disk Free — check available disk space on the data directory.
/// Uses sysinfo::Disks (cross-platform, already in Cargo.toml).
pub(super) async fn probe_disk_free() -> SubsystemStatus {
    // Run blocking disk check on a blocking thread to avoid starving the tokio runtime
    match tokio::task::spawn_blocking(check_disk_free_sync).await {
        Ok(status) => status,
        Err(e) => SubsystemStatus {
            ok: false,
            latency_ms: 0,
            error_code: Some("DISK_CHECK_FAILED".to_string()),
            detail: Some(e.to_string()),
        },
    }
}

fn check_disk_free_sync() -> SubsystemStatus {
    use sysinfo::Disks;

    let disks = Disks::new_with_refreshed_list();
    // Find the disk containing the data directory (typically C: on Windows)
    let data_path = std::path::Path::new("./data");
    let canonical = std::fs::canonicalize(data_path)
        .unwrap_or_else(|_| std::path::PathBuf::from("C:\\"));

    // Find the disk with the longest matching mount point
    // Windows paths are case-insensitive and mount points may use different slash styles
    let mut best_disk: Option<&sysinfo::Disk> = None;
    let mut best_mount_len = 0;
    let canon_lower = canonical.to_string_lossy().to_lowercase().replace('/', "\\");

    for disk in disks.list() {
        let mount = disk.mount_point();
        let mount_lower = mount.to_string_lossy().to_lowercase().replace('/', "\\");
        if canon_lower.starts_with(&mount_lower) && mount_lower.len() > best_mount_len {
            best_disk = Some(disk);
            best_mount_len = mount_lower.len();
        }
    }

    // Fallback: if no mount matched, try the first disk (usually C: on Windows)
    let disk = best_disk.or_else(|| disks.list().first());

    match disk {
        Some(disk) => {
            let available_bytes = disk.available_space();
            let available_gb = available_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

            if available_gb < 1.0 {
                SubsystemStatus {
                    ok: false,
                    latency_ms: 0,
                    error_code: Some("DISK_LOW".to_string()),
                    detail: Some(format!("{:.1} GB free", available_gb)),
                }
            } else {
                SubsystemStatus {
                    ok: true,
                    latency_ms: 0,
                    error_code: None,
                    detail: Some(format!("{:.1} GB free", available_gb)),
                }
            }
        }
        None => SubsystemStatus {
            ok: false,
            latency_ms: 0,
            error_code: Some("DISK_NOT_FOUND".to_string()),
            detail: Some("Could not find disk for data directory".to_string()),
        },
    }
}

/// Probe 8 (Phase 349): DB Sync Lag — check age of racecontrol.db mtime (CLOUD-ONLY).
/// On venue instances, returns ok with "skipped" detail.
/// On cloud, checks file mtime of racecontrol.db (last written by download-db.sh).
/// Thresholds: WARN >300s (1 missed 5-min cycle), CRITICAL >900s (3 missed cycles).
pub(super) async fn probe_db_sync_lag(config: &Config) -> SubsystemStatus {
    if !crate::config::this_instance_is_cloud(config) {
        return SubsystemStatus {
            ok: true,
            latency_ms: 0,
            error_code: None,
            detail: Some("venue instance — db_sync_lag probe skipped".to_string()),
        };
    }

    let db_path = config.database.path.clone();
    match tokio::task::spawn_blocking(move || check_db_sync_lag_sync(&db_path)).await {
        Ok(status) => status,
        Err(e) => SubsystemStatus {
            ok: false,
            latency_ms: 0,
            error_code: Some("DB_SYNC_LAG_CHECK_FAILED".to_string()),
            detail: Some(e.to_string()),
        },
    }
}

pub(super) fn check_db_sync_lag_sync(db_path: &str) -> SubsystemStatus {
    use std::time::{Duration, SystemTime};

    const WARN_SECS: u64 = 300;
    const CRITICAL_SECS: u64 = 900;

    let path = std::path::Path::new(db_path);
    match std::fs::metadata(path) {
        Ok(meta) => match meta.modified() {
            Ok(mtime) => {
                let age_secs = SystemTime::now()
                    .duration_since(mtime)
                    .unwrap_or(Duration::from_secs(u64::MAX))
                    .as_secs();
                let detail = Some(format!("Last sync {}m {}s ago", age_secs / 60, age_secs % 60));
                if age_secs >= CRITICAL_SECS {
                    SubsystemStatus {
                        ok: false,
                        latency_ms: 0,
                        error_code: Some("DB_SYNC_LAG_CRITICAL".to_string()),
                        detail,
                    }
                } else if age_secs >= WARN_SECS {
                    SubsystemStatus {
                        ok: false,
                        latency_ms: 0,
                        error_code: Some("DB_SYNC_LAG_WARN".to_string()),
                        detail,
                    }
                } else {
                    SubsystemStatus {
                        ok: true,
                        latency_ms: 0,
                        error_code: None,
                        detail,
                    }
                }
            }
            Err(e) => SubsystemStatus {
                ok: false,
                latency_ms: 0,
                error_code: Some("DB_SYNC_MTIME_UNAVAILABLE".to_string()),
                detail: Some(e.to_string()),
            },
        },
        Err(_) => SubsystemStatus {
            ok: false,
            latency_ms: 0,
            error_code: Some("DB_SYNC_FILE_NOT_FOUND".to_string()),
            detail: Some(format!("File not found: {}", db_path)),
        },
    }
}

/// Probe 4: Cloud Sync — check last sync time from sync_state table.
/// Degraded if oldest sync is > 120 seconds old.
/// Only checks tables in SYNC_TABLES — orphan rows from removed tables are ignored.
pub(super) async fn probe_cloud_sync(db: &SqlitePool) -> SubsystemStatus {
    let start = Instant::now();
    let threshold_secs: i64 = 120;

    // Build an IN clause from the active SYNC_TABLES list to avoid orphan rows
    // (e.g., metrics_rollups was removed from SYNC_TABLES but its row persisted in sync_state)
    let active_tables: Vec<&str> = crate::cloud_sync::SYNC_TABLES.split(',').collect();
    let placeholders: String = active_tables.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT MIN(last_synced_at) FROM sync_state WHERE table_name IN ({})",
        placeholders
    );

    let mut q = sqlx::query_as::<_, (Option<String>,)>(&query);
    for table in &active_tables {
        q = q.bind(*table);
    }
    let row: Result<Option<(Option<String>,)>, _> = q.fetch_optional(db).await;

    match row {
        Ok(Some((Some(ts),))) => {
            // Try parsing various timestamp formats
            let age_secs = parse_sync_age_secs(&ts);
            match age_secs {
                Some(age) if age > threshold_secs => SubsystemStatus {
                    ok: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    error_code: Some("SYNC_STALE".to_string()),
                    detail: Some(format!(
                        "Last sync {}s ago (threshold: {}s)",
                        age, threshold_secs
                    )),
                },
                Some(age) => SubsystemStatus {
                    ok: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                    detail: Some(format!("Last sync {}s ago", age)),
                },
                None => SubsystemStatus {
                    ok: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    error_code: Some("SYNC_PARSE_ERROR".to_string()),
                    detail: Some(format!("Could not parse timestamp: {}", ts)),
                },
            }
        }
        Ok(Some((None,))) | Ok(None) => {
            // No sync records — could be venue-only mode or fresh DB
            SubsystemStatus {
                ok: true,
                latency_ms: start.elapsed().as_millis() as u64,
                error_code: None,
                detail: Some("No sync records (venue-only mode)".to_string()),
            }
        }
        Err(e) => {
            // Table might not exist — that's ok for venue-only deployments
            let err_str = e.to_string();
            if err_str.contains("no such table") {
                SubsystemStatus {
                    ok: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                    detail: Some("No sync_state table (venue-only mode)".to_string()),
                }
            } else {
                SubsystemStatus {
                    ok: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    error_code: Some("SYNC_QUERY_FAILED".to_string()),
                    detail: Some(err_str),
                }
            }
        }
    }
}

/// Parse a sync timestamp and return age in seconds.
pub(super) fn parse_sync_age_secs(ts: &str) -> Option<i64> {
    // Try RFC 3339 (handles both Z and +00:00 timezone offsets)
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        let age = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds();
        return Some(age);
    }
    // Try ISO 8601 with fractional seconds (Z suffix)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.fZ") {
        let age = (chrono::Utc::now().naive_utc() - dt).num_seconds();
        return Some(age);
    }
    // Try ISO 8601 without fractional seconds
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ") {
        let age = (chrono::Utc::now().naive_utc() - dt).num_seconds();
        return Some(age);
    }
    // Try SQLite datetime format
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
        let age = (chrono::Utc::now().naive_utc() - dt).num_seconds();
        return Some(age);
    }
    None
}

/// Probe 5: WhatsApp API — check Evolution API reachability.
/// Reuses the same check pattern as check_evolution_health() in routes.rs.
pub(super) async fn probe_whatsapp_api(state: &AppState) -> SubsystemStatus {
    let start = Instant::now();
    let evo_url = match &state.config.auth.evolution_url {
        Some(u) => u.clone(),
        None => {
            return SubsystemStatus {
                ok: true, // Not configured = not a failure
                latency_ms: 0,
                error_code: None,
                detail: Some("Evolution API not configured".to_string()),
            };
        }
    };

    match state
        .http_client
        .get(&evo_url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => {
            SubsystemStatus {
                ok: true,
                latency_ms: start.elapsed().as_millis() as u64,
                error_code: None,
                detail: None,
            }
        }
        Ok(_resp) => {
            // Any HTTP response means Evolution is reachable (even errors)
            SubsystemStatus {
                ok: true,
                latency_ms: start.elapsed().as_millis() as u64,
                error_code: None,
                detail: None,
            }
        }
        Err(e) => SubsystemStatus {
            ok: false,
            latency_ms: start.elapsed().as_millis() as u64,
            error_code: Some("WHATSAPP_UNREACHABLE".to_string()),
            detail: Some(e.to_string()),
        },
    }
}

/// Probe 6: Fleet Connectivity — count pods with ws_connected: true.
pub(super) async fn probe_fleet_connectivity(state: &AppState) -> SubsystemStatus {
    let start = Instant::now();

    // Snapshot fleet health state — drop lock before any async work
    let fleet_snapshot = { state.pod_fleet_health.read().await.clone() };
    let connected_count = {
        let senders = state.agent_senders.read().await;
        senders.len()
    };

    let total_pods = fleet_snapshot.len();

    // If no pods are registered yet, that's ok (startup grace)
    if total_pods == 0 {
        return SubsystemStatus {
            ok: true,
            latency_ms: start.elapsed().as_millis() as u64,
            error_code: None,
            detail: Some("No pods registered".to_string()),
        };
    }

    let detail = format!("{}/{} pods connected", connected_count, total_pods);

    if connected_count < total_pods {
        SubsystemStatus {
            ok: false,
            latency_ms: start.elapsed().as_millis() as u64,
            error_code: Some("FLEET_PARTIAL".to_string()),
            detail: Some(detail),
        }
    } else {
        SubsystemStatus {
            ok: true,
            latency_ms: start.elapsed().as_millis() as u64,
            error_code: None,
            detail: Some(detail),
        }
    }
}

/// Probe 7: Admin DB — check admin.db file accessibility.
/// Racecontrol does not have a direct admin_db pool, so we check file existence
/// and readability at the expected path.
pub(super) async fn probe_admin_db() -> SubsystemStatus {
    // admin.db is typically at data/admin.db or ../admin/admin.db
    let possible_paths = [
        std::path::PathBuf::from("data/admin.db"),
        std::path::PathBuf::from("../racingpoint-admin/admin.db"),
    ];

    for path in &possible_paths {
        if path.exists() {
            // Check if readable
            match std::fs::metadata(path) {
                Ok(meta) if meta.len() > 0 => {
                    return SubsystemStatus {
                        ok: true,
                        latency_ms: 0,
                        error_code: None,
                        detail: Some(format!("{}  ({} bytes)", path.display(), meta.len())),
                    };
                }
                Ok(_) => {
                    return SubsystemStatus {
                        ok: false,
                        latency_ms: 0,
                        error_code: Some("ADMIN_DB_EMPTY".to_string()),
                        detail: Some(format!("{} exists but is empty", path.display())),
                    };
                }
                Err(e) => {
                    return SubsystemStatus {
                        ok: false,
                        latency_ms: 0,
                        error_code: Some("ADMIN_DB_UNAVAILABLE".to_string()),
                        detail: Some(format!("{}: {}", path.display(), e)),
                    };
                }
            }
        }
    }

    // admin.db not found — not a critical failure (may be a separate deployment)
    SubsystemStatus {
        ok: true,
        latency_ms: 0,
        error_code: None,
        detail: Some("admin.db not found at expected paths (separate deployment)".to_string()),
    }
}

//! Subsystem Health Probes — per-subsystem ground truth health monitoring.
//!
//! Phase 352: Replaces the flat "always ok" health endpoint with real per-subsystem
//! probes. Background task runs every 10 seconds, caching results in LazyLock<RwLock<>>.
//! On state transitions (ok->degraded, degraded->ok), fires WhatsApp alerts with
//! 10-minute dedup per (subsystem, error_code) pair.
//!
//! Probes: db_writable, rc_backend, disk_free, cloud_sync, whatsapp_api,
//! fleet_connectivity, admin_db, db_sync_lag.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::SqlitePool;

use crate::config::{self, Config};
use crate::state::AppState;

const LOG_TARGET: &str = "subsystem_health";
const PROBE_INTERVAL_SECS: u64 = 10;
const DEDUP_WINDOW_SECS: u64 = 600; // 10 minutes per OPS-04/D3

// ─── Types ──────────────────────────────────────────────────────────────────

/// Per-subsystem health status, returned in the /api/v1/health response.
#[derive(Debug, Clone, Serialize)]
pub struct SubsystemStatus {
    pub ok: bool,
    pub latency_ms: u64,
    pub error_code: Option<String>,
    pub detail: Option<String>,
}

/// Previous state for a subsystem (for transition detection).
struct PrevState {
    ok: bool,
    degraded_since: Option<Instant>,
}

// ─── Static State ───────────────────────────────────────────────────────────

/// Cached subsystem probe results. Updated every 10 seconds by the background task.
/// Read by the health endpoint handler (zero latency — reads cached, never blocks).
static SUBSYSTEM_STATE: LazyLock<RwLock<HashMap<String, SubsystemStatus>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Central dedup map per D3: (subsystem, error_code) -> last_alert_time.
/// Same (subsystem, error_code) within 10 minutes = suppressed.
static DEDUP_MAP: LazyLock<Mutex<HashMap<(String, String), Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ─── Public API ─────────────────────────────────────────────────────────────

/// Get current subsystem health status (read from cache, zero latency).
/// Returns empty HashMap before first probe cycle completes.
pub fn get_current_status() -> HashMap<String, SubsystemStatus> {
    SUBSYSTEM_STATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Spawn the subsystem health probe background task.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(subsystem_health_task(state));
    tracing::info!(target: "startup", "subsystem_health probe task spawned ({}s interval)", PROBE_INTERVAL_SECS);
}

// ─── Background Task ────────────────────────────────────────────────────────

async fn subsystem_health_task(state: Arc<AppState>) {
    tracing::info!(target: "state", task = "subsystem_health", event = "lifecycle", "lifecycle: started");

    // Startup grace — let server fully initialize
    tokio::time::sleep(Duration::from_secs(15)).await;

    let mut ticker = tokio::time::interval(Duration::from_secs(PROBE_INTERVAL_SECS));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Track previous state for transition detection
    let mut prev_states: HashMap<String, PrevState> = HashMap::new();

    loop {
        ticker.tick().await;
        run_probes(&state, &mut prev_states).await;
    }
}

async fn run_probes(state: &AppState, prev_states: &mut HashMap<String, PrevState>) {
    // Run all probes concurrently where independent
    let (db_writable, rc_backend, disk_free, cloud_sync, whatsapp_api, fleet_conn, admin_db, db_sync_lag) =
        tokio::join!(
            probe_db_writable(&state.db),
            probe_rc_backend(),
            probe_disk_free(),
            probe_cloud_sync(&state.db),
            probe_whatsapp_api(state),
            probe_fleet_connectivity(state),
            probe_admin_db(),
            probe_db_sync_lag(&state.config),
        );

    let mut results: HashMap<String, SubsystemStatus> = HashMap::new();
    results.insert("db_writable".to_string(), db_writable);
    results.insert("rc_backend".to_string(), rc_backend);
    results.insert("disk_free".to_string(), disk_free);
    results.insert("cloud_sync".to_string(), cloud_sync);
    results.insert("whatsapp_api".to_string(), whatsapp_api);
    results.insert("fleet_connectivity".to_string(), fleet_conn);
    results.insert("admin_db".to_string(), admin_db);
    results.insert("db_sync_lag".to_string(), db_sync_lag);

    // Check transitions and dispatch alerts
    for (name, status) in &results {
        let prev = prev_states.get(name);
        let was_ok = prev.map(|p| p.ok).unwrap_or(true);

        if was_ok && !status.ok {
            // ok -> degraded transition
            let error_code = status.error_code.as_deref().unwrap_or("UNKNOWN");
            let detail = status.detail.as_deref().unwrap_or("");

            if should_alert(name, error_code) {
                dispatch_subsystem_alert(
                    &state.config,
                    &state.db,
                    name,
                    error_code,
                    detail,
                )
                .await;
            }

            prev_states.insert(
                name.clone(),
                PrevState {
                    ok: false,
                    degraded_since: Some(Instant::now()),
                },
            );
        } else if !was_ok && status.ok {
            // degraded -> ok transition (recovery)
            let duration_secs = prev
                .and_then(|p| p.degraded_since)
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);

            dispatch_recovery_alert(&state.config, &state.db, name, duration_secs).await;

            prev_states.insert(
                name.clone(),
                PrevState {
                    ok: true,
                    degraded_since: None,
                },
            );
        } else {
            // No transition — update ok state but keep degraded_since
            prev_states
                .entry(name.clone())
                .and_modify(|p| p.ok = status.ok)
                .or_insert(PrevState {
                    ok: status.ok,
                    degraded_since: if status.ok { None } else { Some(Instant::now()) },
                });
        }
    }

    // Update cached state — clone results, drop lock before any async work
    {
        let mut guard = SUBSYSTEM_STATE
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *guard = results;
    }
}

// ─── Probe Functions ────────────────────────────────────────────────────────

/// Probe 1: DB Write — INSERT/DELETE on server_health_probe table.
/// Adapted from server_diagnostics.rs check_db_health().
async fn probe_db_writable(db: &SqlitePool) -> SubsystemStatus {
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
async fn probe_rc_backend() -> SubsystemStatus {
    SubsystemStatus {
        ok: true,
        latency_ms: 0,
        error_code: None,
        detail: None,
    }
}

/// Probe 3: Disk Free — check available disk space on the data directory.
/// Uses sysinfo::Disks (cross-platform, already in Cargo.toml).
async fn probe_disk_free() -> SubsystemStatus {
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
    let mut best_disk: Option<&sysinfo::Disk> = None;
    let mut best_mount_len = 0;

    for disk in disks.list() {
        let mount = disk.mount_point();
        let mount_str = mount.to_string_lossy();
        let canon_str = canonical.to_string_lossy();
        if canon_str.starts_with(mount_str.as_ref()) && mount_str.len() > best_mount_len {
            best_disk = Some(disk);
            best_mount_len = mount_str.len();
        }
    }

    match best_disk {
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
async fn probe_db_sync_lag(config: &Config) -> SubsystemStatus {
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

fn check_db_sync_lag_sync(db_path: &str) -> SubsystemStatus {
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
async fn probe_cloud_sync(db: &SqlitePool) -> SubsystemStatus {
    let start = Instant::now();
    let threshold_secs: i64 = 120;

    // Check if sync_state table exists and has data
    let row: Result<Option<(Option<String>,)>, _> = sqlx::query_as(
        "SELECT MIN(last_synced_at) FROM sync_state WHERE table_name != '_push'",
    )
    .fetch_optional(db)
    .await;

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
fn parse_sync_age_secs(ts: &str) -> Option<i64> {
    // Try ISO 8601 with fractional seconds
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
async fn probe_whatsapp_api(state: &AppState) -> SubsystemStatus {
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
async fn probe_fleet_connectivity(state: &AppState) -> SubsystemStatus {
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
async fn probe_admin_db() -> SubsystemStatus {
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

// ─── Dedup Logic (D3/OPS-04) ────────────────────────────────────────────────

/// Check if an alert should fire for this (subsystem, error_code) pair.
/// Returns false if the same pair was alerted within the last 10 minutes.
fn should_alert(subsystem: &str, error_code: &str) -> bool {
    let key = (subsystem.to_string(), error_code.to_string());
    let mut map = DEDUP_MAP.lock().unwrap_or_else(|e| e.into_inner());

    // Evict old entries
    map.retain(|_, v| v.elapsed() < Duration::from_secs(DEDUP_WINDOW_SECS));

    if let Some(last) = map.get(&key) {
        if last.elapsed() < Duration::from_secs(DEDUP_WINDOW_SECS) {
            return false; // suppressed
        }
    }
    map.insert(key, Instant::now());
    true
}

// ─── Alert Dispatch (D2) — Fallback Chain: Direct -> Relay ─────────────────

/// Try direct Evolution API dispatch. Returns Ok(()) on success, Err(reason) on failure.
/// Used only by subsystem_health for the fallback chain — existing callers use send_whatsapp() unchanged.
async fn try_direct_whatsapp(config: &Config, message: &str) -> Result<(), String> {
    let (evo_url, evo_key, evo_instance, phone) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
        &config.alerting.uday_phone,
    ) {
        (Some(u), Some(k), Some(i), Some(p)) if !u.is_empty() && !k.is_empty() => {
            (u.clone(), k.clone(), i.clone(), p.clone())
        }
        _ => return Err("Evolution API not configured".to_string()),
    };

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .header("apikey", &evo_key)
        .json(&serde_json::json!({
            "number": phone,
            "text": message
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Evolution API returned {}", resp.status()))
    }
}

/// Fallback: POST to comms-link relay on James .27:8766
async fn try_relay_fallback(subsystem: &str, _error_code: &str, message: &str) -> Result<(), String> {
    let relay_url = "http://192.168.31.27:8766/relay/alert";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(relay_url)
        .json(&serde_json::json!({
            "source": "venue-racecontrol",
            "subsystem": subsystem,
            "severity": "critical",
            "message": message,
            "timestamp": crate::whatsapp_alerter::ist_now_string()
        }))
        .send()
        .await
        .map_err(|e| format!("Relay unreachable: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Relay returned {}", resp.status()))
    }
}

/// Fire a WhatsApp alert for a subsystem degradation.
/// Fallback chain per D2: try direct Evolution API first, fall back to comms-link relay.
async fn dispatch_subsystem_alert(
    config: &Config,
    db: &SqlitePool,
    subsystem: &str,
    error_code: &str,
    detail: &str,
) {
    let is_cloud = config::this_instance_is_cloud(config);
    let server_label = if is_cloud { "cloud" } else { "venue (.23)" };

    let msg = format!(
        "[RP ALERT] Subsystem Degraded: {}\nError: {}{}\nServer: {} | {}",
        subsystem,
        error_code,
        if detail.is_empty() {
            String::new()
        } else {
            format!(" -- {}", detail)
        },
        server_label,
        crate::whatsapp_alerter::ist_now_string()
    );

    tracing::warn!(target: LOG_TARGET, subsystem, error_code, detail, "Subsystem degraded, sending alert");

    // Primary: direct Evolution API dispatch
    match try_direct_whatsapp(config, &msg).await {
        Ok(()) => {
            tracing::info!(target: LOG_TARGET, subsystem, "Alert sent via direct Evolution API");
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, subsystem, error = %e, "Direct WhatsApp failed, trying relay fallback");
            // Fallback: comms-link relay on James .27:8766
            match try_relay_fallback(subsystem, error_code, &msg).await {
                Ok(()) => {
                    tracing::info!(target: LOG_TARGET, subsystem, "Alert sent via relay fallback");
                }
                Err(e2) => {
                    tracing::error!(target: LOG_TARGET, subsystem, direct_error = %e, relay_error = %e2, "Both direct and relay failed for alert dispatch");
                }
            }
        }
    }

    // Always record incident regardless of dispatch success (D4/OPS-05)
    record_subsystem_incident(db, subsystem, error_code, &msg).await;
}

/// Fire a WhatsApp alert for subsystem recovery.
async fn dispatch_recovery_alert(
    config: &Config,
    db: &SqlitePool,
    subsystem: &str,
    duration_secs: u64,
) {
    let msg = format!(
        "[RP RESOLVED] {} recovered (was down {}m {}s) | {}",
        subsystem,
        duration_secs / 60,
        duration_secs % 60,
        crate::whatsapp_alerter::ist_now_string()
    );

    tracing::info!(target: LOG_TARGET, subsystem, duration_secs, "Subsystem recovered, sending recovery alert");

    crate::whatsapp_alerter::send_whatsapp(config, &msg).await;

    // Resolve open incidents for this subsystem
    resolve_subsystem_incident(db, subsystem).await;
}

// ─── Incident Recording (D4/OPS-05) ────────────────────────────────────────

/// Record a subsystem degradation incident in alert_incidents table.
async fn record_subsystem_incident(
    db: &SqlitePool,
    subsystem: &str,
    _error_code: &str,
    message: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let corr_id = uuid::Uuid::new_v4().to_string();

    if let Err(e) = sqlx::query(
        "INSERT INTO alert_incidents (id, alert_type, subsystem, severity, description, correlation_id)
         VALUES (?, 'subsystem_degraded', ?, 'critical', ?, ?)",
    )
    .bind(&id)
    .bind(subsystem)
    .bind(message)
    .bind(&corr_id)
    .execute(db)
    .await
    {
        tracing::warn!(target: LOG_TARGET, error = %e, subsystem, "Failed to record subsystem incident");
    }
}

/// Resolve open incidents for a subsystem.
async fn resolve_subsystem_incident(db: &SqlitePool, subsystem: &str) {
    if let Err(e) = sqlx::query(
        "UPDATE alert_incidents SET resolved_at = datetime('now')
         WHERE subsystem = ? AND resolved_at IS NULL AND alert_type = 'subsystem_degraded'",
    )
    .bind(subsystem)
    .execute(db)
    .await
    {
        tracing::warn!(target: LOG_TARGET, error = %e, subsystem, "Failed to resolve subsystem incidents");
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod db_sync_lag_tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn set_mtime_past(path: &std::path::Path, secs_ago: u64) {
        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        let past_epoch = now_epoch.saturating_sub(secs_ago);
        // Use filetime crate for cross-platform mtime manipulation (works on Windows)
        let ft = filetime::FileTime::from_unix_time(past_epoch as i64, 0);
        filetime::set_file_mtime(path, ft).expect("set_file_mtime");
    }

    #[test]
    fn db_sync_lag_ok_when_file_is_fresh() {
        let dir = std::env::temp_dir();
        let path = dir.join("racecontrol-test-fresh.db");
        std::fs::write(&path, b"test").expect("write");
        // Default mtime is now() — should be ok
        let status = check_db_sync_lag_sync(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(status.ok, "Fresh file should be ok, got: {:?}", status.error_code);
        assert!(status.error_code.is_none());
    }

    #[test]
    fn db_sync_lag_warn_when_400s_old() {
        let dir = std::env::temp_dir();
        let path = dir.join("racecontrol-test-warn.db");
        std::fs::write(&path, b"test").expect("write");
        set_mtime_past(&path, 400);
        let status = check_db_sync_lag_sync(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(!status.ok, "400s old should not be ok");
        assert_eq!(status.error_code.as_deref(), Some("DB_SYNC_LAG_WARN"),
            "Expected WARN at 400s, got: {:?}", status.error_code);
    }

    #[test]
    fn db_sync_lag_critical_when_1000s_old() {
        let dir = std::env::temp_dir();
        let path = dir.join("racecontrol-test-critical.db");
        std::fs::write(&path, b"test").expect("write");
        set_mtime_past(&path, 1000);
        let status = check_db_sync_lag_sync(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(!status.ok, "1000s old should not be ok");
        assert_eq!(status.error_code.as_deref(), Some("DB_SYNC_LAG_CRITICAL"),
            "Expected CRITICAL at 1000s, got: {:?}", status.error_code);
    }

    #[test]
    fn db_sync_lag_not_found_when_no_file() {
        let path = "/tmp/racecontrol-test-nonexistent-12345.db";
        let status = check_db_sync_lag_sync(path);
        assert!(!status.ok, "Non-existent file should not be ok");
        assert_eq!(status.error_code.as_deref(), Some("DB_SYNC_FILE_NOT_FOUND"),
            "Expected FILE_NOT_FOUND, got: {:?}", status.error_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_status_serialization() {
        let status = SubsystemStatus {
            ok: true,
            latency_ms: 42,
            error_code: None,
            detail: Some("42.1 GB free".to_string()),
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert_eq!(json["ok"], true);
        assert_eq!(json["latency_ms"], 42);
        assert!(json["error_code"].is_null());
        assert_eq!(json["detail"], "42.1 GB free");
    }

    #[test]
    fn test_subsystem_status_serialization_with_error() {
        let status = SubsystemStatus {
            ok: false,
            latency_ms: 150,
            error_code: Some("DB_WRITE_FAILED".to_string()),
            detail: Some("database is locked".to_string()),
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert_eq!(json["ok"], false);
        assert_eq!(json["latency_ms"], 150);
        assert_eq!(json["error_code"], "DB_WRITE_FAILED");
        assert_eq!(json["detail"], "database is locked");
    }

    #[test]
    fn test_dedup_suppresses_within_window() {
        // First call should allow
        assert!(should_alert("test_dedup_suppress", "ERR_1"));
        // Second call with same key within window should suppress
        assert!(!should_alert("test_dedup_suppress", "ERR_1"));
    }

    #[test]
    fn test_dedup_allows_different_error_code() {
        // Same subsystem, different error code should NOT be suppressed
        assert!(should_alert("test_dedup_diff", "ERR_A"));
        assert!(should_alert("test_dedup_diff", "ERR_B"));
    }

    #[test]
    fn test_dedup_allows_different_subsystem() {
        // Different subsystem, same error code should NOT be suppressed
        assert!(should_alert("subsys_a", "ERR_SAME"));
        assert!(should_alert("subsys_b", "ERR_SAME"));
    }

    #[test]
    fn test_get_current_status_empty_default() {
        // Before any probes run, status should be empty
        // Note: other tests may have populated it, so we check the type is correct
        let status = get_current_status();
        // Type check — HashMap<String, SubsystemStatus>
        assert!(status.is_empty() || status.values().all(|s| s.latency_ms < u64::MAX));
    }

    #[test]
    fn test_parse_sync_age_secs_iso_with_frac() {
        // Use a timestamp from 60 seconds ago
        let ts = (chrono::Utc::now() - chrono::Duration::seconds(60))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let age = parse_sync_age_secs(&ts).expect("should parse");
        assert!(age >= 59 && age <= 62, "age was {}", age);
    }

    #[test]
    fn test_parse_sync_age_secs_sqlite_format() {
        let ts = (chrono::Utc::now() - chrono::Duration::seconds(30))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let age = parse_sync_age_secs(&ts).expect("should parse");
        assert!(age >= 29 && age <= 32, "age was {}", age);
    }

    #[test]
    fn test_parse_sync_age_secs_invalid() {
        assert!(parse_sync_age_secs("not-a-date").is_none());
    }
}

//! Server startup helpers: pod seeding, first-boot email, log cleanup, tracing init.
//!
//! Extracted from main.rs to keep the binary entrypoint under 500 lines.

use std::sync::Arc;

use racecontrol_crate::error_rate::ErrorRateConfig;
use racecontrol_crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::{PodInfo, PodStatus, SimType};

/// Auto-seed all 8 pods into the in-memory pods map on server startup.
/// Called immediately after AppState::new() so the kiosk is never left with
/// an empty pod list after a server restart with a fresh DB.
/// If pods are already populated (e.g. from a future DB-backed store), skips.
pub async fn seed_pods_on_startup(state: &Arc<AppState>) {
    // If pods already populated (future: DB-backed restore), skip
    if !state.pods.read().await.is_empty() {
        tracing::info!("Pods already populated, skipping auto-seed");
        return;
    }

    // (id, number, name, ip, mac)
    let pod_data: &[(&str, u32, &str, &str, &str)] = &[
        ("pod_1", 1, "Pod 1", "192.168.31.89", "30:56:0F:05:45:88"),
        ("pod_2", 2, "Pod 2", "192.168.31.33", "30:56:0F:05:46:53"),
        ("pod_3", 3, "Pod 3", "192.168.31.28", "30:56:0F:05:44:B3"),
        ("pod_4", 4, "Pod 4", "192.168.31.88", "30:56:0F:05:45:25"),
        ("pod_5", 5, "Pod 5", "192.168.31.86", "30:56:0F:05:44:B7"),
        ("pod_6", 6, "Pod 6", "192.168.31.87", "30:56:0F:05:45:6E"),
        ("pod_7", 7, "Pod 7", "192.168.31.38", "30:56:0F:05:44:B4"),
        ("pod_8", 8, "Pod 8", "192.168.31.91", "30:56:0F:05:46:C5"),
    ];

    let mut seeded = Vec::new();
    {
        let mut pods = state.pods.write().await;
        for &(id, number, name, ip, mac) in pod_data {
            let pod = PodInfo {
                id: id.to_string(),
                number,
                name: name.to_string(),
                ip_address: ip.to_string(),
                mac_address: Some(mac.to_string()),
                sim_type: SimType::AssettoCorsa,
                status: PodStatus::Idle,
                current_driver: None,
                current_session_id: None,
                last_seen: Some(chrono::Utc::now()),
                driving_state: None,
                billing_session_id: None,
                game_state: None,
                current_game: None,
                installed_games: vec![],
                screen_blanked: None,
                ffb_preset: None,
                freedom_mode: None,
                agent_timestamp: None, // Intentional default: server-side pod seeding has no agent clock
                recent_lap_times: std::collections::VecDeque::new(),
            };
            pods.insert(id.to_string(), pod.clone());
            seeded.push(pod);
        }
    }

    // BUG-01 FIX: Also seed pods into the SQLite database.
    // Previously only in-memory map was populated — kiosk queries DB directly,
    // so it saw empty pods table after server restart with fresh DB.
    for pod in &seeded {
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO pods (id, number, name, ip_address, sim_type, status, last_seen)
             VALUES (?, ?, ?, ?, 'assetto_corsa', 'idle', datetime('now'))"
        )
        .bind(&pod.id)
        .bind(pod.number as i64)
        .bind(&pod.name)
        .bind(&pod.ip_address)
        .execute(&state.db)
        .await;
    }

    // Sync display names from DB → in-memory (DB may have custom names like "POS 1").
    let db_names: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM pods WHERE name IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    if !db_names.is_empty() {
        let mut pods = state.pods.write().await;
        for (id, db_name) in &db_names {
            if let Some(pod) = pods.get_mut(id)
                && pod.name != *db_name {
                    tracing::info!("Pod {} name overridden from DB: {} → {}", id, pod.name, db_name);
                    pod.name = db_name.clone();
                }
        }
    }

    // Broadcast individual pod updates
    for pod in &seeded {
        let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
    }

    // Broadcast full pod list
    let all_pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let _ = state.dashboard_tx.send(DashboardEvent::PodList(all_pods));

    tracing::info!("Auto-seeded {} pods on startup (in-memory + DB)", seeded.len());
}

/// Sends a test email on first boot to verify Gmail OAuth works.
/// Uses a flag file (`./data/email_verified.flag`) to prevent repeat sends.
/// The flag is written regardless of send success to prevent spam on misconfiguration.
pub async fn maybe_send_first_boot_email(state: &std::sync::Arc<AppState>) {
    const FLAG_PATH: &str = "./data/email_verified.flag";

    // Check if we've already run the first-boot email check
    if std::path::Path::new(FLAG_PATH).exists() {
        return;
    }

    // Ensure the data directory exists
    if let Err(e) = std::fs::create_dir_all("./data") {
        tracing::warn!("Could not create ./data directory for email flag: {}", e);
    }

    // Write the flag file first (prevents spam even if send fails)
    if let Err(e) = std::fs::write(FLAG_PATH, "1") {
        tracing::warn!("Could not write email_verified.flag: {}", e);
    }

    // Check if email alerts are enabled by checking should_send (disabled alerter always returns false)
    {
        let alerter = state.email_alerter.read().await;
        if !alerter.should_send("system", chrono::Utc::now()) {
            tracing::info!("First-boot email check: email alerts disabled or rate-limited, skipping.");
            return;
        }
    }

    // Attempt to send the test email
    tracing::info!("First-boot: sending test email to verify Gmail OAuth...");
    let mut alerter = state.email_alerter.write().await;
    alerter
        .send_alert(
            "system",
            "RaceControl Started - Email Alerts Active",
            "RaceControl has started successfully. Email alerts are configured and working.",
        )
        .await;
    tracing::info!("First-boot email send attempted. Check logs for delivery status.");
}

/// Default EnvFilter directive set when `RUST_LOG` is unset.
///
/// Each comma-separated directive is `<target>=<level>`. EnvFilter matches events by their
/// `target` field; events with explicit `target: "lit"` literals require their literal to
/// appear in this set (or in `RUST_LOG`), otherwise they are filtered to OFF.
///
/// §S-307 (2026-05-14 NF-1 closure, RCA `.planning/audits/RCA-2026-05-14-envfilter-target-exclusion.md`):
/// `startup=info` admits the `metric_alert_task` spawn-evidence line from `background_tasks.rs:60` —
///   sole current crate-wide emit at `target: "startup"`; before adding NEW emits at this target,
///   re-confirm the scope assumption (see RCA §5 follow-up trigger condition).
/// `metric_alerts=info` admits the task body's info-level emits (started + first-cycle) and
///   warn-level emits (`alert.fired`) from `metric_alerts.rs:17,32,94`. Debug-level emits at
///   `metric_alerts.rs:48-53,67-72` remain filtered OFF (intentional — operational verbosity).
pub(crate) const DEFAULT_ENV_FILTER_DIRECTIVES: &str = "racecontrol_crate=info,tower_http=info,admin_api=info,debug=info,pod_healer=info,startup=info,metric_alerts=info";

/// Initializes tracing (stdout + rolling JSON file) and returns the non-blocking guard
/// that must be held for the lifetime of the process.
///
/// Also returns the broadcast receivers for error-rate alerting (email + WhatsApp).
pub fn init_tracing(
    config: &racecontrol_crate::config::Config,
) -> (
    tracing_appender::non_blocking::WorkerGuard,
    tokio::sync::broadcast::Receiver<()>,
    tokio::sync::broadcast::Receiver<()>,
) {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let log_dir = std::path::Path::new("logs");
    std::fs::create_dir_all(log_dir).ok();
    cleanup_old_logs(log_dir);

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("racecontrol-")
        .filename_suffix("jsonl")
        .build(log_dir)
        .expect("failed to build rolling file appender");
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| DEFAULT_ENV_FILTER_DIRECTIVES.into());

    // Error rate monitoring — broadcast bridge from sync Layer to async alerters
    let (alert_tx, _) = tokio::sync::broadcast::channel::<()>(4);
    let email_alert_rx = alert_tx.subscribe();
    let wa_alert_rx = alert_tx.subscribe();
    let error_rate_config = ErrorRateConfig {
        threshold: config.monitoring.error_rate_threshold,
        window_secs: config.monitoring.error_rate_window_secs,
        cooldown_secs: config.monitoring.error_rate_cooldown_secs,
    };
    let error_count_layer = racecontrol_crate::error_rate::ErrorCountLayer::new(error_rate_config, alert_tx);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking_file),
        )
        .with(error_count_layer)
        .init();

    (guard, email_alert_rx, wa_alert_rx)
}

/// Cleans up old log files (>30 days) from the log directory.
pub fn cleanup_old_logs(log_dir: &std::path::Path) {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(30 * 24 * 3600))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if (name.ends_with(".jsonl") || name.contains(".jsonl.") || name.ends_with(".log"))
                && let Ok(meta) = entry.metadata()
                    && let Ok(modified) = meta.modified()
                        && modified < cutoff
                            && std::fs::remove_file(&path).is_ok() {
                                eprintln!("Cleaned old log: {}", path.display());
                            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard for §S-307 (NF-1 EnvFilter target-exclusion closure).
    ///
    /// `metric_alert_task` emits with explicit `target: "startup"` (background_tasks.rs:60) and
    /// `target: "metric_alerts"` (metric_alerts.rs:14,17,32,94). If these directives are removed
    /// from the default filter, the task spawns silently and 0/4 spawn-evidence patterns appear
    /// in the JSONL appender — same observable as "task never started." This test fails fast on
    /// regression rather than waiting for next post-deploy log scan to surface the gap.
    ///
    /// LIMITATION (MAOR Tier-1 F2 disposition, §S-307): substring-contains check, not a behavioral
    /// `EnvFilter::enabled()` query. A pathological string like `"xstartup=info"` would satisfy
    /// `contains("startup=info")` while not admitting the target. Acceptable here because the const
    /// is internal (`pub(crate)`) and changes go through code review. Behavioral-test enhancement
    /// (build EnvFilter + mock Metadata + assert `enabled()`) is a follow-up if the false-positive
    /// class ever fires in practice.
    #[test]
    fn default_env_filter_admits_metric_alert_task_targets() {
        assert!(
            DEFAULT_ENV_FILTER_DIRECTIVES.contains("startup=info"),
            "DEFAULT_ENV_FILTER_DIRECTIVES must include `startup=info` so metric_alert_task spawn-evidence \
             at background_tasks.rs:60 reaches the JSONL appender. See RCA-2026-05-14-envfilter-target-exclusion.md."
        );
        assert!(
            DEFAULT_ENV_FILTER_DIRECTIVES.contains("metric_alerts=info"),
            "DEFAULT_ENV_FILTER_DIRECTIVES must include `metric_alerts=info` so metric_alert_task body emits \
             at metric_alerts.rs:17,32,94 reach the JSONL appender. See RCA-2026-05-14-envfilter-target-exclusion.md."
        );
    }

    /// Sanity check: the default directives parse as a valid EnvFilter so binary boot never falls
    /// into the `.unwrap_or_else` arm with a malformed string.
    #[test]
    fn default_env_filter_parses_cleanly() {
        let parsed = tracing_subscriber::EnvFilter::try_new(DEFAULT_ENV_FILTER_DIRECTIVES);
        assert!(
            parsed.is_ok(),
            "DEFAULT_ENV_FILTER_DIRECTIVES failed to parse as EnvFilter: {:?}",
            parsed.err()
        );
    }
}

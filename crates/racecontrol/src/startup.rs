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
        .unwrap_or_else(|_| "racecontrol_crate=info,tower_http=info,admin_api=info,debug=info,pod_healer=info".into());

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

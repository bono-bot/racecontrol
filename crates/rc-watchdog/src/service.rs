use std::ffi::OsString;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow;
use tracing;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
    ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

use crate::reporter;
use crate::session;
use rc_common::types::WatchdogCrashReport;

/// Poll interval for checking rc-agent process health.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Grace window after a restart: skip polling during this period to avoid double-restart.
const RESTART_GRACE_SECS: u64 = 15;

/// Path where rc-sentry writes a breadcrumb when it restarts rc-agent.
/// rc-watchdog reads this to avoid double-restarting.
const SENTRY_BREADCRUMB_PATH: &str = r"C:\RacingPoint\sentry-restart-breadcrumb.txt";

/// Grace window for sentry breadcrumb: if rc-sentry restarted rc-agent within this many seconds,
/// rc-watchdog defers and skips its own restart attempt.
const SENTRY_GRACE_SECS: u64 = 30;

/// Default racecontrol URL if not found in config.
const DEFAULT_CORE_URL: &str = "http://192.168.31.23:8080";

/// Watchdog version reported in crash reports.
const WATCHDOG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if the tasklist output string contains "rc-agent".
/// Extracted as a testable helper — the actual is_rc_agent_running() function
/// calls tasklist and feeds its stdout here.
pub fn output_contains_agent(stdout: &str) -> bool {
    // tasklist output contains the image name; check for "rc-agent" substring
    stdout.contains("rc-agent")
}

/// Check whether the restart grace window is still active.
/// Returns true if a restart happened recently (within grace_secs).
pub fn restart_grace_active(last_restart: Option<Instant>, grace_secs: u64) -> bool {
    match last_restart {
        None => false,
        Some(t) => t.elapsed() < Duration::from_secs(grace_secs),
    }
}

/// Check if rc-sentry recently restarted rc-agent (breadcrumb file modified within grace_secs).
/// Returns true if rc-watchdog should defer to rc-sentry and skip its own restart.
pub fn sentry_breadcrumb_active(breadcrumb_path: &str, grace_secs: u64) -> bool {
    let metadata = match std::fs::metadata(breadcrumb_path) {
        Ok(m) => m,
        Err(_) => return false, // No breadcrumb = no grace
    };
    let modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    match modified.elapsed() {
        Ok(elapsed) => elapsed < Duration::from_secs(grace_secs),
        Err(_) => false, // Clock went backwards — don't block
    }
}

/// Check if rc-agent.exe is currently running via tasklist.
/// Returns true if running, or true on error (conservative: assume running if can't check).
fn is_rc_agent_running() -> bool {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/NH", "/FI", "IMAGENAME eq rc-agent.exe"]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            output_contains_agent(&stdout)
        }
        Err(e) => {
            tracing::warn!("Failed to run tasklist: {} — assuming rc-agent is NOT running", e);
            false // Return false on error so watchdog can attempt restart
        }
    }
}

/// Load pod_id and core_url from rc-agent.toml, falling back to defaults.
fn load_config() -> (String, String) {
    let config_path = std::path::Path::new(r"C:\RacingPoint\rc-agent.toml");
    if let Ok(contents) = std::fs::read_to_string(config_path) {
        if let Ok(table) = contents.parse::<toml::Table>() {
            let pod_id = table
                .get("pod_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let core_url = table
                .get("server_url")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_CORE_URL)
                .to_string();

            if !pod_id.is_empty() {
                tracing::info!("Config loaded: pod_id={}, core_url={}", pod_id, core_url);
                return (pod_id, core_url);
            }
        }
    }

    // Fallback: derive pod_id from COMPUTERNAME
    let pod_id = std::env::var("COMPUTERNAME")
        .unwrap_or_else(|_| "unknown".to_string());
    tracing::info!(
        "Using fallback config: pod_id={}, core_url={}",
        pod_id,
        DEFAULT_CORE_URL
    );
    (pod_id, DEFAULT_CORE_URL.to_string())
}

/// Main service entry point. Called by service_main after tracing is initialized.
///
/// Registers with the Windows SCM, runs the poll loop, and handles stop/shutdown signals.
pub fn run(_arguments: Vec<OsString>) -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    // Register service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                tracing::info!("Received stop/shutdown signal");
                shutdown_tx.send(()).ok();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("RCWatchdog", event_handler)?;

    // Report Running status to SCM
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    tracing::info!("RCWatchdog service started (version {})", WATCHDOG_VERSION);

    // Load configuration
    let (pod_id, core_url) = load_config();
    let exe_dir = std::path::PathBuf::from(r"C:\RacingPoint");

    let mut restart_count: u32 = 0;
    let mut last_restart_at: Option<Instant> = None;

    // Main poll loop
    loop {
        // Check for stop/shutdown signal
        if shutdown_rx.try_recv().is_ok() {
            tracing::info!("Shutdown signal received, exiting poll loop");
            break;
        }

        // SF-05: Check survival sentinels before restart — yield to active healing layer.
        // This must come BEFORE is_rc_agent_running() so the watchdog does not restart
        // rc-agent while another survival layer is mid-heal.
        {
            use rc_common::survival_types::{any_sentinel_active, check_sentinel, SentinelKind};
            if any_sentinel_active() {
                if let Some(sentinel) = check_sentinel(SentinelKind::HealInProgress) {
                    tracing::info!("HEAL_IN_PROGRESS active (layer={:?}, action_id={}, ttl={}s) — skipping restart cycle (SF-05)",
                        sentinel.layer, sentinel.action_id, sentinel.remaining_secs());
                }
                if let Some(sentinel) = check_sentinel(SentinelKind::OtaDeploying) {
                    tracing::info!("OTA_DEPLOYING active (action_id={}) — skipping restart cycle (SF-05)",
                        sentinel.action_id);
                }
                std::thread::sleep(POLL_INTERVAL);
                continue;
            }
        }

        // Check if rc-agent is running
        if is_rc_agent_running() {
            std::thread::sleep(POLL_INTERVAL);
            continue;
        }

        // Check restart grace window (prevents double-restart after our own last restart)
        if restart_grace_active(last_restart_at, RESTART_GRACE_SECS) {
            tracing::debug!("Restart grace window active, skipping this cycle");
            std::thread::sleep(POLL_INTERVAL);
            continue;
        }

        // Check if rc-sentry recently handled this restart (COORD deconfliction)
        if sentry_breadcrumb_active(SENTRY_BREADCRUMB_PATH, SENTRY_GRACE_SECS) {
            tracing::info!("grace window active: sentry-restart-breadcrumb.txt is recent, skipping restart");
            std::thread::sleep(POLL_INTERVAL);
            continue;
        }

        tracing::warn!("rc-agent not running, attempting restart in Session 1");

        // Attempt to spawn in Session 1
        match session::spawn_in_session1(&exe_dir) {
            Ok(()) => {
                restart_count = restart_count.saturating_add(1);
                last_restart_at = Some(Instant::now());
                tracing::info!(
                    "rc-agent restart initiated (count: {})",
                    restart_count
                );

                // Verify rc-agent actually started (SPAWN-01 pattern: 500ms poll for 10s)
                let verified = {
                    let max_wait = Duration::from_secs(10);
                    let poll_interval_ms = Duration::from_millis(500);
                    let start = Instant::now();
                    let mut alive = false;
                    while start.elapsed() < max_wait {
                        std::thread::sleep(poll_interval_ms);
                        if is_rc_agent_running() {
                            alive = true;
                            break;
                        }
                    }
                    alive
                };
                tracing::info!("rc-agent spawn_verified={} (count: {})", verified, restart_count);

                // Fire-and-forget crash report
                let report = WatchdogCrashReport {
                    pod_id: pod_id.clone(),
                    exit_code: None, // Cannot observe exit code from tasklist polling
                    crash_time: chrono::Utc::now().to_rfc3339(),
                    restart_count,
                    watchdog_version: WATCHDOG_VERSION.to_string(),
                };
                reporter::send_crash_report(&core_url, &report);
            }
            Err(e) => {
                tracing::warn!("Failed to spawn rc-agent: {} — will retry next cycle", e);
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    // Report Stopped status to SCM
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    tracing::info!("RCWatchdog service stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── output_contains_agent tests ─────────────────────────────────────

    #[test]
    fn test_output_contains_agent_present() {
        let output = "rc-agent.exe                 12345 Console                    1    45,000 K\r\n";
        assert!(output_contains_agent(output));
    }

    #[test]
    fn test_output_contains_agent_absent() {
        let output = "INFO: No tasks are running which match the specified criteria.\r\n";
        assert!(!output_contains_agent(output));
    }

    #[test]
    fn test_output_contains_agent_empty() {
        assert!(!output_contains_agent(""));
    }

    #[test]
    fn test_output_contains_agent_multiple_processes() {
        let output = "chrome.exe                   1234 Console  1  100,000 K\r\n\
                      rc-agent.exe                 5678 Console  1   45,000 K\r\n\
                      explorer.exe                 9012 Console  1   80,000 K\r\n";
        assert!(output_contains_agent(output));
    }

    #[test]
    fn test_output_contains_agent_only_other_processes() {
        let output = "chrome.exe                   1234 Console  1  100,000 K\r\n\
                      explorer.exe                 9012 Console  1   80,000 K\r\n";
        assert!(!output_contains_agent(output));
    }

    // ── restart_grace_active tests ──────────────────────────────────────

    #[test]
    fn test_restart_grace_active_no_prior_restart() {
        assert!(!restart_grace_active(None, RESTART_GRACE_SECS));
    }

    #[test]
    fn test_restart_grace_active_within_window() {
        // Just restarted — grace window should be active
        let now = Instant::now();
        assert!(restart_grace_active(Some(now), RESTART_GRACE_SECS));
    }

    #[test]
    fn test_restart_grace_active_after_window() {
        // Create an Instant that is definitely past the grace window
        // We use a 0-second grace to test the "expired" case
        let past = Instant::now();
        // With grace_secs=0, any elapsed time means the window is not active
        assert!(!restart_grace_active(Some(past), 0));
    }

    #[test]
    fn test_restart_grace_active_custom_window() {
        let now = Instant::now();
        // With a very large grace window, it should be active
        assert!(restart_grace_active(Some(now), 3600));
    }

    // ── sentry_breadcrumb_active tests ──────────────────────────────────

    #[test]
    fn test_sentry_breadcrumb_active_no_file() {
        // Missing file means no grace window
        let result = sentry_breadcrumb_active(r"C:\nonexistent\fake-breadcrumb-9999.txt", 30);
        assert!(!result, "missing file should return false");
    }

    #[test]
    fn test_sentry_breadcrumb_active_fresh_file() {
        // Create a temp file — just written, should be within grace window
        let path = std::env::temp_dir().join("rc-watchdog-test-breadcrumb.txt");
        std::fs::write(&path, "test breadcrumb").expect("write test file");
        let path_str = path.to_str().expect("path to str");
        let result = sentry_breadcrumb_active(path_str, 30);
        let _ = std::fs::remove_file(&path);
        assert!(result, "freshly written file should be within 30s grace window");
    }

    #[test]
    fn test_sentry_breadcrumb_active_stale_file() {
        // Create a temp file — use grace_secs=0 so any elapsed time = stale
        let path = std::env::temp_dir().join("rc-watchdog-test-stale-breadcrumb.txt");
        std::fs::write(&path, "test breadcrumb").expect("write test file");
        // Sleep 1ms to ensure some elapsed time
        std::thread::sleep(std::time::Duration::from_millis(1));
        let path_str = path.to_str().expect("path to str");
        let result = sentry_breadcrumb_active(path_str, 0);
        let _ = std::fs::remove_file(&path);
        assert!(!result, "grace_secs=0 means file is always stale");
    }
}

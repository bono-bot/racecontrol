//! James monitor — checks 5 services on James (.27) each Task Scheduler run.
//!
//! Graduated response (persisted across runs via failure_state.json):
//!   failure count 1: log warn, write RecoveryAction::Restart reason=first_failure_wait_retry
//!   failure count 2: attempt restart (comms-link, webterm), log RecoveryAction::Restart
//!   failure count 3+: alert Bono via comms-link WS, log RecoveryAction::AlertStaff
//!   recovered: reset count, log RecoveryAction::Restart reason=recovered

use rc_common::recovery::{
    RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_JAMES,
};

use crate::bono_alert::alert_bono;
use crate::failure_state::FailureState;

const MACHINE: &str = "james";
const HTTP_TIMEOUT_SECS: u64 = 3;

pub struct ServiceConfig {
    pub name: &'static str,
    pub check: ServiceCheck,
    pub restart_cmd: Option<RestartCmd>,
}

pub enum ServiceCheck {
    Http(&'static str),    // URL to GET
    Process(&'static str), // image name substring
}

pub struct RestartCmd {
    pub exe: &'static str,
    pub args: &'static [&'static str],
}

/// Service definitions — 5 services monitored on James (.27).
fn services() -> Vec<ServiceConfig> {
    vec![
        ServiceConfig {
            name: "ollama",
            check: ServiceCheck::Http("http://127.0.0.1:11434"),
            restart_cmd: None, // Ollama is a system service — alert only
        },
        ServiceConfig {
            name: "comms-link",
            check: ServiceCheck::Http("http://127.0.0.1:8766/relay/health"),
            restart_cmd: Some(RestartCmd {
                exe: r"C:\Users\bono\racingpoint\comms-link\start-comms-link.bat",
                args: &[],
            }),
        },
        ServiceConfig {
            name: "kiosk",
            check: ServiceCheck::Http("http://192.168.31.23:3300"),
            restart_cmd: None, // Server-side — alert only
        },
        ServiceConfig {
            name: "webterm",
            check: ServiceCheck::Http("http://127.0.0.1:9999"),
            restart_cmd: Some(RestartCmd {
                exe: r"C:\Program Files\Python311\python.exe",
                args: &[r"C:\Users\bono\racingpoint\deploy-staging\webterm.py"],
            }),
        },
        ServiceConfig {
            name: "claude-code",
            check: ServiceCheck::Process("claude"),
            restart_cmd: None, // User-started — alert only
        },
    ]
}

/// Check if an HTTP endpoint is reachable (non-connection-refused).
/// Returns true on any HTTP response (even error codes), false on connection error/timeout.
pub fn check_service_http(url: &str) -> bool {
    match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
    {
        Ok(client) => client.get(url).send().is_ok(),
        Err(_) => false,
    }
}

/// Check if a process containing `name_fragment` is running via tasklist.
/// Conservative: returns true if tasklist can't be run (assume running).
pub fn check_service_process(name_fragment: &str) -> bool {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/NH"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .to_ascii_lowercase()
                .contains(&name_fragment.to_ascii_lowercase())
        }
        Err(_) => true, // Conservative: assume running if can't check
    }
}

fn is_healthy(svc: &ServiceConfig) -> bool {
    match svc.check {
        ServiceCheck::Http(url) => check_service_http(url),
        ServiceCheck::Process(name) => check_service_process(name),
    }
}

fn attempt_restart(svc: &ServiceConfig) {
    if let Some(ref cmd_spec) = svc.restart_cmd {
        let mut cmd = std::process::Command::new(cmd_spec.exe);
        cmd.args(cmd_spec.args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        match cmd.spawn() {
            Ok(_) => tracing::info!("james_monitor: restart spawned for {}", svc.name),
            Err(e) => tracing::warn!("james_monitor: restart failed for {}: {}", svc.name, e),
        }
    } else {
        tracing::info!(
            "james_monitor: no restart cmd for {} — alert-only service",
            svc.name
        );
    }
}

/// Determine the graduated action for a given failure count.
/// Returns (action, reason) tuple.
/// count=1: log + wait (Restart action with first_failure reason)
/// count=2: attempt restart (Restart action with restart_attempted reason)
/// count>=3: alert Bono (AlertStaff)
pub(crate) fn graduated_action(count: u32) -> (RecoveryAction, String) {
    if count >= 3 {
        (
            RecoveryAction::AlertStaff,
            format!("repeated_failure_count:{}", count),
        )
    } else if count == 2 {
        (
            RecoveryAction::Restart,
            "second_failure_restart_attempted".to_string(),
        )
    } else {
        // count == 1
        (
            RecoveryAction::Restart,
            "first_failure_wait_retry".to_string(),
        )
    }
}

/// Run a single monitoring cycle — called by main() on each Task Scheduler invocation.
/// Checks all 5 services, updates persistent failure state, logs decisions, alerts Bono on 3+.
pub fn run_monitor() {
    tracing::info!("james_monitor: starting check run");
    let logger = RecoveryLogger::new(RECOVERY_LOG_JAMES);
    let mut state = FailureState::load();

    for svc in services() {
        let healthy = is_healthy(&svc);

        if healthy {
            if state.count(svc.name) > 0 {
                tracing::info!(
                    "james_monitor: {} recovered — resetting failure count",
                    svc.name
                );
                let mut d = RecoveryDecision::new(
                    MACHINE,
                    svc.name,
                    RecoveryAuthority::JamesMonitor,
                    RecoveryAction::Restart,
                    "recovered",
                );
                d.context = format!("previous_failures:{}", state.count(svc.name));
                let _ = logger.log(&d);
                state.reset(svc.name);
            }
            continue;
        }

        // Service is DOWN
        state.increment(svc.name);
        let count = state.count(svc.name);
        tracing::warn!(
            "james_monitor: {} DOWN (failure #{}/session)",
            svc.name,
            count
        );

        let (action, reason) = graduated_action(count);

        if action == RecoveryAction::Restart && count == 2 {
            attempt_restart(&svc);
        }

        let mut d = RecoveryDecision::new(
            MACHINE,
            svc.name,
            RecoveryAuthority::JamesMonitor,
            action.clone(),
            &reason,
        );
        d.context = format!("failure_count:{}", count);
        let _ = logger.log(&d);

        if matches!(action, RecoveryAction::AlertStaff) {
            let alert_msg = format!(
                "[WATCHDOG] {} DOWN on James (failure #{}). Check immediately.",
                svc.name, count
            );
            if let Err(e) = alert_bono(&alert_msg) {
                tracing::warn!("james_monitor: bono alert failed: {}", e);
            }
        }
    }

    state.save();
    tracing::info!("james_monitor: check run complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::recovery::RecoveryAction;

    #[test]
    fn test_check_service_http_returns_false_on_unused_port() {
        // Port 59998 should not have anything listening
        let result = check_service_http("http://127.0.0.1:59998");
        assert!(!result, "unused port should return false");
    }

    #[test]
    fn test_check_service_process_found_explorer() {
        // explorer.exe is always running on Windows
        #[cfg(windows)]
        {
            let result = check_service_process("explorer");
            assert!(result, "explorer.exe should always be found on Windows");
        }
        #[cfg(not(windows))]
        {
            // On non-Windows, conservative path: tasklist fails → returns true
            let result = check_service_process("nonexistent_xyz");
            // Result depends on whether tasklist exists — just ensure no panic
            let _ = result;
        }
    }

    #[test]
    fn test_check_service_process_absent() {
        // This process name should never exist
        let result = check_service_process("z_nonexistent_process_9999_xyz");
        assert!(!result, "nonexistent process should return false");
    }

    #[test]
    fn test_graduated_action_count_1_is_restart_first_failure() {
        let (action, reason) = graduated_action(1);
        assert_eq!(action, RecoveryAction::Restart);
        assert!(
            reason.contains("first_failure"),
            "count=1 reason should contain 'first_failure', got: {}",
            reason
        );
    }

    #[test]
    fn test_graduated_action_count_2_is_restart_attempted() {
        let (action, reason) = graduated_action(2);
        assert_eq!(action, RecoveryAction::Restart);
        assert!(
            reason.contains("restart_attempted"),
            "count=2 reason should contain 'restart_attempted', got: {}",
            reason
        );
    }

    #[test]
    fn test_graduated_action_count_3_is_alert_staff() {
        let (action, _reason) = graduated_action(3);
        assert_eq!(action, RecoveryAction::AlertStaff);
    }

    #[test]
    fn test_graduated_action_count_10_is_alert_staff() {
        let (action, _reason) = graduated_action(10);
        assert_eq!(action, RecoveryAction::AlertStaff);
    }
}

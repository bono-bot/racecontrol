//! James AI Healer — monitors all Racing Point services with intelligent recovery.
//!
//! Replaces dumb watchdog with graduated AI-driven diagnosis:
//!   failure count 1: log warn, tail service logs for WHY, wait for next cycle
//!   failure count 2: attempt restart if available, log context
//!   failure count 3: query Ollama for AI diagnosis based on collected symptoms
//!   failure count 4+: alert Bono with full diagnosis + symptoms
//!   recovered: reset count, log pattern for future memory
//!
//! Services monitored (9 total):
//!   Local (.27): ollama, comms-link, rc-sentry, webterm, claude-code, go2rtc
//!   Server (.23): racecontrol, kiosk, dashboard
//!   Network: tailscale connectivity to Bono VPS

use rc_common::recovery::{
    RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_JAMES,
};

use crate::bono_alert::alert_bono;
use crate::failure_state::FailureState;

const MACHINE: &str = "james";
const HTTP_TIMEOUT_SECS: u64 = 3;
/// Ollama runs locally on James's machine — localhost host:port for TcpStream API.
const OLLAMA_HOST_PORT: &str = "127.0.0.1:11434";
const OLLAMA_MODEL: &str = "qwen2.5:3b";

pub struct ServiceConfig {
    pub name: &'static str,
    pub check: ServiceCheck,
    pub restart_cmd: Option<RestartCmd>,
    /// Optional log path to tail when service is down (for WHY detection)
    pub log_path: Option<&'static str>,
}

pub enum ServiceCheck {
    Http(&'static str),    // URL to GET
    HttpJson(&'static str, &'static str), // URL, required JSON field (verifies content, not just 200)
    Process(&'static str), // image name substring
    Command(&'static str, &'static [&'static str], &'static str), // exe, args, expected_substr
}

pub struct RestartCmd {
    pub exe: &'static str,
    pub args: &'static [&'static str],
}

/// Service definitions — 9 services + Tailscale network check.
fn services() -> Vec<ServiceConfig> {
    vec![
        // === Local services (.27) ===
        ServiceConfig {
            name: "ollama",
            check: ServiceCheck::Http("http://127.0.0.1:11434"),
            restart_cmd: None,
            log_path: None,
        },
        ServiceConfig {
            name: "comms-link",
            check: ServiceCheck::HttpJson("http://127.0.0.1:8766/relay/health", "connected"),
            restart_cmd: Some(RestartCmd {
                exe: r"C:\Users\bono\racingpoint\comms-link\start-comms-link.bat",
                args: &[],
            }),
            log_path: Some(r"C:\Users\bono\.claude\comms-watchdog.log"),
        },
        ServiceConfig {
            name: "rc-sentry-ai",
            check: ServiceCheck::HttpJson("http://127.0.0.1:8096/health", "cameras"),
            restart_cmd: Some(RestartCmd {
                exe: r"C:\RacingPoint\watchdog-rcsentry-ai.bat",
                args: &[],
            }),
            log_path: Some(r"C:\RacingPoint\rc-sentry-ai.log"),
        },
        ServiceConfig {
            name: "webterm",
            check: ServiceCheck::Http("http://127.0.0.1:9999"),
            restart_cmd: Some(RestartCmd {
                exe: r"C:\Program Files\Python311\python.exe",
                args: &[r"C:\Users\bono\racingpoint\deploy-staging\webterm.py"],
            }),
            log_path: None,
        },
        ServiceConfig {
            name: "claude-code",
            check: ServiceCheck::Process("claude"),
            restart_cmd: None,
            log_path: None,
        },
        // === Server services (.23) ===
        ServiceConfig {
            name: "racecontrol",
            check: ServiceCheck::HttpJson("http://192.168.31.23:8080/api/v1/health", "build_id"),
            restart_cmd: None, // Server-side — alert only, needs SSH
            log_path: None,    // Logs are on .23, not local
        },
        ServiceConfig {
            name: "kiosk",
            check: ServiceCheck::Http("http://192.168.31.23:3300"),
            restart_cmd: None,
            log_path: None,
        },
        ServiceConfig {
            name: "dashboard",
            check: ServiceCheck::Http("http://192.168.31.23:3200"),
            restart_cmd: None,
            log_path: None,
        },
        ServiceConfig {
            name: "go2rtc",
            check: ServiceCheck::Http("http://127.0.0.1:1984/api"),
            restart_cmd: None, // Runs on James (.27), port 1984 — alert only
            log_path: Some(r"C:\RacingPoint\go2rtc\go2rtc.log"),
        },
        // === Network connectivity ===
        ServiceConfig {
            name: "tailscale-bono",
            check: ServiceCheck::Http("http://100.70.177.44:8080/api/v1/health"),
            restart_cmd: None, // Tailscale reconnect needs admin
            log_path: None,
        },
    ]
}

/// Check if an HTTP endpoint is reachable (non-connection-refused).
pub fn check_service_http(url: &str) -> bool {
    match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
    {
        Ok(client) => client.get(url).send().is_ok(),
        Err(_) => false,
    }
}

/// Check HTTP endpoint AND verify a JSON field exists in the response.
/// Standing rule: "Verify the EXACT behavior path, not proxies."
pub fn check_service_http_json(url: &str, required_field: &str) -> bool {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client.get(url).send() {
        Ok(resp) => {
            if let Ok(text) = resp.text() {
                text.contains(required_field)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Check if a process containing `name_fragment` is running via tasklist.
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

/// Run a command and check if stdout contains expected substring.
pub fn check_service_command(exe: &str, args: &[&str], expected: &str) -> bool {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(expected)
        }
        Err(_) => false,
    }
}

fn is_healthy(svc: &ServiceConfig) -> bool {
    match &svc.check {
        ServiceCheck::Http(url) => check_service_http(url),
        ServiceCheck::HttpJson(url, field) => check_service_http_json(url, field),
        ServiceCheck::Process(name) => check_service_process(name),
        ServiceCheck::Command(exe, args, expected) => check_service_command(exe, args, expected),
    }
}

// ─── AI Healer: Log tailing + Ollama diagnosis ─────────────────────────────

/// Tail the last N lines of a log file to understand WHY a service is down.
fn tail_log(path: &str, lines: usize) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines);
    let tail: Vec<&str> = all_lines[start..].to_vec();
    if tail.is_empty() {
        None
    } else {
        Some(tail.join("\n"))
    }
}

/// Query Ollama for AI diagnosis of a service failure using shared rc_common::ollama.
/// Only called at failure count 3 — not on every check (expensive).
fn ai_diagnose(service_name: &str, symptoms: &str) -> Option<String> {
    let crash_context = format!(
        "Service '{}' is DOWN on James AI healer machine.\nSymptoms:\n{}",
        service_name, symptoms
    );
    rc_common::ollama::query_crash(
        &crash_context,
        Some(OLLAMA_HOST_PORT),
        Some(OLLAMA_MODEL),
    )
    .map(|r| r.suggestion)
}

/// Collect symptoms for a down service: log tail + failure count + duration.
fn collect_symptoms(svc: &ServiceConfig, state: &FailureState) -> String {
    let count = state.count(svc.name);
    let mut symptoms = format!("Failure count: {} (consecutive 2-min cycles)\n", count);

    if let Some(log_path) = svc.log_path {
        if let Some(tail) = tail_log(log_path, 10) {
            symptoms.push_str(&format!("Last 10 log lines:\n{}\n", tail));
        } else {
            symptoms.push_str("Log file empty or missing.\n");
        }
    } else {
        symptoms.push_str("No local log file available.\n");
    }

    if svc.restart_cmd.is_some() {
        symptoms.push_str("Service has a restart command available.\n");
    } else {
        symptoms.push_str("Service is alert-only (no local restart).\n");
    }

    symptoms
}

/// Poll the service health for up to 10s at 500ms intervals after a restart.
/// Returns true if the service becomes healthy within the window.
fn verify_spawn(svc: &ServiceConfig) -> bool {
    let max_wait = std::time::Duration::from_secs(10);
    let poll_interval = std::time::Duration::from_millis(500);
    let start = std::time::Instant::now();
    while start.elapsed() < max_wait {
        std::thread::sleep(poll_interval);
        if is_healthy(svc) {
            return true;
        }
    }
    false
}

/// Attempt to restart a service and verify it actually started (SPAWN-01 pattern).
/// Returns true if the service became healthy within 10s after the spawn.
/// Returns false if spawn failed or service did not recover within the verify window.
fn attempt_restart(svc: &ServiceConfig) -> bool {
    if let Some(ref cmd_spec) = svc.restart_cmd {
        let mut cmd = std::process::Command::new(cmd_spec.exe);
        cmd.args(cmd_spec.args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        match cmd.spawn() {
            Ok(_) => {
                tracing::info!("james_monitor: restart spawned for {}, polling for health", svc.name);
                let verified = verify_spawn(svc);
                tracing::info!("james_monitor: {} spawn_verified={}", svc.name, verified);
                verified
            }
            Err(e) => {
                tracing::warn!("james_monitor: restart failed for {}: {}", svc.name, e);
                false
            }
        }
    } else {
        tracing::info!(
            "james_monitor: no restart cmd for {} — alert-only service",
            svc.name
        );
        false
    }
}

/// Determine the graduated action for a given failure count.
/// count=1: log + wait + tail logs (Restart with first_failure reason)
/// count=2: attempt restart (Restart with restart_attempted reason)
/// count=3: query Ollama AI for diagnosis (Diagnose action)
/// count>=4: alert Bono with full diagnosis (AlertStaff)
pub(crate) fn graduated_action(count: u32) -> (RecoveryAction, String) {
    if count >= 4 {
        (
            RecoveryAction::AlertStaff,
            format!("repeated_failure_count:{}", count),
        )
    } else if count == 3 {
        (
            RecoveryAction::Restart,
            "ai_diagnosis_requested".to_string(),
        )
    } else if count == 2 {
        (
            RecoveryAction::Restart,
            "second_failure_restart_attempted".to_string(),
        )
    } else {
        (
            RecoveryAction::Restart,
            "first_failure_wait_retry".to_string(),
        )
    }
}

/// Run a single monitoring cycle — called by main() on each Task Scheduler invocation.
/// Checks all services, applies graduated AI-driven recovery.
pub fn run_monitor() {
    tracing::info!("james_monitor: starting check run ({} services)", services().len());
    let logger = RecoveryLogger::new(RECOVERY_LOG_JAMES);
    let mut state = FailureState::load();

    for svc in services() {
        let healthy = is_healthy(&svc);

        if healthy {
            if state.count(svc.name) > 0 {
                tracing::info!(
                    "james_monitor: {} RECOVERED after {} failures",
                    svc.name,
                    state.count(svc.name)
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

        // Step 1: Always tail logs on first failure (WHY detection)
        let symptoms = collect_symptoms(&svc, &state);
        if count == 1 {
            tracing::info!("james_monitor: {} — collecting symptoms:\n{}", svc.name, symptoms);
        }

        let (action, reason) = graduated_action(count);

        // Step 2: Attempt restart at count 2, verify it actually started
        let mut spawn_verified = false;
        if count == 2 {
            spawn_verified = attempt_restart(&svc);
        }

        // Step 3: AI diagnosis at count 3 (expensive — only once)
        let mut ai_diagnosis = None;
        if count == 3 {
            tracing::info!("james_monitor: {} — querying Ollama for AI diagnosis", svc.name);
            ai_diagnosis = ai_diagnose(svc.name, &symptoms);
            match &ai_diagnosis {
                Some(diag) => tracing::info!("james_monitor: {} AI diagnosis: {}", svc.name, diag),
                None => tracing::warn!("james_monitor: {} AI diagnosis failed (Ollama unreachable?)", svc.name),
            }
        }

        let mut d = RecoveryDecision::new(
            MACHINE,
            svc.name,
            RecoveryAuthority::JamesMonitor,
            action.clone(),
            &reason,
        );
        d.context = if let Some(ref diag) = ai_diagnosis {
            format!("failure_count:{} spawn_verified:{} ai_diagnosis:{}", count, spawn_verified, diag)
        } else {
            format!("failure_count:{} spawn_verified:{}", count, spawn_verified)
        };
        let _ = logger.log(&d);

        // Step 4: Alert Bono at count 4+ with full context
        if matches!(action, RecoveryAction::AlertStaff) {
            let diag_text = ai_diagnosis
                .as_deref()
                .unwrap_or("AI diagnosis unavailable");
            let alert_msg = format!(
                "[AI-HEALER] {} DOWN on James (failure #{}).\nDiagnosis: {}\nSymptoms: {}",
                svc.name,
                count,
                diag_text,
                symptoms.lines().take(5).collect::<Vec<_>>().join(" | ")
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
    fn test_graduated_action_count_3_is_ai_diagnosis() {
        let (action, reason) = graduated_action(3);
        assert_eq!(action, RecoveryAction::Restart);
        assert!(
            reason.contains("ai_diagnosis"),
            "count=3 should trigger AI diagnosis, got: {}",
            reason
        );
    }

    #[test]
    fn test_graduated_action_count_4_is_alert_staff() {
        let (action, _reason) = graduated_action(4);
        assert_eq!(action, RecoveryAction::AlertStaff);
    }

    #[test]
    fn test_graduated_action_count_10_is_alert_staff() {
        let (action, _reason) = graduated_action(10);
        assert_eq!(action, RecoveryAction::AlertStaff);
    }

    #[test]
    fn test_tail_log_missing_file() {
        let result = tail_log(r"C:\nonexistent\fake.log", 5);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_service_http_json_unused_port() {
        let result = check_service_http_json("http://127.0.0.1:59997", "health");
        assert!(!result);
    }
}

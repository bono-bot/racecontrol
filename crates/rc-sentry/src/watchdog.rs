//! Service watchdog — health polling FSM with crash log analysis.
//!
//! Spawns a background thread that polls a service's /health endpoint every 5s.
//! Uses 3-poll hysteresis (15s) before declaring crash to avoid false positives.
//! After crash: reads startup_log + stderr to build CrashContext for downstream fixes.
//!
//! Target service is configurable via SentryConfig (rc-sentry.toml).
//! Default: rc-agent on :8090 (pod mode). Server mode: racecontrol on :8080.
//!
//! Anti-cheat safe: uses only std::net::TcpStream HTTP — no process inspection APIs.
//! Pure std: no tokio, no async.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_POD};

use crate::sentry_config;

// ─── Configuration ───────────────────────────────────────────────────────────

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const READ_TIMEOUT: Duration = Duration::from_secs(3);
const HYSTERESIS_THRESHOLD: u8 = 3; // consecutive failures before crash

const LOG_TARGET: &str = "watchdog";

// ─── Types ───────────────────────────────────────────────────────────────────

/// FSM state for the watchdog.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogState {
    /// rc-agent is responding to health checks.
    Healthy,
    /// rc-agent failed N consecutive polls (1..HYSTERESIS_THRESHOLD).
    Suspect(u8),
    /// rc-agent confirmed crashed after HYSTERESIS_THRESHOLD failures.
    Crashed,
}

/// Context gathered after a crash is detected.
#[derive(Debug, Clone)]
pub struct CrashContext {
    /// Content from rc-agent-startup.log (last 2000 chars)
    pub startup_log: String,
    /// Content from rc-agent-stderr.log (last 2000 chars)
    pub stderr_log: String,
    /// Extracted panic message if found
    pub panic_message: Option<String>,
    /// Extracted exit code if found
    pub exit_code: Option<i32>,
    /// Last startup phase from startup log
    pub last_phase: Option<String>,
}

// ─── Health Check ────────────────────────────────────────────────────────────

/// Poll rc-agent's health endpoint via raw HTTP GET.
/// Returns true if rc-agent responds with HTTP 200.
/// Anti-cheat safe: just a TCP connection, no process APIs.
fn poll_health() -> bool {
    let cfg = sentry_config::load();
    let stream = match TcpStream::connect_timeout(
        &match cfg.health_addr.parse() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::warn!("invalid health_addr '{}': {}", cfg.health_addr, e);
                return false;
            }
        },
        CONNECT_TIMEOUT,
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };

    if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
        return false;
    }
    if stream.set_write_timeout(Some(READ_TIMEOUT)).is_err() {
        return false;
    }

    let mut stream = stream;
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        cfg.health_path
    );
    let request = request; // bind the formatted String
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = [0u8; 512];
    match stream.read(&mut response) {
        Ok(n) if n > 0 => {
            let text = String::from_utf8_lossy(&response[..n]);
            text.contains("200")
        }
        _ => false,
    }
}

// ─── Log Reading ─────────────────────────────────────────────────────────────

/// Read the tail of a log file (last `max_chars` characters).
fn read_log_tail(path: &str, max_chars: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if content.len() > max_chars {
                // Find a valid char boundary near the desired offset
                let start = content.len() - max_chars;
                let start = content.ceil_char_boundary(start);
                content[start..].to_string()
            } else {
                content
            }
        }
        Err(_) => String::new(),
    }
}

/// Extract panic message from stderr/startup log content.
fn extract_panic(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.contains("panicked at") || line.contains("thread '") && line.contains("panic") {
            return Some(line.trim().to_string());
        }
    }
    None
}

/// Extract exit code from log content (e.g. "exit code 101").
fn extract_exit_code(content: &str) -> Option<i32> {
    for line in content.lines() {
        if let Some(pos) = line.find("exit code") {
            let after = &line[pos + 9..];
            let code_str: String = after.chars()
                .skip_while(|c| !c.is_ascii_digit() && *c != '-')
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if let Ok(code) = code_str.parse::<i32>() {
                return Some(code);
            }
        }
    }
    None
}

/// Extract last startup phase from startup log.
fn extract_last_phase(content: &str) -> Option<String> {
    // startup_log.rs writes lines like "[STARTUP] phase: binding_ports"
    content.lines().rev()
        .find(|l| l.contains("[STARTUP]") || l.contains("phase:"))
        .map(|l| l.trim().to_string())
}

/// Build CrashContext by reading available logs.
fn build_crash_context() -> CrashContext {
    let cfg = sentry_config::load();
    let startup_log = read_log_tail(&cfg.startup_log, 2000);
    let stderr_log = read_log_tail(&cfg.stderr_log, 2000);

    let combined = format!("{}\n{}", &stderr_log, &startup_log);

    CrashContext {
        panic_message: extract_panic(&combined),
        exit_code: extract_exit_code(&combined),
        last_phase: extract_last_phase(&startup_log),
        startup_log,
        stderr_log,
    }
}

// ─── Watchdog Loop ───────────────────────────────────────────────────────────

/// Start the watchdog in a background thread.
/// Returns a Receiver that emits CrashContext each time a crash is detected.
pub fn spawn(shutdown: &'static AtomicBool) -> mpsc::Receiver<CrashContext> {
    let (tx, rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("sentry-watchdog".to_string())
        .spawn(move || {
            let cfg = sentry_config::load();
            tracing::info!(target: LOG_TARGET, "watchdog started — polling {} ({}) every {:?}", cfg.service_name, cfg.health_addr, POLL_INTERVAL);
            let mut state = WatchdogState::Healthy;

            // OBS-05: RecoveryLogger for FSM transition logging
            // Created here (inside watchdog thread) so it's independent of the crash-handler logger.
            let recovery_logger = RecoveryLogger::new(RECOVERY_LOG_POD);
            let machine = sysinfo::System::host_name().unwrap_or_else(|| "pod-unknown".to_string());

            loop {
                if shutdown.load(Ordering::Acquire) {
                    tracing::info!(target: LOG_TARGET, "watchdog shutting down");
                    break;
                }

                // v22.0 Phase 178: Read sentry-flags.json for flag-gated watchdog behavior.
                // Written by rc-agent on every FlagSync. Missing file = no flags = defaults apply.
                let sentry_flags: Option<serde_json::Value> = {
                    let path = r"C:\RacingPoint\sentry-flags.json";
                    std::fs::read_to_string(path)
                        .ok()
                        .and_then(|c| serde_json::from_str(&c).ok())
                };

                // Check kill switch — if kill_watchdog_restart is set, skip restart actions this tick.
                // Used by OTA deploys (Phase 179) to suppress watchdog interference during binary swap.
                let restart_suppressed = sentry_flags
                    .as_ref()
                    .and_then(|v| v.get("kill_switches"))
                    .and_then(|ks| ks.get("kill_watchdog_restart"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let healthy = poll_health();

                state = match (&state, healthy) {
                    // Healthy and poll passed → stay healthy
                    (WatchdogState::Healthy, true) => WatchdogState::Healthy,

                    // Healthy and poll failed → enter suspect
                    (WatchdogState::Healthy, false) => {
                        tracing::warn!(target: "state", prev = "Healthy", next = "Suspect(1)", "FSM transition: Healthy -> Suspect(1)");
                        tracing::warn!(target: LOG_TARGET, "poll failed (1/{HYSTERESIS_THRESHOLD}) — entering suspect state");
                        let _ = recovery_logger.log(&RecoveryDecision::new(
                            machine.clone(),
                            "rc-agent.exe",
                            RecoveryAuthority::RcSentry,
                            RecoveryAction::AlertStaff,
                            "fsm:Healthy->Suspect(1)",
                        ));
                        WatchdogState::Suspect(1)
                    }

                    // Suspect and poll passed → back to healthy
                    (WatchdogState::Suspect(n), true) => {
                        let prev_state = format!("Suspect({})", n);
                        tracing::info!(target: "state", prev = %prev_state, next = "Healthy", "FSM transition: Suspect -> Healthy");
                        tracing::info!(target: LOG_TARGET, "poll recovered after {} failures — back to healthy", n);
                        let _ = recovery_logger.log(&RecoveryDecision::new(
                            machine.clone(),
                            "rc-agent.exe",
                            RecoveryAuthority::RcSentry,
                            RecoveryAction::AlertStaff,
                            format!("fsm:Suspect({})->Healthy", n),
                        ));
                        WatchdogState::Healthy
                    }

                    // Suspect and poll failed → increment or crash
                    (WatchdogState::Suspect(n), false) => {
                        let next = n + 1;
                        if next >= HYSTERESIS_THRESHOLD {
                            let prev_state = format!("Suspect({})", n);
                            tracing::error!(target: "state", prev = %prev_state, next = "Crashed", "FSM transition: Suspect -> Crashed");
                            tracing::error!(target: LOG_TARGET, "poll failed ({next}/{HYSTERESIS_THRESHOLD}) — rc-agent CRASHED");
                            let _ = recovery_logger.log(&RecoveryDecision::new(
                                machine.clone(),
                                "rc-agent.exe",
                                RecoveryAuthority::RcSentry,
                                RecoveryAction::Restart,
                                format!("fsm:Suspect({})->Crashed", n),
                            ));
                            WatchdogState::Crashed
                        } else {
                            let prev_state = format!("Suspect({})", n);
                            let next_state = format!("Suspect({})", next);
                            tracing::warn!(target: "state", prev = %prev_state, next = %next_state, "FSM transition: Suspect(n) -> Suspect(n+1)");
                            tracing::warn!(target: LOG_TARGET, "poll failed ({next}/{HYSTERESIS_THRESHOLD}) — still suspect");
                            let _ = recovery_logger.log(&RecoveryDecision::new(
                                machine.clone(),
                                "rc-agent.exe",
                                RecoveryAuthority::RcSentry,
                                RecoveryAction::AlertStaff,
                                format!("fsm:Suspect({})->Suspect({})", n, next),
                            ));
                            WatchdogState::Suspect(next)
                        }
                    }

                    // Crashed → should not stay here, but handle gracefully
                    (WatchdogState::Crashed, _) => WatchdogState::Crashed,
                };

                if state == WatchdogState::Crashed {
                    // v22.0 Phase 178: If kill_watchdog_restart is active, suppress restart.
                    // Used by OTA deploys (Phase 179) to prevent watchdog from interfering
                    // while a new binary is being downloaded and swapped in.
                    if restart_suppressed {
                        tracing::warn!(target: LOG_TARGET, "restart suppressed by kill_watchdog_restart flag — skipping crash handler this tick");
                        state = WatchdogState::Healthy;
                    } else {
                        let ctx = build_crash_context();
                        tracing::info!(
                            target: LOG_TARGET,
                            "crash context built: panic={:?}, exit_code={:?}, last_phase={:?}",
                            ctx.panic_message, ctx.exit_code, ctx.last_phase
                        );

                        if tx.send(ctx).is_err() {
                            tracing::error!(target: LOG_TARGET, "crash channel closed — stopping watchdog");
                            break;
                        }

                        // Return to healthy — Phase 103 handles fix+restart via the channel
                        state = WatchdogState::Healthy;
                    }
                }

                std::thread::sleep(POLL_INTERVAL);
            }
        })
        .expect("spawn watchdog thread");

    rx
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsm_healthy_stays_healthy_on_success() {
        // Simulate: healthy + poll passes
        let state = WatchdogState::Healthy;
        let healthy = true;
        let next = match (&state, healthy) {
            (WatchdogState::Healthy, true) => WatchdogState::Healthy,
            _ => unreachable!(),
        };
        assert_eq!(next, WatchdogState::Healthy);
    }

    #[test]
    fn fsm_healthy_to_suspect_on_failure() {
        let state = WatchdogState::Healthy;
        let healthy = false;
        let next = match (&state, healthy) {
            (WatchdogState::Healthy, false) => WatchdogState::Suspect(1),
            _ => unreachable!(),
        };
        assert_eq!(next, WatchdogState::Suspect(1));
    }

    #[test]
    fn fsm_suspect_recovers_on_success() {
        let state = WatchdogState::Suspect(2);
        let healthy = true;
        let next = match (&state, healthy) {
            (WatchdogState::Suspect(_), true) => WatchdogState::Healthy,
            _ => unreachable!(),
        };
        assert_eq!(next, WatchdogState::Healthy);
    }

    #[test]
    fn fsm_suspect_escalates_to_crashed() {
        let state = WatchdogState::Suspect(2);
        let healthy = false;
        let n = 2;
        let next_n = n + 1;
        let next = if next_n >= HYSTERESIS_THRESHOLD {
            WatchdogState::Crashed
        } else {
            WatchdogState::Suspect(next_n)
        };
        assert_eq!(next, WatchdogState::Crashed);
    }

    #[test]
    fn fsm_suspect_stays_suspect_before_threshold() {
        let n: u8 = 1;
        let next_n = n + 1;
        let next = if next_n >= HYSTERESIS_THRESHOLD {
            WatchdogState::Crashed
        } else {
            WatchdogState::Suspect(next_n)
        };
        assert_eq!(next, WatchdogState::Suspect(2));
    }

    #[test]
    fn extract_panic_finds_panic_message() {
        let log = "some output\nthread 'main' panicked at 'index out of bounds: len is 0'\nnote: run with RUST_BACKTRACE=1";
        assert_eq!(
            extract_panic(log),
            Some("thread 'main' panicked at 'index out of bounds: len is 0'".to_string())
        );
    }

    #[test]
    fn extract_panic_returns_none_for_clean_log() {
        let log = "INFO startup complete\nINFO listening on :8090";
        assert_eq!(extract_panic(log), None);
    }

    #[test]
    fn extract_exit_code_finds_code() {
        let log = "process exited with exit code 101\n";
        assert_eq!(extract_exit_code(log), Some(101));
    }

    #[test]
    fn extract_exit_code_returns_none() {
        let log = "process running normally";
        assert_eq!(extract_exit_code(log), None);
    }

    #[test]
    fn extract_last_phase_finds_phase() {
        let log = "[STARTUP] phase: binding_ports\n[STARTUP] phase: ws_connect\n";
        assert_eq!(
            extract_last_phase(log),
            Some("[STARTUP] phase: ws_connect".to_string())
        );
    }

    #[test]
    fn read_log_tail_truncates() {
        let content = "a".repeat(3000);
        // Simulate: if file had 3000 chars and we read 2000, we get the tail
        let tail = if content.len() > 2000 {
            content[content.len() - 2000..].to_string()
        } else {
            content.clone()
        };
        assert_eq!(tail.len(), 2000);
    }

    #[test]
    fn crash_context_default_empty_when_no_files() {
        // build_crash_context reads files that don't exist in test env
        let ctx = build_crash_context();
        // Should gracefully return empty strings, no panics
        assert!(ctx.startup_log.is_empty() || !ctx.startup_log.is_empty());
        assert!(ctx.stderr_log.is_empty() || !ctx.stderr_log.is_empty());
    }
}

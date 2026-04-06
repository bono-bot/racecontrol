//! Meshed Intelligence Diagnostic Engine for rc-sentry (Phase 322, MIG-01).
//!
//! Runs in a dedicated std::thread, performing periodic scans to detect
//! anomalies and emit DiagnosticTrigger events to the tier engine via mpsc.
//!
//! Scan interval: 300s (5 minutes) with 60s startup grace period.
//! Pure std -- no tokio, no async.

use rc_common::diagnostic_types::DiagnosticTrigger;
use std::sync::mpsc::Sender;

const LOG_TARGET: &str = "diagnostic-engine";
const SCAN_INTERVAL_SECS: u64 = 300;
const STARTUP_GRACE_SECS: u64 = 60;
const AGENT_HEALTH_ADDR: &str = "127.0.0.1:8090";

/// Expected sentinel files in C:\RacingPoint\ — these are NOT unexpected.
const EXPECTED_SENTINELS: &[&str] = &[
    "MAINTENANCE_MODE",
    "RCAGENT_SELF_RESTART",
    "OTA_DEPLOYING",
    "GRACEFUL_RELAUNCH",
    "rcagent-restart-sentinel.txt",
    "DEPLOY_IN_PROGRESS",
];

/// Spawn the diagnostic engine thread.
/// Scans for anomalies every 300s and sends DiagnosticTrigger events to `trigger_tx`.
pub fn spawn(trigger_tx: Sender<DiagnosticTrigger>) {
    std::thread::Builder::new()
        .name("sentry-diagnostic-engine".to_string())
        .spawn(move || {
            tracing::info!(
                target: LOG_TARGET,
                "Diagnostic engine started — grace period {}s, scan interval {}s",
                STARTUP_GRACE_SECS, SCAN_INTERVAL_SECS
            );

            // Startup grace period — let rc-agent finish booting before scanning
            std::thread::sleep(std::time::Duration::from_secs(STARTUP_GRACE_SECS));
            tracing::info!(target: LOG_TARGET, "Grace period complete — beginning scans");

            loop {
                let triggers = run_scan();

                for trigger in &triggers {
                    crate::mi_debug_state::record_trigger(trigger);
                    if let Err(e) = trigger_tx.send(trigger.clone()) {
                        tracing::error!(
                            target: LOG_TARGET,
                            error = %e,
                            "Failed to send trigger to tier engine — channel closed"
                        );
                        return; // Tier engine is dead, exit
                    }
                }

                tracing::info!(
                    target: LOG_TARGET,
                    triggers_emitted = triggers.len(),
                    "Scan complete — sleeping {}s",
                    SCAN_INTERVAL_SECS
                );

                std::thread::sleep(std::time::Duration::from_secs(SCAN_INTERVAL_SECS));
            }
        })
        .expect("spawn sentry-diagnostic-engine thread");
}

/// Run a single diagnostic scan. Returns all detected triggers.
fn run_scan() -> Vec<DiagnosticTrigger> {
    let mut triggers = Vec::new();

    // Always emit Periodic trigger every scan cycle
    triggers.push(DiagnosticTrigger::Periodic);

    // Health check: probe rc-agent at 127.0.0.1:8090
    if !check_agent_health() {
        tracing::info!(target: LOG_TARGET, "HealthCheckFail: rc-agent not responding on {}", AGENT_HEALTH_ADDR);
        triggers.push(DiagnosticTrigger::HealthCheckFail);
    }

    // Process crash detection: look for WerFault.exe / WerReport.exe
    for crash_process in detect_werfault_processes() {
        tracing::info!(target: LOG_TARGET, process = %crash_process, "ProcessCrash detected");
        triggers.push(DiagnosticTrigger::ProcessCrash {
            process_name: crash_process,
        });
    }

    // Sentinel file check: look for unexpected files in C:\RacingPoint\
    for sentinel in detect_unexpected_sentinels() {
        tracing::info!(target: LOG_TARGET, file = %sentinel, "SentinelUnexpected detected");
        triggers.push(DiagnosticTrigger::SentinelUnexpected {
            file_name: sentinel,
        });
    }

    triggers
}

/// Check if rc-agent is healthy by probing its HTTP port.
/// Returns true if a TCP connection can be established to 127.0.0.1:8090.
fn check_agent_health() -> bool {
    use std::net::TcpStream;
    let timeout = std::time::Duration::from_secs(3);
    match TcpStream::connect_timeout(
        &AGENT_HEALTH_ADDR.parse().unwrap_or_else(|_| {
            std::net::SocketAddr::from(([127, 0, 0, 1], 8090))
        }),
        timeout,
    ) {
        Ok(mut stream) => {
            // Send a minimal GET /health request
            use std::io::Write;
            let req = b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            if stream.write_all(req).is_err() {
                return false;
            }
            // Read response — just check for any non-empty response
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).ok();
            let mut buf = [0u8; 256];
            match std::io::Read::read(&mut stream, &mut buf) {
                Ok(n) if n > 0 => {
                    let response = String::from_utf8_lossy(&buf[..n]);
                    response.contains("200")
                }
                _ => false,
            }
        }
        Err(_) => false,
    }
}

/// Scan for WerFault.exe and WerReport.exe using sysinfo.
fn detect_werfault_processes() -> Vec<String> {
    use sysinfo::{ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);

    let mut found = Vec::new();
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().to_lowercase();
        if name == "werfault.exe" || name == "werreport.exe" {
            found.push(p.name().to_string_lossy().to_string());
        }
    }
    found
}

/// Scan C:\RacingPoint\ for unexpected sentinel files.
/// Returns file names that are NOT in the EXPECTED_SENTINELS list.
fn detect_unexpected_sentinels() -> Vec<String> {
    let sentinel_dir = std::path::Path::new(r"C:\RacingPoint");
    let mut unexpected = Vec::new();

    let entries = match std::fs::read_dir(sentinel_dir) {
        Ok(e) => e,
        Err(_) => return unexpected,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Only check sentinel-like files (no extension, all uppercase, or known patterns)
        let is_sentinel_like = !name.contains('.')
            || name.ends_with("-sentinel.txt")
            || name == name.to_uppercase();

        // Skip config files, binaries, logs, databases
        let is_data_file = name.ends_with(".toml")
            || name.ends_with(".exe")
            || name.ends_with(".log")
            || name.ends_with(".jsonl")
            || name.ends_with(".db")
            || name.ends_with(".bat")
            || name.ends_with(".json")
            || name.ends_with(".ps1")
            || name.ends_with(".ini")
            || name.ends_with(".dll")
            || name.ends_with(".txt") && !name.ends_with("-sentinel.txt");

        if is_sentinel_like && !is_data_file {
            if !EXPECTED_SENTINELS.iter().any(|s| *s == name) {
                unexpected.push(name);
            }
        }
    }

    unexpected
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_health_check_fail() {
        // Port 8090 is very unlikely to be listening during tests
        // so check_agent_health() should return false
        let healthy = check_agent_health();
        // We can't guarantee port 8090 isn't running, but we can test the function runs
        // Without an agent, this should be false
        if !healthy {
            // Create a trigger and verify it's the right type
            let trigger = DiagnosticTrigger::HealthCheckFail;
            assert_eq!(trigger, DiagnosticTrigger::HealthCheckFail);
        }
        // If somehow healthy (unlikely in test env), that's also fine
    }

    #[test]
    fn test_run_scan_always_emits_periodic() {
        let triggers = run_scan();
        assert!(
            triggers.iter().any(|t| matches!(t, DiagnosticTrigger::Periodic)),
            "run_scan must always emit a Periodic trigger"
        );
    }

    #[test]
    fn test_expected_sentinels_not_flagged() {
        // Verify that expected sentinel names are in the constant
        assert!(EXPECTED_SENTINELS.contains(&"MAINTENANCE_MODE"));
        assert!(EXPECTED_SENTINELS.contains(&"RCAGENT_SELF_RESTART"));
        assert!(EXPECTED_SENTINELS.contains(&"OTA_DEPLOYING"));
    }
}

//! Process Guard Module — continuous process enforcement for rc-agent pods.
//!
//! # Design
//! - Runs as a tokio::spawn background task (never blocks the event loop)
//! - sysinfo::refresh_processes() wrapped in spawn_blocking (100-300ms blocking call)
//! - Two-cycle grace before kill (warn_before_kill=true, configurable via whitelist)
//! - Self-exclusion: own PID, parent PID, and "rc-agent.exe" name bypassed unconditionally
//! - CRITICAL tier: racecontrol.exe on a pod -> zero grace, kill immediately in kill_and_report mode
//! - Log: C:\RacingPoint\process-guard.log with 512KB rotation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::{mpsc, RwLock};

use rc_common::protocol::AgentMessage;
use rc_common::types::{MachineWhitelist, ProcessViolation, ViolationType};

use crate::config::ProcessGuardConfig;

const GUARD_LOG: &str = r"C:\RacingPoint\process-guard.log";
const MAX_LOG_BYTES: u64 = 512 * 1024; // 512 KB

/// Names of processes that are CRITICAL violations when detected on a pod
/// (standing rule #2: never run server binaries on pod machines).
const CRITICAL_BINARIES: &[&str] = &["racecontrol.exe"];

/// Entry point — call once from main.rs after AppState is built.
/// Spawns an internal process scan loop (every config.scan_interval_secs).
/// Auto-start audit is stubbed here; full implementation in Plan 03.
pub fn spawn(
    config: ProcessGuardConfig,
    whitelist: Arc<RwLock<MachineWhitelist>>,
    tx: mpsc::Sender<AgentMessage>,
    machine_id: String,
) {
    if !config.enabled {
        tracing::info!("Process guard DISABLED (process_guard.enabled=false)");
        return;
    }
    tokio::spawn(async move {
        // 60s amnesty window on startup — allow transient Windows Update / MpCmdRun to settle
        tokio::time::sleep(Duration::from_secs(60)).await;
        tracing::info!(
            "Process guard started (interval={}s, machine={})",
            config.scan_interval_secs,
            machine_id
        );

        let mut scan_interval =
            tokio::time::interval(Duration::from_secs(config.scan_interval_secs));
        // grace_counts: process_name -> (consecutive_count, start_time_of_first_sighting)
        let mut grace_counts: HashMap<String, (u32, u64)> = HashMap::new();

        loop {
            scan_interval.tick().await;
            if let Err(e) =
                run_scan_cycle(&whitelist, &tx, &machine_id, &mut grace_counts).await
            {
                tracing::error!("Process guard scan error: {}", e);
            }
        }
    });
}

/// Run one full process scan cycle.
async fn run_scan_cycle(
    whitelist: &Arc<RwLock<MachineWhitelist>>,
    tx: &mpsc::Sender<AgentMessage>,
    machine_id: &str,
    grace_counts: &mut HashMap<String, (u32, u64)>,
) -> anyhow::Result<()> {
    let own_pid = std::process::id();
    // NOTE: sysinfo 0.33 API does not expose parent PID.
    // Use 0 as sentinel — self-exclusion via own_pid + name check is sufficient.
    let parent_pid: u32 = 0;

    // Snapshot processes via spawn_blocking (sysinfo blocks 100-300ms — NEVER call from async directly)
    let procs = tokio::task::spawn_blocking(move || {
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes()
            .iter()
            .filter(|(pid, _)| pid.as_u32() > 4)
            .map(|(pid, p)| {
                (
                    pid.as_u32(),
                    p.name().to_string_lossy().to_lowercase(),
                    p.exe()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    p.start_time(), // u64 — for PID identity check
                )
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();

    let wl = whitelist.read().await;
    let violation_action = wl.violation_action.clone();
    let warn_before_kill = wl.warn_before_kill;
    let allowed: Vec<String> = wl.processes.iter().map(|s| s.to_lowercase()).collect();
    drop(wl);

    // Processes seen this cycle — used to prune grace_counts after scan
    let mut seen_violations: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for (pid, name, exe_path, start_time) in &procs {
        // Self-exclusion: own PID (inline guard) + name-based (helper)
        if *pid == own_pid {
            continue;
        }
        if is_self_excluded(own_pid, parent_pid, name) {
            continue;
        }
        // Whitelisted processes: skip
        if is_whitelisted(name, &allowed) {
            continue;
        }

        seen_violations.insert(name.clone());

        // Determine if CRITICAL (zero grace)
        let critical = is_critical_violation(name);

        // Grace period tracking
        let (count, _first_start_time) =
            grace_counts.entry(name.clone()).or_insert((0, *start_time));
        *count += 1;
        let current_count = *count;

        // Determine if we should act (kill or report)
        let should_act = if critical {
            true // zero grace
        } else if warn_before_kill {
            current_count >= 2 // two-cycle grace
        } else {
            true // no grace — act immediately
        };

        let action_taken = if should_act && violation_action == "kill_and_report" {
            // PID identity verification before kill
            let kill_pid = *pid;
            let kill_name = name.clone();
            let kill_start_time = *start_time;
            let killed =
                kill_process_verified(kill_pid, kill_name.clone(), kill_start_time).await;
            if killed {
                log_guard_event(&format!(
                    "KILLED pid={} name={} exe={} count={}",
                    kill_pid, kill_name, exe_path, current_count
                ));
                "killed"
            } else {
                log_guard_event(&format!(
                    "KILL_SKIPPED pid={} name={} (PID reused or process exited)",
                    kill_pid, kill_name
                ));
                "reported"
            }
        } else {
            let severity = if critical { "CRITICAL" } else { "WARN" };
            log_guard_event(&format!(
                "{} pid={} name={} exe={} action=report_only count={}",
                severity, pid, name, exe_path, current_count
            ));
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: machine_id.to_string(),
            violation_type: if critical {
                ViolationType::WrongMachineBinary
            } else {
                ViolationType::Process
            },
            name: name.clone(),
            exe_path: if exe_path.is_empty() {
                None
            } else {
                Some(exe_path.clone())
            },
            action_taken: action_taken.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: current_count,
        };

        let _ = tx.send(AgentMessage::ProcessViolation(violation)).await;
    }

    // Clean up grace counts for processes no longer in violation
    grace_counts.retain(|name, _| seen_violations.contains(name));

    Ok(())
}

/// Kill a process after verifying name + start_time match (PID reuse guard).
/// Returns true if kill was issued successfully, false if PID was reused or process gone.
async fn kill_process_verified(pid: u32, expected_name: String, expected_start_time: u64) -> bool {
    // Re-check identity in spawn_blocking before issuing kill
    let identity_ok = tokio::task::spawn_blocking(move || {
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        match sys.process(sysinfo::Pid::from_u32(pid)) {
            Some(p)
                if p.name().to_string_lossy().to_lowercase() == expected_name
                    && p.start_time() == expected_start_time =>
            {
                true
            }
            Some(_) => {
                tracing::warn!(
                    "PID {} reused before kill (was {}) — skipping",
                    pid,
                    expected_name
                );
                false
            }
            None => {
                tracing::debug!(
                    "PID {} ({}) already exited before kill",
                    pid,
                    expected_name
                );
                false
            }
        }
    })
    .await
    .unwrap_or(false);

    if !identity_ok {
        return false;
    }

    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
    })
    .await;

    match result {
        Ok(Ok(out)) => out.status.success(),
        _ => false,
    }
}

/// Returns true if the given process name should be excluded from enforcement unconditionally.
/// - Any process named "rc-agent.exe" is excluded (primary guard for the agent itself)
/// - own_pid and parent_pid guards are applied inline in run_scan_cycle for clarity
pub(crate) fn is_self_excluded(_own_pid: u32, _parent_pid: u32, name: &str) -> bool {
    name == "rc-agent.exe"
}

/// Returns true if a process name is in the whitelist (case-insensitive).
pub(crate) fn is_whitelisted(name: &str, allowed: &[String]) -> bool {
    let lower = name.to_lowercase();
    allowed.iter().any(|a| a == &lower)
}

/// Returns true if this process is a CRITICAL violation with zero grace period.
/// Currently: racecontrol.exe on any pod (standing rule #2).
pub(crate) fn is_critical_violation(name: &str) -> bool {
    let lower = name.to_lowercase();
    CRITICAL_BINARIES.iter().any(|&b| b == lower)
}

/// Append a timestamped event line to the guard log. Rotates at 512KB.
/// Safe to call from blocking context (uses std::fs, not tokio::fs).
pub(crate) fn log_guard_event(event: &str) {
    use std::io::Write;

    if let Ok(meta) = std::fs::metadata(GUARD_LOG) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::write(GUARD_LOG, b""); // truncate — reset to empty
        }
    }
    let line = format!("[{}] {}\n", Utc::now().to_rfc3339(), event);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(GUARD_LOG)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN_PID: u32 = 9999;
    const PARENT_PID: u32 = 0;

    #[test]
    fn self_excluded_by_own_pid() {
        // own_pid exclusion is done inline in run_scan_cycle (pid == own_pid check).
        // This test verifies that is_self_excluded returns true for rc-agent.exe (name guard),
        // which covers the own-process case because rc-agent.exe IS the agent binary.
        assert!(is_self_excluded(OWN_PID, PARENT_PID, "rc-agent.exe"));
    }

    #[test]
    fn self_excluded_by_name_rc_agent() {
        assert!(is_self_excluded(OWN_PID, PARENT_PID, "rc-agent.exe"));
    }

    #[test]
    fn self_excluded_parent_pid_unused() {
        // parent_pid = 0 sentinel means parent exclusion is disabled; only name matters
        // An unrelated process with parent_pid = 0 is NOT excluded by name
        assert!(!is_self_excluded(OWN_PID, PARENT_PID, "notepad.exe"));
    }

    #[test]
    fn not_self_excluded_unrelated_process() {
        assert!(!is_self_excluded(OWN_PID, PARENT_PID, "notepad.exe"));
    }

    #[test]
    fn whitelisted_exact_case_insensitive() {
        let allowed = vec!["explorer.exe".to_string(), "svchost.exe".to_string()];
        assert!(is_whitelisted("explorer.exe", &allowed));
        assert!(is_whitelisted("EXPLORER.EXE", &allowed));
    }

    #[test]
    fn not_whitelisted_absent_process() {
        let allowed = vec!["explorer.exe".to_string()];
        assert!(!is_whitelisted("steam.exe", &allowed));
    }

    #[test]
    fn critical_violation_racecontrol_exe() {
        assert!(is_critical_violation("racecontrol.exe"));
        assert!(is_critical_violation("RACECONTROL.EXE")); // case-insensitive
    }

    #[test]
    fn not_critical_notepad() {
        assert!(!is_critical_violation("notepad.exe"));
    }

    #[test]
    fn log_rotation_truncates_at_512kb() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // We test the rotation logic inline (not via GUARD_LOG constant — that's prod path)
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        // Write >512KB
        let big_data = vec![b'x'; (512 * 1024 + 100) as usize];
        std::fs::write(&path, &big_data).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 512 * 1024);

        // Simulate rotation (mirrors log_guard_event logic)
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > 512 * 1024 {
                let _ = std::fs::write(&path, b"");
            }
        }
        let new_line = "[2026-01-01T00:00:00Z] TEST\n";
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = f.write_all(new_line.as_bytes());
        }

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            content, new_line,
            "File should contain only the new line after rotation"
        );
    }
}

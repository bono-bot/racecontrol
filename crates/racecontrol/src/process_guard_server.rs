//! Server-side process guard background task.
//!
//! Scans running processes on the server machine, compares against the
//! merged whitelist, applies grace periods, and optionally kills violations.
//! Pushes ProcessViolation records to `state.pod_violations["server"]`.

use rc_common::types::{ProcessViolation, ViolationType};

use super::merge_for_machine;

/// Binary names that are CRITICAL on the server — zero grace period, always action-worthy.
/// rc-agent.exe must never run on server .23 (standing rule #2).
const SERVER_CRITICAL_BINARIES: &[&str] = &["rc-agent.exe"];

/// Returns true if `name` matches a CRITICAL binary for the server (case-insensitive).
/// Strips .exe from both sides for comparison (sysinfo may omit the extension).
pub(crate) fn is_server_critical(name: &str) -> bool {
    let lower = name.to_lowercase();
    let base = lower.trim_end_matches(".exe");
    SERVER_CRITICAL_BINARIES
        .iter()
        .any(|&b| b == lower || b.trim_end_matches(".exe") == base)
}

/// Append a timestamped line to C:\RacingPoint\process-guard.log with 512KB rotation.
/// Safe to call from a blocking context (uses std::fs).
fn log_server_guard_event(line: &str) {
    use std::io::Write;
    const GUARD_LOG: &str = r"C:\RacingPoint\process-guard.log";
    const MAX_LOG_BYTES: u64 = 512 * 1024;

    if let Ok(meta) = std::fs::metadata(GUARD_LOG)
        && meta.len() >= MAX_LOG_BYTES {
            // Truncate to 0 bytes before appending (rotation)
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(GUARD_LOG);
        }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(GUARD_LOG)
    {
        let _ = writeln!(f, "{}", line);
    }
}

/// Spawn the server-side process guard background task.
///
/// Reads `state.config.process_guard` for config. Merges the "server" whitelist.
/// Scans every `poll_interval_secs` seconds (default 60).
/// Logs to C:\RacingPoint\process-guard.log (512KB rotation).
/// Pushes ProcessViolation records to `state.pod_violations["server"]`.
///
/// No-op if `config.process_guard.enabled == false`.
pub fn spawn_server_guard(state: std::sync::Arc<crate::state::AppState>) {
    if !state.config.process_guard.enabled {
        tracing::info!(
            "[server-guard] process_guard.enabled=false — server guard not started"
        );
        return;
    }

    // v3.6: Process guard is designed for Windows venue machines only.
    // On cloud (Linux VPS), it flags normal system processes (docker, postgres, node)
    // as violations — producing thousands of false WARNs per day.
    if state.config.cloud.origin_id == "cloud" {
        tracing::info!(
            "[server-guard] Skipping server guard on cloud instance (origin_id=cloud) — \
             process guard is venue-only"
        );
        return;
    }

    let config = state.config.process_guard.clone();

    // Sanity check: enabled=true but empty allowlist is almost certainly a config loading failure.
    // Log loudly so operators notice before 28K false violations pile up.
    if config.allowed.is_empty() {
        tracing::error!(
            "[server-guard] process_guard.enabled=true but allowed list is EMPTY — \
             every process will be flagged as a violation. This usually means \
             racecontrol.toml failed to parse. Check config file for corruption."
        );
    }

    tokio::spawn(async move {
        tracing::info!(
            "[server-guard] Starting server process guard (interval={}s, action={}, allowed={})",
            config.poll_interval_secs,
            config.violation_action,
            config.allowed.len()
        );

        let own_pid = std::process::id();
        // grace_counts: process_name -> (consecutive_count, first_seen_start_time)
        let mut grace_counts: std::collections::HashMap<String, (u32, u64)> =
            std::collections::HashMap::new();

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(config.poll_interval_secs),
        );
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume the immediate first tick — fire after first full interval
        interval.tick().await;

        loop {
            interval.tick().await;

            // Build current whitelist for server
            let whitelist = merge_for_machine(&config, "server").unwrap_or_default();

            // Snapshot processes in spawn_blocking (sysinfo is blocking, 100-300ms)
            let procs: Vec<(u32, String, String, u64)> = tokio::task::spawn_blocking(|| {
                use sysinfo::{ProcessesToUpdate, System};
                let mut sys = System::new();
                sys.refresh_processes(ProcessesToUpdate::All, true);
                // sysinfo 0.33: .processes() returns &HashMap — must .iter() first
                sys.processes()
                    .iter()
                    .filter(|(pid, _)| pid.as_u32() > 4)
                    .map(|(pid, proc)| {
                        (
                            pid.as_u32(),
                            proc.name().to_string_lossy().to_string(),
                            proc.exe()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            proc.start_time(),
                        )
                    })
                    .collect()
            })
            .await
            .unwrap_or_default();

            let now = chrono::Utc::now();
            let mut violations_this_cycle: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            for (pid, name, exe_path, start_time) in &procs {
                // Self-exclusion: skip own PID
                if *pid == own_pid {
                    continue;
                }
                // Self-exclusion: skip racecontrol.exe (own binary)
                let name_lower = name.to_lowercase();
                if name_lower == "racecontrol.exe" {
                    continue;
                }
                // v3.6: Skip infrastructure processes that are always valid on the server
                // ssh.exe = Tailscale SSH tunnels, sshd.exe = OpenSSH server,
                // w32tm.exe = NTP client, node.exe = Next.js frontends
                if matches!(name_lower.as_str(),
                    "ssh.exe" | "sshd.exe" | "w32tm.exe" | "node.exe" | "conhost.exe"
                ) {
                    continue;
                }

                let is_critical = is_server_critical(name);
                // Match process name against allowlist — strip .exe from both sides
                // because sysinfo may return "rc-agent" while allowlist has "rc-agent.exe"
                let name_base = name_lower.trim_end_matches(".exe");
                let in_whitelist = whitelist.processes.iter().any(|w| {
                    let w_base = w.trim_end_matches(".exe");
                    w_base == name_base || w == &name_lower
                });

                if in_whitelist && !is_critical {
                    grace_counts.remove(&name_lower);
                    continue;
                }

                violations_this_cycle.insert(name_lower.clone());

                // Grace period tracking — CRITICAL processes skip grace entirely
                if !is_critical {
                    let entry = grace_counts
                        .entry(name_lower.clone())
                        .or_insert((0, *start_time));
                    entry.0 += 1;
                    if entry.0 < 2 {
                        // First sighting — warn only, do not action
                        let msg = format!(
                            "[{}] WARN server pid={} name={} (grace 1/2)",
                            now.to_rfc3339(),
                            pid,
                            name
                        );
                        tracing::warn!("[server-guard] {}", msg);
                        log_server_guard_event(&msg);
                        continue;
                    }
                }

                // Determine action
                let action_taken =
                    if config.violation_action == "kill_and_report" || is_critical {
                        let kill_name = name.clone();
                        let kill_pid = *pid;
                        let kill_start = *start_time;
                        // PID identity re-verify before kill
                        let verify_ok = tokio::task::spawn_blocking(move || {
                            use sysinfo::{ProcessesToUpdate, System};
                            let mut sys2 = System::new();
                            sys2.refresh_processes(ProcessesToUpdate::All, true);
                            match sys2.process(sysinfo::Pid::from_u32(kill_pid)) {
                                Some(p)
                                    if p.name().to_string_lossy().to_lowercase()
                                        == kill_name.to_lowercase()
                                        && p.start_time() == kill_start =>
                                {
                                    true
                                }
                                _ => false,
                            }
                        })
                        .await
                        .unwrap_or(false);

                        if verify_ok {
                            #[cfg(windows)]
                            {
                                use std::os::windows::process::CommandExt;
                                const CREATE_NO_WINDOW: u32 = 0x08000000;
                                let _ = std::process::Command::new("taskkill")
                                    .args(["/F", "/PID", &pid.to_string()])
                                    .creation_flags(CREATE_NO_WINDOW)
                                    .output();
                            }
                            #[cfg(not(windows))]
                            {
                                let _ = std::process::Command::new("kill")
                                    .args(["-9", &pid.to_string()])
                                    .output();
                            }
                            "killed"
                        } else {
                            "reported"
                        }
                    } else {
                        "reported"
                    };

                let severity = if is_critical { "CRITICAL" } else { "WARN" };
                let log_line = format!(
                    "[{}] {} server pid={} name={} exe={} action={}",
                    now.to_rfc3339(),
                    severity,
                    pid,
                    name,
                    exe_path,
                    action_taken
                );
                if is_critical {
                    tracing::error!("[server-guard] {}", log_line);
                } else {
                    tracing::warn!("[server-guard] {}", log_line);
                }
                log_server_guard_event(&log_line);

                // Push to pod_violations["server"]
                let consecutive_count = grace_counts
                    .get(&name_lower)
                    .map(|(c, _)| *c)
                    .unwrap_or(1);
                let violation = ProcessViolation {
                    machine_id: "server".to_string(),
                    violation_type: if is_critical {
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
                    timestamp: now.to_rfc3339(),
                    consecutive_count,
                };
                {
                    let mut vmap = state.pod_violations.write().await;
                    vmap.entry("server".to_string()).or_default().push(violation);
                }
            }

            // Clean up grace_counts: retain only names still in violation this cycle
            grace_counts.retain(|name, _| violations_this_cycle.contains(name));
        }
    });
}

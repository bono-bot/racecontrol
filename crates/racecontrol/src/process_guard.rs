//! Process guard server module.
//!
//! Provides:
//! - `merge_for_machine()`: merges global whitelist + per-machine overrides into MachineWhitelist
//! - `get_whitelist_handler`: GET /api/v1/guard/whitelist/{machine_id}
//!
//! Phase 102 scope: config loading + fetch endpoint only.
//! Phase 103 adds rc-agent guard module.
//! Phase 104 adds violation storage and kiosk notifications.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::collections::HashSet;
use std::sync::Arc;

use crate::config::ProcessGuardConfig;
use crate::fleet_health::ViolationStore;
use crate::state::AppState;
use rc_common::types::{MachineWhitelist, ProcessViolation};

/// Returns the machine_type string for a given machine_id.
/// Returns None for unknown / invalid IDs.
///
/// Valid machine IDs: "pod-1" through "pod-8", "james", "server"
pub fn machine_type_for_id(machine_id: &str) -> Option<&'static str> {
    match machine_id {
        "james" => Some("james"),
        "server" => Some("server"),
        id if id.starts_with("pod-") => {
            let num_str = &id[4..]; // everything after "pod-"
            let n: u32 = num_str.parse().ok()?;
            if n >= 1 && n <= 8 {
                Some("pod")
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Merge the global whitelist with per-machine overrides for a specific machine.
///
/// Returns None if machine_id is not a valid Racing Point machine identifier.
///
/// Merge algorithm:
///   1. Include global allowed entries matching machine_type ("all" or exact type match)
///   2. Apply per-machine deny_processes (remove from set)
///   3. Apply per-machine allow_extra_* (add to sets)
///   4. Build and return MachineWhitelist
pub fn merge_for_machine(config: &ProcessGuardConfig, machine_id: &str) -> Option<MachineWhitelist> {
    let machine_type = machine_type_for_id(machine_id)?;

    // Step 1: collect global allowed entries for this machine type
    let mut processes: HashSet<String> = config
        .allowed
        .iter()
        .filter(|entry| {
            entry.machines.is_empty()
                || entry.machines.iter().any(|m| m == "all" || m == machine_type)
        })
        .map(|entry| entry.name.to_lowercase())
        .collect();

    let mut ports: HashSet<u16> = HashSet::new();
    let mut autostart_keys: HashSet<String> = HashSet::new();

    // Step 2 & 3: apply per-machine overrides
    if let Some(override_cfg) = config.overrides.get(machine_type) {
        // Remove denied processes (case-insensitive)
        for deny in &override_cfg.deny_processes {
            processes.remove(&deny.to_lowercase());
        }
        // Add extra allowed processes
        for extra in &override_cfg.allow_extra_processes {
            processes.insert(extra.to_lowercase());
        }
        // Add extra ports
        for &port in &override_cfg.allow_extra_ports {
            ports.insert(port);
        }
        // Add extra autostart keys
        for key in &override_cfg.allow_extra_autostart {
            autostart_keys.insert(key.clone());
        }
    }

    let mut processes_vec: Vec<String> = processes.into_iter().collect();
    processes_vec.sort();
    let mut ports_vec: Vec<u16> = ports.into_iter().collect();
    ports_vec.sort();
    let mut autostart_vec: Vec<String> = autostart_keys.into_iter().collect();
    autostart_vec.sort();

    Some(MachineWhitelist {
        machine_id: machine_id.to_string(),
        processes: processes_vec,
        ports: ports_vec,
        autostart_keys: autostart_vec,
        violation_action: config.violation_action.clone(),
        warn_before_kill: config.warn_before_kill,
    })
}

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

    if let Ok(meta) = std::fs::metadata(GUARD_LOG) {
        if meta.len() >= MAX_LOG_BYTES {
            // Truncate to 0 bytes before appending (rotation)
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(GUARD_LOG);
        }
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
            let whitelist =
                crate::process_guard::merge_for_machine(&config, "server").unwrap_or_default();

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
                        rc_common::types::ViolationType::WrongMachineBinary
                    } else {
                        rc_common::types::ViolationType::Process
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

/// GET /api/v1/guard/whitelist/{machine_id}
///
/// Returns the merged MachineWhitelist for the specified machine.
/// No auth — internal LAN only (consistent with /api/v1/fleet/health).
///
/// machine_id must be one of: "pod-1" through "pod-8", "james", "server"
pub async fn get_whitelist_handler(
    State(state): State<Arc<AppState>>,
    Path(machine_id): Path<String>,
) -> impl IntoResponse {
    match merge_for_machine(&state.config.process_guard, &machine_id) {
        Some(whitelist) => (StatusCode::OK, Json(whitelist)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!(
                    "Unknown machine_id: {}. Valid: pod-1..pod-8, james, server",
                    machine_id
                )
            })),
        )
            .into_response(),
    }
}

/// POST /api/v1/guard/report
///
/// Intake endpoint for rc-process-guard (James workstation standalone binary).
/// Accepts a ProcessViolation JSON payload, stores to pod_violations[machine_id].
///
/// Auth: X-Guard-Token header checked against config.process_guard.report_secret.
/// If report_secret is None (not configured), request is accepted with a warning log.
///
/// Config key: [process_guard] report_secret = "rp-guard-2026" in racecontrol.toml.
/// Default: None (accepts all — dev mode). Always set in production.
pub async fn post_guard_report_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(violation): Json<ProcessViolation>,
) -> impl IntoResponse {
    // Auth check
    let expected = state.config.process_guard.report_secret.as_deref();
    if let Some(secret) = expected {
        let provided = headers
            .get("x-guard-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided != secret {
            tracing::warn!(
                target: "process-guard",
                "POST /guard/report rejected: invalid X-Guard-Token from machine={}",
                violation.machine_id
            );
            return StatusCode::UNAUTHORIZED.into_response();
        }
    } else {
        tracing::warn!(
            target: "process-guard",
            "POST /guard/report: report_secret not configured — accepting unauthenticated report from machine={}",
            violation.machine_id
        );
    }

    let machine_id = violation.machine_id.clone();
    tracing::info!(
        target: "process-guard",
        "HTTP violation report: machine={} type={:?} name={} action={}",
        machine_id, violation.violation_type, violation.name, violation.action_taken
    );

    // Store in pod_violations (same ViolationStore used by WS pod violations)
    {
        let mut violations = state.pod_violations.write().await;
        violations
            .entry(machine_id.clone())
            .or_insert_with(ViolationStore::new)
            .push(violation);
    }

    StatusCode::OK.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedProcess, ProcessGuardConfig, ProcessGuardOverride};
    use std::collections::HashMap;

    fn make_test_config() -> ProcessGuardConfig {
        ProcessGuardConfig {
            enabled: true,
            poll_interval_secs: 60,
            violation_action: "report_only".to_string(),
            warn_before_kill: true,
            allowed: vec![
                AllowedProcess {
                    name: "svchost.exe".to_string(),
                    category: "system".to_string(),
                    machines: vec!["all".to_string()],
                },
                AllowedProcess {
                    name: "rc-agent.exe".to_string(),
                    category: "racecontrol".to_string(),
                    machines: vec!["pod".to_string()],
                },
                AllowedProcess {
                    name: "ollama.exe".to_string(),
                    category: "ollama".to_string(),
                    machines: vec!["pod".to_string()],
                },
                AllowedProcess {
                    name: "racecontrol.exe".to_string(),
                    category: "racecontrol".to_string(),
                    machines: vec!["server".to_string()],
                },
                AllowedProcess {
                    name: "steam.exe".to_string(),
                    category: "game".to_string(),
                    machines: vec!["all".to_string()],
                },
            ],
            overrides: {
                let mut m = HashMap::new();
                m.insert(
                    "pod".to_string(),
                    ProcessGuardOverride {
                        allow_extra_processes: vec![],
                        allow_extra_ports: vec![8090],
                        allow_extra_autostart: vec!["RCAgent".to_string()],
                        deny_processes: vec!["steam.exe".to_string()],
                    },
                );
                m.insert(
                    "james".to_string(),
                    ProcessGuardOverride {
                        allow_extra_processes: vec!["ollama.exe".to_string(), "Code.exe".to_string()],
                        allow_extra_ports: vec![11434],
                        allow_extra_autostart: vec![],
                        deny_processes: vec!["rc-agent.exe".to_string()],
                    },
                );
                m
            },
            report_secret: None,
        }
    }

    // Test 1: pod machine includes entries with machines=["all"] and machines=["pod"]
    #[test]
    fn test_merge_pod_includes_all_and_pod_entries() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        // svchost.exe has machines=["all"] — must be present
        assert!(
            result.processes.contains(&"svchost.exe".to_string()),
            "svchost.exe (all) must be in pod result"
        );
        // rc-agent.exe has machines=["pod"] — must be present before deny filter
        // (no pod deny for rc-agent, so it stays)
        assert!(
            result.processes.contains(&"rc-agent.exe".to_string()),
            "rc-agent.exe (pod) must be in pod result"
        );
    }

    // Test 2: pod machine does NOT include entries with machines=["james"] or machines=["server"]
    #[test]
    fn test_merge_pod_excludes_james_and_server_entries() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        // racecontrol.exe has machines=["server"] — must NOT be in pod result
        assert!(
            !result.processes.contains(&"racecontrol.exe".to_string()),
            "racecontrol.exe (server-only) must NOT be in pod result"
        );
    }

    // Test 3: james machine includes entries with machines=["all"] and machines=["james"]
    // but NOT machines=["pod"] or machines=["server"]
    #[test]
    fn test_merge_james_includes_all_and_james_entries_excludes_pod_server() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "james").expect("james should be valid");
        // svchost.exe has machines=["all"] — must be present
        assert!(
            result.processes.contains(&"svchost.exe".to_string()),
            "svchost.exe (all) must be in james result"
        );
        // ollama.exe and Code.exe are in james allow_extra_processes — must be present
        assert!(
            result.processes.contains(&"ollama.exe".to_string()),
            "ollama.exe (james extra) must be in james result"
        );
        assert!(
            result.processes.contains(&"code.exe".to_string()),
            "code.exe (james extra) must be in james result"
        );
        // racecontrol.exe has machines=["server"] — must NOT be in james result
        assert!(
            !result.processes.contains(&"racecontrol.exe".to_string()),
            "racecontrol.exe (server-only) must NOT be in james result"
        );
        // ollama.exe in global has machines=["pod"] — would NOT be included via global,
        // but james override adds it explicitly
    }

    // Test 4: pod result has deny_processes from overrides["pod"] removed
    // (steam.exe is in global allowed=["all"] but denied for pods)
    #[test]
    fn test_merge_pod_deny_processes_removes_steam() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        assert!(
            !result.processes.contains(&"steam.exe".to_string()),
            "steam.exe must be absent from pod result (in deny_processes)"
        );
    }

    // Test 5: james result has allow_extra_processes from overrides["james"] present
    // (ollama.exe in james override)
    #[test]
    fn test_merge_james_allow_extra_processes_present() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "james").expect("james should be valid");
        assert!(
            result.processes.contains(&"ollama.exe".to_string()),
            "ollama.exe must be in james result (james allow_extra_processes)"
        );
        // rc-agent.exe is not in global for james, so deny_processes has no effect
        // (it was machines=["pod"], not james)
        assert!(
            !result.processes.contains(&"rc-agent.exe".to_string()),
            "rc-agent.exe must NOT be in james result (pod-only binary)"
        );
    }

    // Test 6: unknown machine_id returns None
    #[test]
    fn test_merge_unknown_machine_returns_none() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "unknown-machine");
        assert!(result.is_none(), "unknown-machine should return None");
    }

    // Test 7: result.violation_action matches config.process_guard.violation_action
    #[test]
    fn test_merge_violation_action_matches_config() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        assert_eq!(
            result.violation_action, "report_only",
            "violation_action must match config value"
        );
        assert!(result.warn_before_kill, "warn_before_kill must match config value");
    }

    // Test 8: machine_type_for_id correctness
    #[test]
    fn test_machine_type_for_id() {
        assert_eq!(machine_type_for_id("pod-7"), Some("pod"));
        assert_eq!(machine_type_for_id("pod-1"), Some("pod"));
        assert_eq!(machine_type_for_id("pod-8"), Some("pod"));
        assert_eq!(machine_type_for_id("james"), Some("james"));
        assert_eq!(machine_type_for_id("server"), Some("server"));
        // Out-of-range pod number
        assert_eq!(machine_type_for_id("pod-99"), None);
        assert_eq!(machine_type_for_id("pod-0"), None);
        // Completely unknown
        assert_eq!(machine_type_for_id("unknown"), None);
        assert_eq!(machine_type_for_id(""), None);
    }

    // Test 9: is_server_critical detects rc-agent.exe with zero grace (case-insensitive)
    #[test]
    fn test_is_server_critical_rc_agent() {
        assert!(is_server_critical("rc-agent.exe"));
        assert!(is_server_critical("RC-AGENT.EXE")); // case insensitive
        assert!(!is_server_critical("svchost.exe"));
        assert!(!is_server_critical("racecontrol.exe")); // not CRITICAL for server (self)
    }
}

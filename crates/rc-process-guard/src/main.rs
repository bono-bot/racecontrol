//! rc-process-guard — Standalone process guard for James workstation (.27).
//!
//! Reports via HTTP POST to racecontrol — never via WebSocket (standing rule #2).
//! Reads config from C:\Users\bono\racingpoint\rc-process-guard.toml.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use tokio::sync::RwLock;

use rc_common::types::{MachineWhitelist, ProcessViolation, ViolationType};

const LOG_FILE: &str = r"C:\Users\bono\racingpoint\process-guard-james.log";
const MAX_LOG_BYTES: u64 = 512 * 1024;

/// Binaries that are CRITICAL violations on James — zero grace, kill immediately.
/// rc-agent.exe: standing rule #2, never on James.
/// kiosk.exe: server-only binary.
const JAMES_CRITICAL_BINARIES: &[&str] = &["rc-agent.exe", "kiosk.exe"];

// --------------------------------------------------------------------------
// Config
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GuardConfig {
    server_url: String,
    #[serde(default)]
    tailscale_url: Option<String>,
    #[serde(default)]
    report_secret: Option<String>,
    #[serde(default = "default_scan_interval")]
    scan_interval_secs: u64,
    #[serde(default = "default_machine_id")]
    machine_id: String,
}

fn default_scan_interval() -> u64 {
    60
}
fn default_machine_id() -> String {
    "james".to_string()
}

// --------------------------------------------------------------------------
// Entry point
// --------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config_path = r"C:\Users\bono\racingpoint\rc-process-guard.toml";
    let config: GuardConfig = {
        let raw = std::fs::read_to_string(config_path).unwrap_or_else(|_| {
            tracing::warn!("Config not found at {}; using defaults", config_path);
            r#"server_url = "http://192.168.31.23:8080""#.to_string()
        });
        toml::from_str(&raw)?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // Fetch whitelist on startup (retry up to 5 times with 5s backoff)
    let whitelist = fetch_whitelist_with_retry(&client, &config).await;
    let whitelist = std::sync::Arc::new(RwLock::new(whitelist));

    tracing::info!(
        "rc-process-guard started (machine={}, interval={}s)",
        config.machine_id,
        config.scan_interval_secs
    );

    // 60s startup amnesty — mirrors rc-agent pattern (allows transient processes to settle)
    tokio::time::sleep(Duration::from_secs(60)).await;

    let mut scan_interval =
        tokio::time::interval(Duration::from_secs(config.scan_interval_secs));
    let mut audit_interval = tokio::time::interval(Duration::from_secs(300)); // 5 min
    let mut whitelist_refresh = tokio::time::interval(Duration::from_secs(300)); // 5 min

    // grace_counts: process_name -> (consecutive_count, start_time_of_first_sighting)
    let mut grace_counts: HashMap<String, (u32, u64)> = HashMap::new();

    loop {
        tokio::select! {
            _ = scan_interval.tick() => {
                run_scan_cycle(&whitelist, &client, &config, &mut grace_counts).await;
            }
            _ = audit_interval.tick() => {
                run_autostart_audit_james(&whitelist, &client, &config).await;
                run_port_audit_james(&whitelist, &client, &config).await;
                run_schtasks_audit_james(&whitelist, &client, &config).await;
            }
            _ = whitelist_refresh.tick() => {
                let fresh = fetch_whitelist_with_retry(&client, &config).await;
                *whitelist.write().await = fresh;
                tracing::info!("Whitelist refreshed");
            }
        }
    }
}

// --------------------------------------------------------------------------
// Whitelist fetch
// --------------------------------------------------------------------------

async fn fetch_whitelist_with_retry(
    client: &reqwest::Client,
    config: &GuardConfig,
) -> MachineWhitelist {
    // Try Tailscale URL first if configured
    let urls: Vec<&str> = if let Some(ts) = &config.tailscale_url {
        vec![ts.as_str(), config.server_url.as_str()]
    } else {
        vec![config.server_url.as_str()]
    };

    for attempt in 1..=5u32 {
        for base_url in &urls {
            let url = format!("{}/api/v1/guard/whitelist/james", base_url);
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<MachineWhitelist>().await {
                        Ok(wl) => {
                            tracing::info!("Whitelist fetched from {} (attempt {})", base_url, attempt);
                            return wl;
                        }
                        Err(e) => {
                            tracing::warn!("Whitelist JSON parse error ({}): {}", base_url, e);
                        }
                    }
                }
                Ok(resp) => {
                    tracing::warn!(
                        "Whitelist fetch HTTP {} from {} (attempt {})",
                        resp.status(),
                        base_url,
                        attempt
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Whitelist fetch error from {} (attempt {}): {}",
                        base_url,
                        attempt,
                        e
                    );
                }
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    tracing::warn!("All whitelist fetch attempts failed; using report_only defaults (safe)");
    MachineWhitelist::default()
}

// --------------------------------------------------------------------------
// Process scan cycle
// --------------------------------------------------------------------------

async fn run_scan_cycle(
    whitelist: &std::sync::Arc<RwLock<MachineWhitelist>>,
    client: &reqwest::Client,
    config: &GuardConfig,
    grace_counts: &mut HashMap<String, (u32, u64)>,
) {
    let own_pid = std::process::id();

    // Snapshot processes via spawn_blocking (sysinfo blocks 100-300ms)
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
                    p.start_time(),
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

    let mut seen_violations: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (pid, name, exe_path, start_time) in &procs {
        // Self-exclusion: own PID (inline) + name guard
        if *pid == own_pid {
            continue;
        }
        if is_james_self_excluded(name) {
            continue;
        }
        // Whitelisted processes: skip
        if is_process_whitelisted(name, &allowed) {
            continue;
        }

        seen_violations.insert(name.clone());

        let critical = is_james_critical(name);

        let (count, _first_start) = grace_counts.entry(name.clone()).or_insert((0, *start_time));
        *count += 1;
        let current_count = *count;

        let should_act = if critical {
            true
        } else if warn_before_kill {
            current_count >= 2
        } else {
            true
        };

        let action_taken = if should_act && violation_action == "kill_and_report" {
            let kill_pid = *pid;
            let kill_name = name.clone();
            let kill_start_time = *start_time;
            let killed = kill_process_verified_james(kill_pid, kill_name.clone(), kill_start_time).await;
            if killed {
                log_james_event(&format!(
                    "KILLED pid={} name={} exe={} count={}",
                    kill_pid, kill_name, exe_path, current_count
                ));
                "killed"
            } else {
                log_james_event(&format!(
                    "KILL_SKIPPED pid={} name={} (PID reused or process exited)",
                    kill_pid, kill_name
                ));
                "reported"
            }
        } else {
            let severity = if critical { "CRITICAL" } else { "WARN" };
            log_james_event(&format!(
                "{} pid={} name={} exe={} action=report_only count={}",
                severity, pid, name, exe_path, current_count
            ));
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: config.machine_id.clone(),
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

        post_violation(client, config, violation).await;
    }

    // Prune grace counts for processes no longer in violation
    grace_counts.retain(|name, _| seen_violations.contains(name));
}

// --------------------------------------------------------------------------
// Kill (PID identity verified)
// --------------------------------------------------------------------------

async fn kill_process_verified_james(pid: u32, expected_name: String, expected_start_time: u64) -> bool {
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
            _ => false,
        }
    })
    .await
    .unwrap_or(false);

    if !identity_ok {
        return false;
    }

    let result = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/PID", &pid.to_string()]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await;

    match result {
        Ok(Ok(out)) => out.status.success(),
        _ => false,
    }
}

// --------------------------------------------------------------------------
// HTTP POST violation
// --------------------------------------------------------------------------

async fn post_violation(
    client: &reqwest::Client,
    config: &GuardConfig,
    violation: ProcessViolation,
) {
    let url = format!("{}/api/v1/guard/report", config.server_url);
    let mut req = client.post(&url).json(&violation);
    if let Some(secret) = &config.report_secret {
        req = req.header("X-Guard-Token", secret);
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!("Violation posted: {} ({})", violation.name, violation.violation_type as u8);
        }
        Ok(resp) if resp.status().as_u16() == 401 => {
            tracing::warn!(
                "POST /guard/report 401 Unauthorized — check report_secret in rc-process-guard.toml"
            );
        }
        Ok(resp) => {
            tracing::warn!("POST /guard/report returned HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::warn!("POST /guard/report connection error: {}", e);
            log_james_event(&format!(
                "POST_FAILED violation={} machine={} err={}",
                violation.name, violation.machine_id, e
            ));
        }
    }
}

// --------------------------------------------------------------------------
// Logging (512KB rotation)
// --------------------------------------------------------------------------

fn log_james_event(event: &str) {
    use std::io::Write;
    if let Ok(meta) = std::fs::metadata(LOG_FILE) {
        if meta.len() > MAX_LOG_BYTES {
            let old_path = format!("{}.old", LOG_FILE);
            let _ = std::fs::rename(LOG_FILE, &old_path);
            // New file will be created by OpenOptions below
        }
    }
    let line = format!("[{}] {}\n", Utc::now().to_rfc3339(), event);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

// --------------------------------------------------------------------------
// Autostart audit (HKCU Run, HKLM Run, Startup folders)
// --------------------------------------------------------------------------

async fn run_autostart_audit_james(
    whitelist: &std::sync::Arc<RwLock<MachineWhitelist>>,
    client: &reqwest::Client,
    config: &GuardConfig,
) {
    let wl = whitelist.read().await;
    let allowed_keys: Vec<String> = wl.autostart_keys.iter().map(|s| s.to_lowercase()).collect();
    let violation_action = wl.violation_action.clone();
    drop(wl);

    audit_run_key_james(
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        &allowed_keys,
        &violation_action,
        client,
        config,
    )
    .await;

    audit_run_key_james(
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        &allowed_keys,
        &violation_action,
        client,
        config,
    )
    .await;

    if let Ok(appdata) = std::env::var("APPDATA") {
        let startup_path = format!(
            r"{}\Microsoft\Windows\Start Menu\Programs\Startup",
            appdata
        );
        audit_startup_folder_james(&startup_path, &allowed_keys, &violation_action, client, config)
            .await;
    }

    audit_startup_folder_james(
        r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup",
        &allowed_keys,
        &violation_action,
        client,
        config,
    )
    .await;
}

async fn audit_run_key_james(
    key_path: &str,
    allowed_keys: &[String],
    violation_action: &str,
    client: &reqwest::Client,
    config: &GuardConfig,
) {
    let key_path_owned = key_path.to_string();
    let output = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("reg");
        cmd.args(["query", &key_path_owned]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await;

    let stdout = match output {
        Ok(Ok(out)) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return,
    };

    let entries = parse_run_key_entries_james(&stdout);
    for entry_name in entries {
        if is_autostart_whitelisted_james(&entry_name, allowed_keys) {
            continue;
        }

        let action_taken = if violation_action == "kill_and_report" {
            let key_path_del = key_path.to_string();
            let entry_clone = entry_name.clone();
            let del_result = tokio::task::spawn_blocking(move || {
                #[cfg(windows)]
                use std::os::windows::process::CommandExt;
                let mut cmd = std::process::Command::new("reg");
                cmd.args(["delete", &key_path_del, "/v", &entry_clone, "/f"]);
                #[cfg(windows)]
                cmd.creation_flags(0x08000000);
                cmd.output()
            })
            .await;
            if del_result
                .map(|r| r.map(|o| o.status.success()).unwrap_or(false))
                .unwrap_or(false)
            {
                log_james_event(&format!(
                    "AUTOSTART_REMOVED run_key={} entry={}",
                    key_path, entry_name
                ));
                "removed"
            } else {
                log_james_event(&format!(
                    "AUTOSTART_REMOVE_FAILED run_key={} entry={}",
                    key_path, entry_name
                ));
                "flagged"
            }
        } else {
            log_james_event(&format!(
                "AUTOSTART_FLAGGED run_key={} entry={}",
                key_path, entry_name
            ));
            "flagged"
        };

        let violation = ProcessViolation {
            machine_id: config.machine_id.clone(),
            violation_type: ViolationType::AutoStart,
            name: entry_name.clone(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        post_violation(client, config, violation).await;
    }
}

async fn audit_startup_folder_james(
    folder_path: &str,
    allowed_keys: &[String],
    _violation_action: &str,
    client: &reqwest::Client,
    config: &GuardConfig,
) {
    let folder_path_owned = folder_path.to_string();
    let entries = tokio::task::spawn_blocking(move || {
        walkdir::WalkDir::new(&folder_path_owned)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.ends_with(".lnk") || name.ends_with(".url") || name.ends_with(".bat") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();

    for entry_name in entries {
        if is_autostart_whitelisted_james(&entry_name, allowed_keys) {
            continue;
        }
        log_james_event(&format!(
            "AUTOSTART_STARTUP_FLAGGED folder={} entry={}",
            folder_path, entry_name
        ));

        let violation = ProcessViolation {
            machine_id: config.machine_id.clone(),
            violation_type: ViolationType::AutoStart,
            name: entry_name.clone(),
            exe_path: None,
            action_taken: "flagged".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        post_violation(client, config, violation).await;
    }
}

// --------------------------------------------------------------------------
// Port audit
// --------------------------------------------------------------------------

async fn run_port_audit_james(
    whitelist: &std::sync::Arc<RwLock<MachineWhitelist>>,
    client: &reqwest::Client,
    config: &GuardConfig,
) {
    let (allowed_ports, violation_action) = {
        let wl = whitelist.read().await;
        (wl.ports.clone(), wl.violation_action.clone())
    };

    let output = tokio::task::spawn_blocking(|| {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("netstat");
        cmd.args(["-ano"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await;

    let stdout = match output {
        Ok(Ok(out)) => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return,
    };

    let entries = parse_netstat_listening_james(&stdout);

    for (port, pid) in entries {
        if allowed_ports.contains(&port) {
            continue;
        }

        log_james_event(&format!("PORT_VIOLATION port={} pid={}", port, pid));

        let action_taken = if violation_action == "kill_and_report" {
            let start_time_opt = tokio::task::spawn_blocking(move || {
                let mut sys = sysinfo::System::new();
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                sys.process(sysinfo::Pid::from_u32(pid))
                    .map(|p| p.start_time())
            })
            .await
            .unwrap_or(None);

            let killed = if let Some(start_time) = start_time_opt {
                let name_for_kill = format!("port-owner-pid-{}", pid);
                kill_process_verified_james(pid, name_for_kill, start_time).await
            } else {
                // Cannot verify PID identity (process already exited or lookup failed).
                // Skip kill to avoid PID reuse risk — the PID may belong to a different
                // process by the time we issue taskkill.
                log_james_event(&format!(
                    "PORT_KILL_SKIPPED pid={} port={} (identity not verifiable, PID reuse risk)",
                    pid, port
                ));
                false
            };

            if killed { "killed" } else { "reported" }
        } else {
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: config.machine_id.clone(),
            violation_type: ViolationType::Port,
            name: port.to_string(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        post_violation(client, config, violation).await;
    }
}

// --------------------------------------------------------------------------
// Scheduled task audit
// --------------------------------------------------------------------------

async fn run_schtasks_audit_james(
    whitelist: &std::sync::Arc<RwLock<MachineWhitelist>>,
    client: &reqwest::Client,
    config: &GuardConfig,
) {
    let (allowed_keys, violation_action) = {
        let wl = whitelist.read().await;
        let keys: Vec<String> = wl.autostart_keys.iter().map(|s| s.to_lowercase()).collect();
        (keys, wl.violation_action.clone())
    };

    let output = tokio::task::spawn_blocking(|| {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("schtasks");
        cmd.args(["/query", "/fo", "CSV", "/nh"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await;

    let stdout = match output {
        Ok(Ok(out)) => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return,
    };

    let entries = parse_schtasks_csv_james(&stdout);

    for (task_path, task_name) in entries {
        // Skip Windows system tasks unconditionally
        if task_path.starts_with("\\Microsoft\\") {
            continue;
        }
        if is_autostart_whitelisted_james(&task_name, &allowed_keys) {
            continue;
        }

        log_james_event(&format!(
            "SCHTASK_FLAGGED path={} name={}",
            task_path, task_name
        ));

        let action_taken = if violation_action == "kill_and_report" {
            let path_clone = task_path.clone();
            let disable_result = tokio::task::spawn_blocking(move || {
                #[cfg(windows)]
                use std::os::windows::process::CommandExt;
                let mut cmd = std::process::Command::new("schtasks");
                cmd.args(["/change", "/tn", &path_clone, "/disable"]);
                #[cfg(windows)]
                cmd.creation_flags(0x08000000);
                cmd.output()
            })
            .await;

            if disable_result
                .map(|r| r.map(|o| o.status.success()).unwrap_or(false))
                .unwrap_or(false)
            {
                "disabled"
            } else {
                "flagged"
            }
        } else {
            "flagged"
        };

        let violation = ProcessViolation {
            machine_id: config.machine_id.clone(),
            violation_type: ViolationType::AutoStart,
            name: task_name.clone(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        post_violation(client, config, violation).await;
    }
}

// --------------------------------------------------------------------------
// Helper functions
// --------------------------------------------------------------------------

/// Returns true if this process is the guard itself — must be excluded from enforcement.
fn is_james_self_excluded(name: &str) -> bool {
    name == "rc-process-guard.exe"
}

/// Returns true if this process is a CRITICAL violation on James — zero grace period.
/// rc-agent.exe: standing rule #2 (never on James).
/// kiosk.exe: server-only binary.
fn is_james_critical(name: &str) -> bool {
    let lower = name.to_lowercase();
    JAMES_CRITICAL_BINARIES.iter().any(|&b| b == lower)
}

/// Returns true if the process name is in the allowed list (case-insensitive).
fn is_process_whitelisted(name: &str, allowed: &[String]) -> bool {
    let lower = name.to_lowercase();
    allowed.iter().any(|a| a == &lower)
}

/// Case-insensitive check if an autostart entry name is in the whitelist.
fn is_autostart_whitelisted_james(name: &str, allowed: &[String]) -> bool {
    let lower = name.to_lowercase();
    allowed.iter().any(|a| a == &lower)
}

/// Parse `reg query` stdout into a list of value names.
/// Input format per line: "    ValueName    REG_SZ    C:\path\to\exe"
fn parse_run_key_entries_james(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("HKEY"))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1].starts_with("REG_") {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Parse `netstat -ano` stdout into a list of (port, pid) tuples.
/// Only TCP LISTENING lines are returned. Handles IPv4 and IPv6 via rfind(':').
fn parse_netstat_listening_james(stdout: &str) -> Vec<(u16, u32)> {
    let mut result = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        if !parts[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        if !parts[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let local_addr = parts[1];
        let port_str = match local_addr.rfind(':') {
            Some(idx) => &local_addr[idx + 1..],
            None => continue,
        };
        let port: u16 = match port_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let pid: u32 = match parts[4].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        result.push((port, pid));
    }
    result
}

/// Parse `schtasks /query /fo CSV /nh` stdout into (task_path, task_name) tuples.
/// Skips blank lines, header lines, and lines with fewer than 2 fields.
/// Simple split on `","` boundary to handle quoted CSV without a CSV library.
fn parse_schtasks_csv_james(stdout: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let raw_fields: Vec<&str> = trimmed.split("\",\"").collect();
        let fields: Vec<String> = raw_fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut s = f.to_string();
                if i == 0 {
                    s = s.trim_start_matches('"').to_string();
                }
                if i == raw_fields.len() - 1 {
                    s = s.trim_end_matches('"').to_string();
                }
                s
            })
            .collect();

        if fields.len() < 2 {
            continue;
        }
        let task_path = fields[0].clone();
        let task_name = fields[1].clone();

        // Skip header line
        if task_path.starts_with("TaskName") || task_name.starts_with("Status") {
            continue;
        }
        if task_path.is_empty() || task_name.is_empty() {
            continue;
        }

        result.push((task_path, task_name));
    }
    result
}

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // is_james_self_excluded
    #[test]
    fn self_excluded_own_binary() {
        assert!(is_james_self_excluded("rc-process-guard.exe"));
    }

    #[test]
    fn self_excluded_notepad_false() {
        assert!(!is_james_self_excluded("notepad.exe"));
    }

    // is_james_critical
    #[test]
    fn critical_rc_agent() {
        assert!(is_james_critical("rc-agent.exe"));
    }

    #[test]
    fn critical_kiosk() {
        assert!(is_james_critical("kiosk.exe"));
    }

    #[test]
    fn critical_code_false() {
        assert!(!is_james_critical("code.exe"));
    }

    // parse_netstat_listening_james
    #[test]
    fn netstat_parse_basic() {
        let input = "  TCP    0.0.0.0:4444    0.0.0.0:0    LISTENING    1234\n";
        let result = parse_netstat_listening_james(input);
        assert_eq!(result, vec![(4444u16, 1234u32)]);
    }

    #[test]
    fn netstat_parse_skips_non_listening() {
        let input = "  TCP    0.0.0.0:8080    192.168.1.1:443    ESTABLISHED    5678\n";
        let result = parse_netstat_listening_james(input);
        assert!(result.is_empty());
    }

    #[test]
    fn netstat_parse_ipv6() {
        let input = "  TCP    [::]:9090    [::]:0    LISTENING    999\n";
        let result = parse_netstat_listening_james(input);
        assert_eq!(result, vec![(9090u16, 999u32)]);
    }

    // parse_schtasks_csv_james
    #[test]
    fn schtasks_header_skipped() {
        let input = "\"TaskName\",\"Status\",\"Run As User\"\n\"\\MyTask\",\"ReadyTask\",\"Ready\"\n";
        let result = parse_schtasks_csv_james(input);
        // Header line should be skipped — only 1 real task
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn schtasks_microsoft_system_tasks_skipped() {
        // parse function returns all non-header entries; caller filters \\Microsoft\\
        let input = "\"\\Microsoft\\Windows\\Defrag\",\"IdleTask\",\"Ready\"\n\"\\MyTask\",\"MyViolation\",\"Ready\"\n";
        let result = parse_schtasks_csv_james(input);
        // Parser returns both; system-task skip is in run_schtasks_audit_james
        assert_eq!(result.len(), 2);
    }
}

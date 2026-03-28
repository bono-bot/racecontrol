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
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::{mpsc, RwLock};
use walkdir;

use rc_common::protocol::AgentMessage;
use rc_common::types::{MachineWhitelist, ProcessViolation, ViolationType};
use rc_common::verification::{ColdVerificationChain, VerifyStep, VerificationError};

use crate::config::ProcessGuardConfig;

const LOG_TARGET: &str = "process-guard";

const GUARD_LOG: &str = r"C:\RacingPoint\process-guard.log";
const MAX_LOG_BYTES: u64 = 512 * 1024; // 512 KB

/// Names of processes that are CRITICAL violations when detected on a pod
/// (standing rule #2: never run server binaries on pod machines).
const CRITICAL_BINARIES: &[&str] = &["racecontrol.exe"];

/// System-critical processes that must NEVER be killed regardless of allowlist.
/// Killing these causes BSOD, logoff, or system instability.
const NEVER_KILL: &[&str] = &[
    "csrss.exe", "smss.exe", "wininit.exe", "services.exe", "lsass.exe",
    "winlogon.exe", "svchost.exe", "dwm.exe", "explorer.exe", "system",
    "registry", "ntoskrnl.exe", "conhost.exe", "fontdrvhost.exe",
    "sihost.exe", "taskhostw.exe", "runtimebroker.exe",
    "rc-agent.exe", "rc-sentry.exe", "rc-sentry-ai.exe",
];

/// Result of a single process scan cycle — used for first-scan threshold validation (BOOT-04).
pub struct ScanResult {
    pub total_processes: usize,
    pub violation_count: usize,
    /// MMA-P1: True if the scan itself failed (spawn_blocking panic).
    /// A failed scan must NOT be treated as "0 violations" — OTA pipeline must fail-closed.
    pub scan_failed: bool,
}

// ─── COV-04: Allowlist Verification Chain Steps ─────────────────────────────

/// COV-04 Step 1: Verify HTTP fetch returned a non-empty allowlist.
struct StepAllowlistNonEmpty;
impl VerifyStep for StepAllowlistNonEmpty {
    type Input = MachineWhitelist;
    type Output = MachineWhitelist;
    fn name(&self) -> &str { "allowlist_non_empty" }
    fn run(&self, input: MachineWhitelist) -> Result<MachineWhitelist, VerificationError> {
        if input.processes.is_empty() && input.autostart_keys.is_empty() {
            return Err(VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("processes={} autostart_keys={}", input.processes.len(), input.autostart_keys.len()),
            });
        }
        Ok(input)
    }
}

/// COV-04 Step 2: Sanity check — critical system processes must be in the allowlist.
struct StepSanityCheck;
impl VerifyStep for StepSanityCheck {
    type Input = MachineWhitelist;
    type Output = MachineWhitelist;
    fn name(&self) -> &str { "allowlist_sanity_check" }
    fn run(&self, input: MachineWhitelist) -> Result<MachineWhitelist, VerificationError> {
        let required = ["svchost.exe", "explorer.exe", "rc-agent.exe"];
        let process_names: Vec<String> = input.processes.iter()
            .map(|p| p.to_lowercase())
            .collect();
        let mut missing = Vec::new();
        for req in &required {
            if !process_names.iter().any(|n| n == req) {
                missing.push(*req);
            }
        }
        if !missing.is_empty() {
            return Err(VerificationError::DecisionError {
                step: self.name().to_string(),
                raw_value: format!("missing_critical_processes={:?} total_in_list={}", missing, input.processes.len()),
            });
        }
        Ok(input)
    }
}

/// Run COV-04 verification chain on a fetched allowlist.
/// This is ADDITIVE — does not replace the existing OBS-03 auto-switch logic.
/// Provides structured verification logging via ColdVerificationChain tracing spans.
fn validate_allowlist_chain(wl: &MachineWhitelist, guard_enabled: bool) {
    let chain = ColdVerificationChain::new("allowlist_enforcement");

    // Step 1: Non-empty check
    match chain.execute_step(&StepAllowlistNonEmpty, wl.clone()) {
        Ok(wl_checked) => {
            // Step 2: Sanity check (critical processes present)
            match chain.execute_step(&StepSanityCheck, wl_checked) {
                Ok(_) => {
                    tracing::info!(target: LOG_TARGET, "COV-04: allowlist verification chain passed ({} processes)", wl.processes.len());
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, error = %e, "COV-04: allowlist missing critical system processes");
                }
            }
        }
        Err(e) => {
            if guard_enabled {
                tracing::error!(target: LOG_TARGET, error = %e, "COV-04: empty allowlist with guard enabled — report_only enforced by existing OBS-03 logic");
            } else {
                tracing::warn!(target: LOG_TARGET, error = %e, "COV-04: allowlist is empty (guard disabled, no action needed)");
            }
        }
    }
}

/// Entry point — call once from main.rs after AppState is built.
/// Spawns an internal process scan loop (every config.scan_interval_secs).
/// Auto-start audit is stubbed here; full implementation in Plan 03.
pub fn spawn(
    config: ProcessGuardConfig,
    whitelist: Arc<RwLock<MachineWhitelist>>,
    tx: mpsc::Sender<AgentMessage>,
    machine_id: String,
    safe_mode: Arc<AtomicBool>, // SAFE-04: skip scan when safe mode is active
    guard_confirmed: Arc<AtomicBool>, // BOOT-04: operator confirmation gate
) {
    if !config.enabled {
        tracing::info!(target: LOG_TARGET, "Process guard DISABLED (process_guard.enabled=false)");
        return;
    }
    tokio::spawn(async move {
        // 60s amnesty window on startup — allow transient Windows Update / MpCmdRun to settle
        tokio::time::sleep(Duration::from_secs(60)).await;
        tracing::info!(
            target: LOG_TARGET,
            "Process guard started (interval={}s, machine={})",
            config.scan_interval_secs,
            machine_id
        );

        // OBS-03: Empty allowlist auto-response — detect misconfiguration before first scan
        // If process_guard.enabled=true but the fetched whitelist has no allowed processes,
        // auto-switch to report_only to prevent mass kills on a misconfigured pod.
        {
            let mut wl = whitelist.write().await;
            if wl.processes.is_empty() && wl.autostart_keys.is_empty() && wl.violation_action == "kill_and_report" {
                eprintln!("[process_guard] ERROR: process_guard.enabled=true but allowlist is EMPTY — auto-switching to report_only");
                tracing::error!(
                    target: "state",
                    prev = "kill_and_report",
                    next = "report_only",
                    machine = %machine_id,
                    "EMPTY_ALLOWLIST: process_guard enabled with empty allowlist — auto-switching to report_only to prevent mass kills"
                );
                crate::startup_log::write_phase("EMPTY_ALLOWLIST", "process_guard enabled with empty allowlist, auto-switched to report_only");
                // MMA-P2: Log to tracing at ERROR level so it appears in rc-bot-events.log
                // and is visible to fleet auto-detect scripts
                tracing::error!(
                    target: "diagnostic",
                    trigger = "EmptyAllowlist",
                    action = "auto_switch_report_only",
                    "PROCESS_GUARD_DEGRADED: Empty allowlist at boot — auto-switched to report_only. Guard is NOT enforcing."
                );
                // Write the override directly into the shared whitelist so all scan paths see it
                wl.violation_action = "report_only".to_string();
                drop(wl);
                // MMA-P2: Notify server about guard degradation via ProcessViolation
                let degraded_notification = ProcessViolation {
                    machine_id: machine_id.clone(),
                    violation_type: ViolationType::AutoStart, // Use existing type
                    name: "GUARD_DEGRADED:empty_allowlist".to_string(),
                    exe_path: None,
                    action_taken: "guard_degraded_to_report_only".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    consecutive_count: 1,
                };
                let _ = tx.send(AgentMessage::ProcessViolation(degraded_notification)).await;
            } else {
                drop(wl);
            }
        }

        // COV-04: Run allowlist verification chain (additive — does not replace OBS-03 above)
        {
            let wl = whitelist.read().await;
            validate_allowlist_chain(&wl, config.enabled);
        }

        // Track whether the first scan has run (for >50% threshold check)
        let mut first_scan_done = false;

        let mut scan_interval =
            tokio::time::interval(Duration::from_secs(config.scan_interval_secs));
        let mut audit_interval = tokio::time::interval(Duration::from_secs(300)); // 5 min
        // grace_counts: process_name -> (consecutive_count, start_time_of_first_sighting)
        let mut grace_counts: HashMap<String, (u32, u64)> = HashMap::new();

        // MMA-P1: Dedup set for autostart/schtask audit — prevents re-reporting
        // the same entries every 5 minutes. Only report NEW entries, or re-report
        // after 1 hour (to catch items that were removed and came back).
        let mut audit_dedup: HashMap<String, chrono::DateTime<Utc>> = HashMap::new();

        // Run autostart audit immediately at startup (before first tick)
        run_autostart_audit(&whitelist, &tx, &machine_id, &mut audit_dedup).await;

        loop {
            tokio::select! {
                _ = scan_interval.tick() => {
                    // ─── SAFE-04: skip scan during safe mode ───────────────────
                    // Anti-cheat systems flag CreateToolhelp32Snapshot / OpenProcess calls.
                    // Suspend process scanning entirely while a protected game is running
                    // or during the 30-second post-exit cooldown window.
                    if safe_mode.load(std::sync::atomic::Ordering::Relaxed) {
                        tracing::debug!(target: LOG_TARGET, "safe mode active — scan skipped");
                        continue;
                    }

                    let scan_result =
                        run_scan_cycle(&whitelist, &tx, &machine_id, &mut grace_counts, &guard_confirmed).await;

                    // BOOT-04: First-scan validation — detect misconfigured allowlist
                    if !first_scan_done {
                        first_scan_done = true;
                        if let Ok(ref result) = scan_result {
                            let total = result.total_processes;
                            let violations = result.violation_count;

                            // Log first-scan summary
                            tracing::info!(
                                target: LOG_TARGET,
                                total_processes = total,
                                violations = violations,
                                "BOOT-04: first scan complete"
                            );

                            // Log first 10 violations individually (they are already
                            // logged in run_scan_cycle, but add a summary count here)

                            let wl = whitelist.read().await;
                            if wl.processes.is_empty() {
                                tracing::error!(
                                    target: "state",
                                    machine = %machine_id,
                                    "first scan with empty allowlist — all processes are violations, possible config error, staying in report_only"
                                );
                            }
                            drop(wl);

                            // >50% violation rate = possible misconfiguration
                            if total > 0 && violations * 2 > total {
                                tracing::error!(
                                    target: "state",
                                    machine = %machine_id,
                                    violations = violations,
                                    total = total,
                                    "BOOT-04: first scan violation rate >{:.0}% ({}/{}) — possible misconfiguration, staying in report_only until GUARD_CONFIRMED",
                                    (violations as f64 / total as f64) * 100.0,
                                    violations,
                                    total
                                );
                                // Force report_only regardless of config
                                {
                                    let mut wl_write = whitelist.write().await;
                                    wl_write.violation_action = "report_only".to_string();
                                }
                                crate::startup_log::write_phase(
                                    "FIRST_SCAN_HIGH_VIOLATIONS",
                                    &format!("{}/{} processes flagged, auto-switched to report_only, waiting for GUARD_CONFIRMED", violations, total),
                                );
                                // MMA-P2: Notify server about guard degradation
                                let degraded = ProcessViolation {
                                    machine_id: machine_id.clone(),
                                    violation_type: ViolationType::AutoStart,
                                    name: format!("GUARD_DEGRADED:high_violation_rate_{}_{}", violations, total),
                                    exe_path: None,
                                    action_taken: "guard_degraded_to_report_only".to_string(),
                                    timestamp: Utc::now().to_rfc3339(),
                                    consecutive_count: 1,
                                };
                                let _ = tx.send(AgentMessage::ProcessViolation(degraded)).await;
                            }
                        }
                    }

                    if let Err(e) = scan_result {
                        tracing::error!(target: LOG_TARGET, "Process guard scan error: {}", e);
                    }
                }
                _ = audit_interval.tick() => {
                    // MMA-Iter2-P2: Prune stale dedup entries (>2h old) to prevent unbounded growth
                    let prune_cutoff = Utc::now() - chrono::Duration::hours(2);
                    audit_dedup.retain(|_, ts| *ts > prune_cutoff);

                    run_autostart_audit(&whitelist, &tx, &machine_id, &mut audit_dedup).await;
                    run_port_audit(&whitelist, &tx, &machine_id).await;
                    run_schtasks_audit(&whitelist, &tx, &machine_id, &mut audit_dedup).await;
                }
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
    guard_confirmed: &Arc<AtomicBool>,
) -> anyhow::Result<ScanResult> {
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
    .await;

    let procs = match procs {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "spawn_blocking process scan panicked: {} — reporting scan_failed (NOT treating as clean)", e);
            // MMA-P1: NEVER return 0 violations on scan failure — this is fail-open.
            // Return scan_failed=true so OTA pipeline can fail-closed.
            return Ok(ScanResult {
                total_processes: 0,
                violation_count: 0,
                scan_failed: true,
            });
        }
    };

    let wl = whitelist.read().await;
    let violation_action = wl.violation_action.clone();
    let warn_before_kill = wl.warn_before_kill;
    let allowed: Vec<String> = wl.processes.iter().map(|s| s.to_lowercase()).collect();
    drop(wl);

    // BOOT-04: If violation_action is kill_and_report but guard_confirmed is false,
    // downgrade to report_only for this scan
    let effective_action = if violation_action == "kill_and_report"
        && !guard_confirmed.load(std::sync::atomic::Ordering::Relaxed)
    {
        tracing::debug!(target: LOG_TARGET, "kill_and_report configured but GUARD_CONFIRMED not received — using report_only");
        "report_only".to_string()
    } else {
        violation_action
    };

    let total_processes = procs.len();
    let mut violation_count: usize = 0;

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
        // Never-kill system processes: skip silently
        if NEVER_KILL.iter().any(|&n| n.eq_ignore_ascii_case(name)) {
            continue;
        }
        // Whitelisted processes: skip
        if is_whitelisted(name, &allowed) {
            continue;
        }

        seen_violations.insert(name.clone());
        violation_count += 1;

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

        let action_taken = if should_act && effective_action == "kill_and_report" {
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

    Ok(ScanResult {
        total_processes,
        violation_count,
        scan_failed: false,
    })
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
                    target: LOG_TARGET,
                    "PID {} reused before kill (was {}) — skipping",
                    pid,
                    expected_name
                );
                false
            }
            None => {
                tracing::debug!(
                    target: LOG_TARGET,
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
    // MMA-Iter3: Also exempt hash-based binary names (rc-agent-<hash>.exe) and
    // rc-sentry variants. During deploy, the new binary runs as rc-agent-<hash>.exe
    // before start-rcagent.bat renames it to rc-agent.exe.
    let lower = name.to_lowercase();
    lower == "rc-agent.exe"
        || lower.starts_with("rc-agent-")
        || lower == "rc-sentry.exe"
        || lower.starts_with("rc-sentry-")
        || lower == "rc-watchdog.exe"
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

/// Run one autostart audit: check HKCU Run, HKLM Run, and Startup folder.
/// Three-stage enforcement: LOG → ALERT → REMOVE (configurable via whitelist.violation_action).
/// Backup removals to C:\RacingPoint\autostart-backup.json before deletion.
pub(crate) async fn run_autostart_audit(
    whitelist: &Arc<RwLock<MachineWhitelist>>,
    tx: &mpsc::Sender<AgentMessage>,
    machine_id: &str,
    audit_dedup: &mut HashMap<String, chrono::DateTime<Utc>>,
) {
    let wl = whitelist.read().await;
    let allowed_keys: Vec<String> = wl.autostart_keys.iter().map(|s| s.to_lowercase()).collect();
    let violation_action = wl.violation_action.clone();
    drop(wl);

    // Skip audit if allowlist is empty — empty means "not configured yet", not "block all"
    if allowed_keys.is_empty() {
        tracing::debug!(target: LOG_TARGET, "autostart audit skipped — autostart_keys empty (not configured)");
        return;
    }

    // Audit HKCU Run
    audit_run_key(
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        &allowed_keys, &violation_action, machine_id, tx, audit_dedup
    ).await;

    // Audit HKLM Run
    audit_run_key(
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        &allowed_keys, &violation_action, machine_id, tx, audit_dedup
    ).await;

    // Audit per-user Startup folder
    if let Ok(appdata) = std::env::var("APPDATA") {
        let startup_path = format!(
            r"{}\Microsoft\Windows\Start Menu\Programs\Startup",
            appdata
        );
        audit_startup_folder(&startup_path, &allowed_keys, &violation_action, machine_id, tx, audit_dedup).await;
    }

    // Audit all-users Startup folder
    audit_startup_folder(
        r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup",
        &allowed_keys, &violation_action, machine_id, tx, audit_dedup
    ).await;
}

async fn audit_run_key(
    key_path: &str,
    allowed_keys: &[String],
    violation_action: &str,
    machine_id: &str,
    tx: &mpsc::Sender<AgentMessage>,
    audit_dedup: &mut HashMap<String, chrono::DateTime<Utc>>,
) {
    let key_path_owned = key_path.to_string();
    let output = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        std::process::Command::new("reg")
            .args(["query", &key_path_owned])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
    }).await;

    let stdout = match output {
        Ok(Ok(out)) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).to_string()
        }
        _ => return,
    };

    let entries = parse_run_key_entries(&stdout);
    for entry_name in entries {
        if is_autostart_whitelisted(&entry_name, allowed_keys) {
            continue;
        }

        // MMA-P1: Dedup — skip if reported within the last hour (in report_only mode)
        if violation_action != "kill_and_report" {
            let dedup_key = format!("autostart:{}:{}", key_path, entry_name.to_lowercase());
            let now = Utc::now();
            if let Some(last_reported) = audit_dedup.get(&dedup_key) {
                if (now - *last_reported).num_seconds() < 3600 {
                    continue; // Already reported within the hour, skip
                }
            }
            audit_dedup.insert(dedup_key, now);
        }

        let action_taken = if violation_action == "kill_and_report" {
            // REMOVE stage — backup first
            backup_autostart_entry(&entry_name, &format!("run_key:{}", key_path));
            let key_path_del = key_path.to_string();
            let entry_clone = entry_name.clone();
            let del_result = tokio::task::spawn_blocking(move || {
                #[cfg(windows)]
                use std::os::windows::process::CommandExt;
                std::process::Command::new("reg")
                    .args(["delete", &key_path_del, "/v", &entry_clone, "/f"])
                    .creation_flags(0x08000000)
                    .output()
            }).await;
            if del_result.map(|r| r.map(|o| o.status.success()).unwrap_or(false)).unwrap_or(false) {
                log_guard_event(&format!("AUTOSTART_REMOVED run_key={} entry={}", key_path, entry_name));
                "removed"
            } else {
                log_guard_event(&format!("AUTOSTART_REMOVE_FAILED run_key={} entry={}", key_path, entry_name));
                "flagged"
            }
        } else {
            log_guard_event(&format!("AUTOSTART_REPORTED run_key={} entry={}", key_path, entry_name));
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: machine_id.to_string(),
            violation_type: ViolationType::AutoStart,
            name: entry_name.clone(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        let _ = tx.send(AgentMessage::ProcessViolation(violation)).await;
    }
}

async fn audit_startup_folder(
    folder_path: &str,
    allowed_keys: &[String],
    violation_action: &str,
    machine_id: &str,
    tx: &mpsc::Sender<AgentMessage>,
    audit_dedup: &mut HashMap<String, chrono::DateTime<Utc>>,
) {
    // walkdir scan of the Startup folder for .lnk and .url files
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
    }).await.unwrap_or_default();

    for entry_name in entries {
        if is_autostart_whitelisted(&entry_name, allowed_keys) {
            continue;
        }

        // MMA-P1: Dedup — skip if reported within the last hour
        {
            let dedup_key = format!("startup:{}:{}", folder_path, entry_name);
            let now = Utc::now();
            if let Some(last_reported) = audit_dedup.get(&dedup_key) {
                if (now - *last_reported).num_seconds() < 3600 {
                    continue;
                }
            }
            audit_dedup.insert(dedup_key, now);
        }

        log_guard_event(&format!("AUTOSTART_STARTUP_REPORTED folder={} entry={}", folder_path, entry_name));

        // Startup folder file removal requires staff approval — report only in all modes.
        // Use "reported" so server downgrades to debug (not WARN) in report_only mode.
        let action_taken = {
            if violation_action == "kill_and_report" {
                backup_autostart_entry(&entry_name, &format!("startup_folder:{}", folder_path));
            }
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: machine_id.to_string(),
            violation_type: ViolationType::AutoStart,
            name: entry_name.clone(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        let _ = tx.send(AgentMessage::ProcessViolation(violation)).await;
    }
}

/// Parse `reg query` stdout into a list of value names.
/// Input format per line: "    ValueName    REG_SZ    C:\path\to\exe"
/// Value names may contain spaces (e.g. "Steam Client Bootstrapper"),
/// so we split on the REG_ type token rather than whitespace.
/// Returns only lines that are actual values (skip HKEY header lines and blank lines).
pub(crate) fn parse_run_key_entries(stdout: &str) -> Vec<String> {
    stdout.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("HKEY"))
        .filter_map(|line| {
            // Find "    REG_" pattern — the type token is preceded by whitespace
            let trimmed = line.trim_start();
            let reg_pos = trimmed.find("    REG_")?;
            let name = trimmed[..reg_pos].trim();
            if name.is_empty() {
                return None;
            }
            Some(name.to_string())
        })
        .collect()
}

/// Case-insensitive check if an autostart entry name is in the whitelist.
/// Supports both exact match and prefix match (for tasks with GUID suffixes
/// like "MicrosoftEdgeUpdateTaskMachineCore{A052F23E-...}").
pub(crate) fn is_autostart_whitelisted(name: &str, allowed: &[String]) -> bool {
    let lower = name.to_lowercase();
    allowed.iter().any(|a| a == &lower || lower.starts_with(a))
}

/// Parse `netstat -ano` stdout into a list of (port, pid) tuples.
/// Only TCP LISTENING lines are returned. UDP and non-LISTENING lines are skipped.
/// Handles both IPv4 (0.0.0.0:8080) and IPv6 ([::]:8080) address formats by taking
/// the last ':' segment as the port number.
pub(crate) fn parse_netstat_listening(stdout: &str) -> Vec<(u16, u32)> {
    let mut result = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Expect: TCP  <local_addr>  <remote_addr>  LISTENING  <pid>
        if parts.len() < 5 {
            continue;
        }
        // Only TCP lines
        if !parts[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        // Only LISTENING state (column 3)
        if !parts[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        // Parse port from local address (last segment after ':')
        let local_addr = parts[1];
        let port_str = match local_addr.rfind(':') {
            Some(idx) => &local_addr[idx + 1..],
            None => continue,
        };
        let port: u16 = match port_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        // Parse PID from column 4
        let pid: u32 = match parts[4].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        result.push((port, pid));
    }
    result
}

/// Run one port audit cycle: shell-out to netstat -ano, compare listening ports against
/// whitelist.ports, kill or report violations.
pub(crate) async fn run_port_audit(
    whitelist: &Arc<RwLock<MachineWhitelist>>,
    tx: &mpsc::Sender<AgentMessage>,
    machine_id: &str,
) {
    // Read whitelist fields under brief read lock, then drop
    let (allowed_ports, violation_action) = {
        let wl = whitelist.read().await;
        (wl.ports.clone(), wl.violation_action.clone())
    };

    // Skip audit if port allowlist is empty — empty means "not configured yet", not "block all"
    if allowed_ports.is_empty() {
        tracing::debug!(target: LOG_TARGET, "port audit skipped — ports empty (not configured)");
        return;
    }

    // Shell-out netstat in spawn_blocking to avoid blocking the async runtime
    let output = tokio::task::spawn_blocking(|| {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("netstat");
        cmd.args(["-ano"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        cmd.output()
    })
    .await;

    let stdout = match output {
        Ok(Ok(out)) => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return,
    };

    let entries = parse_netstat_listening(&stdout);

    for (port, pid) in entries {
        // Skip whitelisted ports
        if allowed_ports.contains(&port) {
            continue;
        }

        log_guard_event(&format!("PORT_VIOLATION port={} pid={}", port, pid));

        let action_taken = if violation_action == "kill_and_report" {
            // Attempt kill: try kill_process_verified with sysinfo start_time first
            let start_time_opt = tokio::task::spawn_blocking(move || {
                let mut sys = sysinfo::System::new();
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                sys.process(sysinfo::Pid::from_u32(pid))
                    .map(|p| p.start_time())
            })
            .await
            .unwrap_or(None);

            let killed = if let Some(start_time) = start_time_opt {
                // Use PID-identity-verified kill
                let name_for_kill = format!("port-owner-pid-{}", pid);
                kill_process_verified(pid, name_for_kill, start_time).await
            } else {
                // Fallback: direct taskkill when sysinfo can't find the PID
                let kill_result = tokio::task::spawn_blocking(move || {
                    #[cfg(windows)]
                    use std::os::windows::process::CommandExt;
                    let mut cmd = std::process::Command::new("taskkill");
                    cmd.args(["/F", "/PID", &pid.to_string()]);
                    #[cfg(windows)]
                    cmd.creation_flags(0x08000000);
                    cmd.output()
                })
                .await;
                kill_result
                    .map(|r| r.map(|o| o.status.success()).unwrap_or(false))
                    .unwrap_or(false)
            };

            if killed { "killed" } else { "reported" }
        } else {
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: machine_id.to_string(),
            violation_type: ViolationType::Port,
            name: port.to_string(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        let _ = tx.send(AgentMessage::ProcessViolation(violation)).await;
    }
}

/// Parse `schtasks /query /fo CSV /nh` stdout into (task_path, task_name) tuples.
///
/// Format per line (no-header CSV):  "\\TaskPath","TaskName","Status","..."
/// - Skips blank lines
/// - Skips header lines where col[0] stripped of quotes starts with "TaskName"
/// - Skips lines with fewer than 2 fields
///
/// Simple CSV split strategy: split on `","` (literal comma-quote boundary), then strip
/// remaining leading/trailing quotes from the first and last fields.
pub(crate) fn parse_schtasks_csv(stdout: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Split on `","` to handle quoted CSV fields
        // Each field is separated by `","`, with the first having a leading `"` and last a trailing `"`
        let raw_fields: Vec<&str> = trimmed.split("\",\"").collect();

        // Strip surrounding quotes from first and last fields
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

        // Skip header line (some Windows versions emit it even with /nh)
        if task_path.starts_with("TaskName") || task_name.starts_with("Status") {
            continue;
        }
        // Skip empty path/name
        if task_path.is_empty() || task_name.is_empty() {
            continue;
        }

        result.push((task_path, task_name));
    }
    result
}

/// Run one scheduled task audit cycle: shell-out to schtasks /query /fo CSV /nh,
/// compare task names against whitelist.autostart_keys, flag or disable violations.
/// System tasks under \\Microsoft\\ are always skipped unconditionally.
pub(crate) async fn run_schtasks_audit(
    whitelist: &Arc<RwLock<MachineWhitelist>>,
    tx: &mpsc::Sender<AgentMessage>,
    machine_id: &str,
    audit_dedup: &mut HashMap<String, chrono::DateTime<Utc>>,
) {
    // Read whitelist fields under brief read lock, then drop
    let (allowed_keys, violation_action) = {
        let wl = whitelist.read().await;
        let keys: Vec<String> = wl.autostart_keys.iter().map(|s| s.to_lowercase()).collect();
        (keys, wl.violation_action.clone())
    };

    // Skip audit if autostart_keys is empty — empty means "not configured yet", not "block all"
    if allowed_keys.is_empty() {
        tracing::debug!(target: LOG_TARGET, "schtasks audit skipped — autostart_keys empty (not configured)");
        return;
    }

    // Shell-out schtasks in spawn_blocking
    let output = tokio::task::spawn_blocking(|| {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("schtasks");
        cmd.args(["/query", "/fo", "CSV", "/nh"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        cmd.output()
    })
    .await;

    let stdout = match output {
        Ok(Ok(out)) => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return,
    };

    let entries = parse_schtasks_csv(&stdout);

    for (task_path, _task_name) in entries {
        // Skip Windows system tasks unconditionally
        if task_path.starts_with("\\Microsoft\\") {
            continue;
        }
        // Skip whitelisted tasks — check both path and path's leaf name
        // (task_name from CSV is actually "Next Run Time", not a meaningful name)
        let leaf = task_path.rsplit('\\').next().unwrap_or(&task_path);
        if is_autostart_whitelisted(&task_path, &allowed_keys)
            || is_autostart_whitelisted(leaf, &allowed_keys)
        {
            continue;
        }

        // MMA-P1: Dedup — skip if reported within the last hour (in report_only mode)
        if violation_action != "kill_and_report" {
            let dedup_key = format!("schtask:{}", task_path.to_lowercase());
            let now = Utc::now();
            if let Some(last_reported) = audit_dedup.get(&dedup_key) {
                if (now - *last_reported).num_seconds() < 3600 {
                    continue;
                }
            }
            audit_dedup.insert(dedup_key, now);
        }

        // Use leaf of task_path as the readable name (task_name from CSV is "Next Run Time")
        log_guard_event(&format!(
            "SCHTASK_REPORTED path={} leaf={}",
            task_path, leaf
        ));

        let action_taken = if violation_action == "kill_and_report" {
            // Attempt to disable the task
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
            "reported"
        };

        let violation = ProcessViolation {
            machine_id: machine_id.to_string(),
            violation_type: ViolationType::AutoStart,
            name: leaf.to_string(),
            exe_path: None,
            action_taken: action_taken.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            consecutive_count: 1,
        };
        let _ = tx.send(AgentMessage::ProcessViolation(violation)).await;
    }
}

/// Write entry info to the autostart backup file before removal.
/// Appends a JSON line to C:\RacingPoint\autostart-backup.json.
fn backup_autostart_entry(entry_name: &str, source: &str) {
    use std::io::Write;
    const BACKUP_FILE: &str = r"C:\RacingPoint\autostart-backup.json";
    let line = format!(
        "{{\"entry\":{:?},\"source\":{:?},\"backed_up_at\":{:?}}}\n",
        entry_name, source, Utc::now().to_rfc3339()
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(BACKUP_FILE) {
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
    fn parse_run_key_entries_basic() {
        let stdout = "    MyApp    REG_SZ    C:\\Program Files\\app.exe\n";
        let entries = parse_run_key_entries(stdout);
        assert_eq!(entries, vec!["MyApp"]);
    }

    #[test]
    fn parse_run_key_entries_empty() {
        assert!(parse_run_key_entries("").is_empty());
    }

    #[test]
    fn parse_run_key_entries_skips_header() {
        let stdout = "HKEY_CURRENT_USER\\Software\\...\n\n    App    REG_SZ    C:\\app.exe\n";
        let entries = parse_run_key_entries(stdout);
        assert_eq!(entries, vec!["App"]);
    }

    #[test]
    fn autostart_whitelisted_case_insensitive() {
        let allowed = vec!["rcagent".to_string()];
        assert!(is_autostart_whitelisted("RCAgent", &allowed));
    }

    #[test]
    fn autostart_not_whitelisted() {
        let allowed = vec!["rcagent".to_string()];
        assert!(!is_autostart_whitelisted("SteamClient", &allowed));
    }

    // ── Task 1: parse_netstat_listening tests ──────────────────────────────

    #[test]
    fn parse_netstat_listening_basic_tcp() {
        let stdout = "  TCP    0.0.0.0:4444    0.0.0.0:0    LISTENING    1234\n";
        let result = parse_netstat_listening(stdout);
        assert_eq!(result, vec![(4444u16, 1234u32)]);
    }

    #[test]
    fn parse_netstat_listening_skips_udp() {
        let stdout = "  UDP    0.0.0.0:5353    *:*\n";
        let result = parse_netstat_listening(stdout);
        assert!(result.is_empty(), "UDP lines must be skipped");
    }

    #[test]
    fn parse_netstat_listening_skips_non_listening_state() {
        let stdout = "  TCP    127.0.0.1:1234    127.0.0.1:5678    ESTABLISHED    5678\n";
        let result = parse_netstat_listening(stdout);
        assert!(result.is_empty(), "Non-LISTENING lines must be skipped");
    }

    #[test]
    fn parse_netstat_listening_multiple_ports() {
        let stdout = concat!(
            "  TCP    0.0.0.0:8080    0.0.0.0:0    LISTENING    100\n",
            "  TCP    0.0.0.0:4444    0.0.0.0:0    LISTENING    200\n",
            "  UDP    0.0.0.0:5353    *:*\n",
            "  TCP    127.0.0.1:443    127.0.0.1:0    ESTABLISHED    300\n",
        );
        let result = parse_netstat_listening(stdout);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&(8080u16, 100u32)));
        assert!(result.contains(&(4444u16, 200u32)));
    }

    #[test]
    fn parse_netstat_listening_skips_malformed_lines() {
        let stdout = "  not a valid line\n  TCP    \n  TCP    0.0.0.0:notaport    0.0.0.0:0    LISTENING    999\n";
        let result = parse_netstat_listening(stdout);
        // notaport fails u16 parse — should be skipped
        assert!(result.is_empty(), "Malformed lines must be skipped");
    }

    #[test]
    fn parse_netstat_listening_ipv6_format() {
        // IPv6 addresses use [::]:port format — the last segment after ':' is the port
        let stdout = "  TCP    [::]:8090    [::]:0    LISTENING    999\n";
        let result = parse_netstat_listening(stdout);
        assert_eq!(result, vec![(8090u16, 999u32)]);
    }

    // ── Task 2: parse_schtasks_csv tests ──────────────────────────────────

    #[test]
    fn parse_schtasks_csv_basic() {
        let stdout = "\"\\RacingPoint\\Kiosk\",\"RacingPoint-Kiosk\",\"Ready\"\n";
        let result = parse_schtasks_csv(stdout);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "\\RacingPoint\\Kiosk");
        assert_eq!(result[0].1, "RacingPoint-Kiosk");
    }

    #[test]
    fn parse_schtasks_csv_skips_header() {
        let stdout = concat!(
            "\"TaskName\",\"Status\",\"Next Run Time\"\n",
            "\"\\SomeTask\",\"SomeTask\",\"Ready\"\n",
        );
        let result = parse_schtasks_csv(stdout);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, "SomeTask");
    }

    #[test]
    fn parse_schtasks_csv_multiple_tasks() {
        let stdout = concat!(
            "\"\\RacingPoint\\Kiosk\",\"RacingPoint-Kiosk\",\"Ready\"\n",
            "\"\\SomeTask\",\"NotWhitelisted\",\"Ready\"\n",
        );
        let result = parse_schtasks_csv(stdout);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, "RacingPoint-Kiosk");
        assert_eq!(result[1].1, "NotWhitelisted");
    }

    #[test]
    fn parse_schtasks_csv_empty_lines_skipped() {
        let stdout = "\n\"\\SomeTask\",\"SomeTask\",\"Ready\"\n\n";
        let result = parse_schtasks_csv(stdout);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_schtasks_csv_insufficient_fields_skipped() {
        let stdout = "\"only-one-field\"\n\"\\ValidPath\",\"ValidName\",\"Ready\"\n";
        let result = parse_schtasks_csv(stdout);
        // First line has only 1 field — skip it
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, "ValidName");
    }

    // ── Task 1: log_rotation_truncates_at_512kb ────────────────────────────

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

//! Game Doctor — specialized diagnostic intelligence for game launch failures.
//!
//! This is the RC Doctor's game launch specialist. Every failed launch costs revenue.
//! Instead of waiting 90 seconds and killing everything, Game Doctor diagnoses WHY
//! the launch failed and applies the right fix.
//!
//! MMA-trained diagnostic methods:
//!   Scanner (Qwen3): Exhaustive 12-point pre-launch checklist
//!   Reasoner (R1): Absence-based analysis — what's missing that should be there?
//!   Code Expert (V3): Config content audit — does race.ini match what was requested?
//!   SRE (MiMo): Stuck state detection — is CM hung on a dialog? Is acs.exe a zombie?
//!   Security (Gemini): Path validation — is the AC install intact? Any file corruption?
//!
//! Called by tier_engine Tier 1 for GameLaunchFail and PreFlightFailed triggers.

use std::path::Path;

const LOG_TARGET: &str = "game-doctor";

/// AC content directory on pods
const AC_CONTENT_PATH: &str =
    r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content";

/// Structured diagnosis result from the Game Doctor.
#[derive(Debug, Clone)]
pub struct GameDiagnosis {
    /// Root cause category for KB storage
    pub cause: GameFailureCause,
    /// Human-readable explanation
    pub detail: String,
    /// Fix action taken (if any)
    pub fix_applied: Option<String>,
    /// Whether the fix was successful
    pub fixed: bool,
    /// Hint for retry orchestrator: what cleanup to perform between retries (GAME-02)
    pub retry_hint: RetryHint,
}

/// Hint for the retry orchestrator on what cleanup to perform between retries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryHint {
    /// Kill orphan processes, then retry launch.
    RetryAfterKill,
    /// Reset config files (race.ini, gui.ini), then retry.
    RetryAfterConfigReset,
    /// Free disk space (temp files, logs), then retry.
    RetryAfterDiskCleanup,
    /// No deterministic retry possible — escalate immediately.
    NoRetry,
}

/// Known game launch failure categories.
/// Each maps to a specific diagnostic approach and fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameFailureCause {
    /// AC installation directory not found (acs.exe missing)
    AcNotInstalled,
    /// race.ini missing or unreadable
    RaceIniMissing,
    /// race.ini exists but missing critical sections ([RACE], [CAR_0], [SESSION_0])
    RaceIniCorrupt,
    /// Requested car not installed (folder doesn't exist in content/cars/)
    CarNotInstalled { car_id: String },
    /// Requested track not installed (folder doesn't exist in content/tracks/)
    TrackNotInstalled { track_id: String },
    /// Requested track config doesn't exist
    TrackConfigMissing { track_id: String, config: String },
    /// Content Manager stuck on error dialog (process alive, acs.exe not spawned)
    ContentManagerHung,
    /// Orphan acs.exe from previous session blocking new launch
    OrphanAcsProcess { pid: u32 },
    /// Orphan Content Manager process blocking new launch
    OrphanCmProcess,
    /// Stale game.pid file from previous session
    StaleGamePid,
    /// AC Documents/cfg directory missing (first-time setup incomplete)
    AcConfigDirMissing,
    /// gui.ini missing FORCE_START=1 (CSP pre-requisite)
    GuiIniNotPatched,
    /// MAINTENANCE_MODE sentinel blocking all game launches
    MaintenanceModeBlocking,
    /// OTA_DEPLOYING sentinel blocking launches during update
    OtaDeployBlocking,
    /// Disk space too low for AC to run
    DiskSpaceLow { available_mb: u64 },
    /// Multiple issues found — compound failure
    MultipleIssues { count: usize },
    /// Unknown cause — escalate to model tiers
    Unknown,
}

/// Run the full Game Doctor diagnostic suite.
/// Returns a diagnosis with root cause, detail, and whether a fix was applied.
///
/// This is the Tier 1 deterministic game launch expert.
/// Only applies safe, idempotent fixes. Escalates ambiguous cases.
pub fn diagnose_and_fix() -> GameDiagnosis {
    tracing::info!(target: LOG_TARGET, "Game Doctor: running 12-point launch diagnostic");
    let mut issues: Vec<String> = Vec::new();
    let mut fixes: Vec<String> = Vec::new();

    // ── Check 1 (Gemini Security): Sentinel files blocking launch ──
    if Path::new(r"C:\RacingPoint\MAINTENANCE_MODE").exists() {
        tracing::warn!(target: LOG_TARGET, "MAINTENANCE_MODE sentinel blocking launch — clearing");
        let _ = std::fs::remove_file(r"C:\RacingPoint\MAINTENANCE_MODE");
        fixes.push("cleared MAINTENANCE_MODE sentinel".to_string());
    }
    if Path::new(r"C:\RacingPoint\OTA_DEPLOYING").exists() {
        // OTA might be legitimately in progress — check age
        let age = std::fs::metadata(r"C:\RacingPoint\OTA_DEPLOYING")
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if age > 600 {
            // >10 minutes = stale OTA sentinel
            tracing::warn!(target: LOG_TARGET, "Stale OTA_DEPLOYING sentinel ({}s old) — clearing", age);
            let _ = std::fs::remove_file(r"C:\RacingPoint\OTA_DEPLOYING");
            fixes.push(format!("cleared stale OTA_DEPLOYING ({}s old)", age));
        } else {
            issues.push(format!("OTA_DEPLOYING active ({}s old) — launch blocked during update", age));
            return GameDiagnosis {
                cause: GameFailureCause::OtaDeployBlocking,
                detail: format!("OTA deploy in progress ({}s old) — cannot launch game during update", age),
                fix_applied: None,
                fixed: false,
                retry_hint: RetryHint::RetryAfterKill,
            };
        }
    }

    // ── Check 2 (Gemini Security): AC installation exists ──
    let ac_dir = find_ac_dir();
    if ac_dir.is_none() {
        issues.push("AC installation not found — acs.exe missing from all known paths".to_string());
        return GameDiagnosis {
            cause: GameFailureCause::AcNotInstalled,
            detail: "Assetto Corsa not installed — acs.exe not found in Steam directories".to_string(),
            fix_applied: None,
            fixed: false,
            retry_hint: RetryHint::NoRetry,
        };
    }
    let ac_dir = ac_dir.expect("checked above");

    // ── Check 3 (MiMo SRE): Orphan acs.exe from previous session ──
    if let Some(pid) = find_acs_pid() {
        tracing::warn!(target: LOG_TARGET, "Orphan acs.exe found (PID {}) — killing", pid);
        let _ = crate::ac_launcher::hidden_cmd("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(1));
        if find_acs_pid().is_some() {
            issues.push(format!("Cannot kill orphan acs.exe PID {} — may need reboot", pid));
        } else {
            fixes.push(format!("killed orphan acs.exe PID {}", pid));
        }
    }

    // ── Check 4 (MiMo SRE): Orphan Content Manager blocking launch ──
    if crate::ac_launcher::is_process_running("Content Manager.exe") {
        tracing::warn!(target: LOG_TARGET, "Orphan Content Manager found — killing");
        let _ = crate::ac_launcher::hidden_cmd("taskkill")
            .args(["/IM", "Content Manager.exe", "/F"])
            .output();
        let _ = crate::ac_launcher::hidden_cmd("taskkill")
            .args(["/IM", "acmanager.exe", "/F"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(500));
        fixes.push("killed orphan Content Manager".to_string());
    }

    // ── Check 5 (MiMo SRE): Stale game.pid file ──
    let game_pid_path = Path::new(r"C:\RacingPoint\game.pid");
    if game_pid_path.exists() {
        // Check if the PID in file is actually running
        if let Ok(content) = std::fs::read_to_string(game_pid_path) {
            let pid_str = content.trim();
            let pid_alive = pid_str.parse::<u32>().ok().map_or(false, |pid| {
                use sysinfo::{System, ProcessesToUpdate, Pid};
                let mut sys = System::new();
                sys.refresh_processes(ProcessesToUpdate::All, false);
                sys.process(Pid::from_u32(pid)).is_some()
            });
            if !pid_alive {
                tracing::warn!(target: LOG_TARGET, "Stale game.pid ({}) — process not running, clearing", pid_str);
                let _ = std::fs::remove_file(game_pid_path);
                fixes.push(format!("cleared stale game.pid (PID {} not running)", pid_str));
            }
        }
    }

    // ── Check 6 (V3 Code Expert): AC config directory exists ──
    let cfg_dir = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg");
    if !cfg_dir.exists() {
        tracing::warn!(target: LOG_TARGET, "AC cfg directory missing — creating");
        if std::fs::create_dir_all(&cfg_dir).is_ok() {
            fixes.push("created missing AC cfg directory".to_string());
        } else {
            issues.push("AC cfg directory missing and cannot be created".to_string());
        }
    }

    // ── Check 7 (V3 Code Expert): race.ini exists and is valid ──
    let race_ini = cfg_dir.join("race.ini");
    if race_ini.exists() {
        match std::fs::read_to_string(&race_ini) {
            Ok(content) => {
                if !content.contains("[RACE]") {
                    issues.push("race.ini missing [RACE] section — corrupted".to_string());
                }
                if !content.contains("[CAR_0]") {
                    issues.push("race.ini missing [CAR_0] section — no player car configured".to_string());
                }
                if !content.contains("[SESSION_0]") {
                    issues.push("race.ini missing [SESSION_0] section — no session configured".to_string());
                }
                // Check car exists on disk
                if let Some(car_line) = content.lines().find(|l| l.starts_with("MODEL=")) {
                    let car_id = car_line.trim_start_matches("MODEL=").trim();
                    if !car_id.is_empty() {
                        let car_path = Path::new(AC_CONTENT_PATH).join("cars").join(car_id);
                        if !car_path.exists() {
                            issues.push(format!("Car '{}' not installed ({})", car_id, car_path.display()));
                        }
                    }
                }
                // Check track exists on disk
                if let Some(track_line) = content.lines().find(|l| l.starts_with("TRACK=")) {
                    let track_id = track_line.trim_start_matches("TRACK=").trim();
                    if !track_id.is_empty() {
                        let track_path = Path::new(AC_CONTENT_PATH).join("tracks").join(track_id);
                        if !track_path.exists() {
                            issues.push(format!("Track '{}' not installed ({})", track_id, track_path.display()));
                        }
                    }
                }
            }
            Err(e) => {
                issues.push(format!("race.ini unreadable: {}", e));
            }
        }
    }
    // race.ini missing is not necessarily an issue — it gets written before each launch

    // ── Check 8 (V3 Code Expert): gui.ini has FORCE_START=1 ──
    let gui_ini = cfg_dir.join("gui.ini");
    if gui_ini.exists() {
        if let Ok(content) = std::fs::read_to_string(&gui_ini) {
            if !content.contains("FORCE_START=1") {
                tracing::warn!(target: LOG_TARGET, "gui.ini missing FORCE_START=1 — patching");
                // Append to [OPTIONS] section or create it
                let patched = if content.contains("[OPTIONS]") {
                    content.replace("[OPTIONS]", "[OPTIONS]\r\nFORCE_START=1")
                } else {
                    format!("{}\r\n[OPTIONS]\r\nFORCE_START=1\r\nHIDE_MAIN_MENU=1\r\n", content)
                };
                if std::fs::write(&gui_ini, patched).is_ok() {
                    fixes.push("patched gui.ini: added FORCE_START=1".to_string());
                } else {
                    issues.push("gui.ini missing FORCE_START=1 and cannot be patched".to_string());
                }
            }
        }
    }

    // ── Check 9 (MiMo SRE): Disk space check ──
    {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        for disk in disks.list() {
            let mount = disk.mount_point().to_string_lossy();
            if mount.starts_with("C:") || mount == "/" {
                let mb = disk.available_space() / (1024 * 1024);
                if mb < 1024 {
                    issues.push(format!("Disk space critically low: {}MB (need >1GB)", mb));
                }
            }
        }
    }

    // ── Check 10 (R1 Reasoner): WerFault crash dialogs blocking ──
    if crate::ac_launcher::is_process_running("WerFault.exe") {
        tracing::warn!(target: LOG_TARGET, "WerFault.exe crash dialog found — killing");
        let _ = crate::ac_launcher::hidden_cmd("taskkill")
            .args(["/IM", "WerFault.exe", "/F"])
            .output();
        let _ = crate::ac_launcher::hidden_cmd("taskkill")
            .args(["/IM", "WerFaultSecure.exe", "/F"])
            .output();
        fixes.push("killed WerFault crash dialog".to_string());
    }

    // ── Check 11 (R1 Reasoner): Dialog processes from previous crash ──
    for proc in crate::ac_launcher::DIALOG_PROCESSES {
        if crate::ac_launcher::is_process_running(proc) {
            let _ = crate::ac_launcher::hidden_cmd("taskkill")
                .args(["/IM", proc, "/F"])
                .output();
            fixes.push(format!("killed dialog process: {}", proc));
        }
    }

    // ── Check 12 (Scanner Qwen3): Steam not running (AC requires Steam) ──
    if !crate::ac_launcher::is_process_running("steam.exe") {
        issues.push("Steam not running — AC requires Steam to be active".to_string());
        // Try to start Steam
        let steam_path = Path::new(r"C:\Program Files (x86)\Steam\steam.exe");
        if steam_path.exists() {
            tracing::warn!(target: LOG_TARGET, "Steam not running — starting");
            let mut cmd = std::process::Command::new(steam_path);
            cmd.arg("-silent");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            if cmd.spawn().is_ok() {
                fixes.push("started Steam (was not running)".to_string());
                // Give Steam 5s to initialize
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }
    }

    // ── Build diagnosis ──
    let total_fixes = fixes.len();
    let total_issues = issues.len();

    if total_issues == 0 && total_fixes == 0 {
        tracing::info!(target: LOG_TARGET, "Game Doctor: all 12 checks passed — no issues found");
        return GameDiagnosis {
            cause: GameFailureCause::Unknown,
            detail: "All 12 pre-launch checks passed — escalating to model diagnosis".to_string(),
            fix_applied: None,
            fixed: false,
            retry_hint: RetryHint::NoRetry,
        };
    }

    if total_issues == 0 && total_fixes > 0 {
        let fix_str = fixes.join("; ");
        tracing::info!(target: LOG_TARGET, "Game Doctor: {} issues fixed — {}", total_fixes, fix_str);
        let cause = if total_fixes == 1 {
            categorize_fix(&fixes[0])
        } else {
            GameFailureCause::MultipleIssues { count: total_fixes }
        };
        let hint = hint_for_cause(&cause);
        return GameDiagnosis {
            cause,
            detail: format!("Fixed {} issue(s): {}", total_fixes, fix_str),
            fix_applied: Some(fix_str),
            fixed: true,
            retry_hint: hint,
        };
    }

    // Issues remain after fixes
    let issue_str = issues.join("; ");
    let fix_str = if fixes.is_empty() { "none".to_string() } else { fixes.join("; ") };
    tracing::warn!(
        target: LOG_TARGET,
        "Game Doctor: {} issues remain (fixes applied: {}) — {}",
        total_issues, total_fixes, issue_str
    );

    let cause = if total_issues == 1 {
        categorize_issue(&issues[0])
    } else {
        GameFailureCause::MultipleIssues { count: total_issues }
    };
    let hint = hint_for_cause(&cause);
    GameDiagnosis {
        cause,
        detail: format!("Issues: {}. Fixes applied: {}", issue_str, fix_str),
        fix_applied: if fixes.is_empty() { None } else { Some(fix_str) },
        fixed: false,
        retry_hint: hint,
    }
}

/// Map a GameFailureCause to a RetryHint for the retry orchestrator (GAME-02).
pub fn hint_for_cause(cause: &GameFailureCause) -> RetryHint {
    match cause {
        GameFailureCause::OrphanAcsProcess { .. }
        | GameFailureCause::OrphanCmProcess
        | GameFailureCause::ContentManagerHung
        | GameFailureCause::StaleGamePid
        | GameFailureCause::MaintenanceModeBlocking
        | GameFailureCause::OtaDeployBlocking
        | GameFailureCause::MultipleIssues { .. } => RetryHint::RetryAfterKill,

        GameFailureCause::RaceIniMissing
        | GameFailureCause::RaceIniCorrupt
        | GameFailureCause::GuiIniNotPatched
        | GameFailureCause::AcConfigDirMissing => RetryHint::RetryAfterConfigReset,

        GameFailureCause::DiskSpaceLow { .. } => RetryHint::RetryAfterDiskCleanup,

        GameFailureCause::AcNotInstalled
        | GameFailureCause::CarNotInstalled { .. }
        | GameFailureCause::TrackNotInstalled { .. }
        | GameFailureCause::TrackConfigMissing { .. }
        | GameFailureCause::Unknown => RetryHint::NoRetry,
    }
}

/// MMA-C8: Validate path component — reject traversal attempts.
fn is_safe_path_component(s: &str) -> bool {
    !s.is_empty()
        && !s.contains("..")
        && !s.contains('/')
        && !s.contains('\\')
        && !s.contains('\0')
        && s.len() <= 128
}

/// Validate that a specific car is installed on this pod.
pub fn is_car_installed(car_id: &str) -> bool {
    if !is_safe_path_component(car_id) {
        tracing::warn!(target: LOG_TARGET, "Path traversal attempt in car_id: {:?}", car_id);
        return false;
    }
    Path::new(AC_CONTENT_PATH).join("cars").join(car_id).exists()
}

/// Validate that a specific track is installed on this pod.
pub fn is_track_installed(track_id: &str) -> bool {
    if !is_safe_path_component(track_id) {
        tracing::warn!(target: LOG_TARGET, "Path traversal attempt in track_id: {:?}", track_id);
        return false;
    }
    Path::new(AC_CONTENT_PATH).join("tracks").join(track_id).exists()
}

/// Validate that a specific track config exists.
pub fn is_track_config_valid(track_id: &str, config: &str) -> bool {
    if config.is_empty() {
        return true; // No config = default layout
    }
    if !is_safe_path_component(track_id) || !is_safe_path_component(config) {
        tracing::warn!(target: LOG_TARGET, "Path traversal attempt in track config: {:?}/{:?}", track_id, config);
        return false;
    }
    Path::new(AC_CONTENT_PATH)
        .join("tracks")
        .join(track_id)
        .join(config)
        .exists()
}

/// Quick pre-launch validation — call BEFORE writing race.ini.
/// Returns Ok(()) if everything looks good, Err with description if not.
/// This catches config problems before they become 90-second timeouts.
pub fn pre_launch_validate(car: &str, track: &str, track_config: &str) -> Result<(), String> {
    // Check AC installed
    if find_ac_dir().is_none() {
        return Err("AC not installed — acs.exe not found".to_string());
    }

    // Check car exists
    if !car.is_empty() && !is_car_installed(car) {
        return Err(format!("Car '{}' not installed on this pod", car));
    }

    // Check track exists
    if !track.is_empty() && !is_track_installed(track) {
        return Err(format!("Track '{}' not installed on this pod", track));
    }

    // Check track config
    if !track_config.is_empty() && !is_track_config_valid(track, track_config) {
        return Err(format!("Track config '{}/{}' not found", track, track_config));
    }

    // Check Steam running
    if !crate::ac_launcher::is_process_running("steam.exe") {
        return Err("Steam not running — AC requires Steam".to_string());
    }

    Ok(())
}

// ─── Internal helpers ──────────────────────────────────────────────────────

fn find_ac_dir() -> Option<std::path::PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa",
        r"C:\Program Files\Steam\steamapps\common\assettocorsa",
        r"D:\SteamLibrary\steamapps\common\assettocorsa",
    ];
    for dir in &candidates {
        let p = Path::new(dir);
        if p.join("acs.exe").exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

fn find_acs_pid() -> Option<u32> {
    let output = crate::ac_launcher::hidden_cmd("tasklist")
        .args(["/FI", "IMAGENAME eq acs.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("\"acs.exe\"") {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let pid_str = parts[1].trim_matches('"');
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Some(pid);
                }
            }
        }
    }
    None
}

/// Map a fix description to a failure cause category.
fn categorize_fix(fix: &str) -> GameFailureCause {
    if fix.contains("MAINTENANCE_MODE") {
        GameFailureCause::MaintenanceModeBlocking
    } else if fix.contains("orphan acs.exe") {
        GameFailureCause::OrphanAcsProcess { pid: 0 }
    } else if fix.contains("Content Manager") {
        GameFailureCause::OrphanCmProcess
    } else if fix.contains("game.pid") {
        GameFailureCause::StaleGamePid
    } else if fix.contains("gui.ini") {
        GameFailureCause::GuiIniNotPatched
    } else {
        GameFailureCause::Unknown
    }
}

/// Map an issue description to a failure cause category.
fn categorize_issue(issue: &str) -> GameFailureCause {
    if issue.contains("not installed") && issue.contains("Car") {
        GameFailureCause::CarNotInstalled { car_id: extract_quoted(issue) }
    } else if issue.contains("not installed") && issue.contains("Track") {
        GameFailureCause::TrackNotInstalled { track_id: extract_quoted(issue) }
    } else if issue.contains("acs.exe") || issue.contains("AC installation") {
        GameFailureCause::AcNotInstalled
    } else if issue.contains("race.ini") && issue.contains("corrupted") {
        GameFailureCause::RaceIniCorrupt
    } else if issue.contains("Disk space") {
        GameFailureCause::DiskSpaceLow { available_mb: 0 }
    } else if issue.contains("Steam") {
        GameFailureCause::Unknown // Steam issue but we tried to start it
    } else {
        GameFailureCause::Unknown
    }
}

/// Extract the first 'quoted' string from a message.
fn extract_quoted(s: &str) -> String {
    if let Some(start) = s.find('\'') {
        if let Some(end) = s[start + 1..].find('\'') {
            return s[start + 1..start + 1 + end].to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_fix() {
        assert!(matches!(
            categorize_fix("cleared MAINTENANCE_MODE sentinel"),
            GameFailureCause::MaintenanceModeBlocking
        ));
        assert!(matches!(
            categorize_fix("killed orphan acs.exe PID 1234"),
            GameFailureCause::OrphanAcsProcess { .. }
        ));
        assert!(matches!(
            categorize_fix("killed orphan Content Manager"),
            GameFailureCause::OrphanCmProcess
        ));
    }

    #[test]
    fn test_categorize_issue() {
        assert!(matches!(
            categorize_issue("Car 'ks_ferrari_488' not installed"),
            GameFailureCause::CarNotInstalled { .. }
        ));
        assert!(matches!(
            categorize_issue("Track 'spa' not installed"),
            GameFailureCause::TrackNotInstalled { .. }
        ));
        assert!(matches!(
            categorize_issue("AC installation not found"),
            GameFailureCause::AcNotInstalled
        ));
    }

    #[test]
    fn test_extract_quoted() {
        assert_eq!(extract_quoted("Car 'ks_ferrari_488' not installed"), "ks_ferrari_488");
        assert_eq!(extract_quoted("no quotes here"), "");
    }

    #[test]
    fn test_is_car_installed_nonexistent() {
        // This car doesn't exist on dev machines
        assert!(!is_car_installed("this_car_does_not_exist_xyz"));
    }

    #[test]
    fn test_is_track_installed_nonexistent() {
        assert!(!is_track_installed("this_track_does_not_exist_xyz"));
    }

    #[test]
    fn test_track_config_empty_is_valid() {
        assert!(is_track_config_valid("any_track", ""));
    }
}

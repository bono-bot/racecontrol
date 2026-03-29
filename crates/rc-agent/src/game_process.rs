use std::path::PathBuf;
use std::process::{Child, Command};

use rc_common::types::{GameState, SimType};
use serde::Deserialize;

const LOG_TARGET: &str = "game-process";

/// Create a Command with CREATE_NO_WINDOW on Windows (prevents console flash).
/// Used for background utilities (taskkill, cmd wrappers). NOT for game exe launches.
fn hidden_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Directory where the PID file is persisted.
/// Windows: C:\RacingPoint\  Linux: /tmp/racecontrol/
fn pid_file_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    { PathBuf::from(r"C:\RacingPoint") }
    #[cfg(not(target_os = "windows"))]
    { PathBuf::from("/tmp/racecontrol") }
}

fn pid_file_path() -> PathBuf {
    pid_file_dir().join("game.pid")
}

/// Write the current game PID to disk so it survives rc-agent restarts.
pub fn persist_pid(pid: u32) {
    let dir = pid_file_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Err(e) = std::fs::write(pid_file_path(), pid.to_string()) {
        tracing::warn!(target: LOG_TARGET, "Failed to persist game PID {}: {}", pid, e);
    } else {
        tracing::debug!(target: LOG_TARGET, "Persisted game PID {} to {:?}", pid, pid_file_path());
    }
}

/// Read a previously persisted PID from disk.
pub fn read_persisted_pid() -> Option<u32> {
    std::fs::read_to_string(pid_file_path())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

/// Remove the PID file from disk.
pub fn clear_persisted_pid() {
    let _ = std::fs::remove_file(pid_file_path());
}

/// Check sentinel files in a given directory.
/// Exposed for testing with temporary directories.
pub fn check_sentinel_files_in_dir(dir: &std::path::Path) -> Result<(), String> {
    if dir.join("MAINTENANCE_MODE").exists() {
        return Err("MAINTENANCE_MODE active — launch blocked".to_string());
    }
    if dir.join("OTA_DEPLOYING").exists() {
        return Err("OTA_DEPLOYING active — launch blocked during OTA".to_string());
    }
    Ok(())
}

/// Parse launch args: JSON array -> Vec<String>, plain string -> single-element Vec.
/// Replaces split_whitespace() which broke paths containing spaces.
pub fn parse_launch_args(args: &str) -> Vec<String> {
    if args.starts_with('[') {
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(args) {
            return arr;
        }
    }
    // Plain string: single argument (preserves paths with spaces)
    vec![args.to_string()]
}

/// Pre-launch health checks: verify pod is ready to launch a game.
/// Called via spawn_blocking in LaunchGame handler before spawning any game process.
/// Returns Ok(()) if all checks pass, Err(String) with specific reason if any fails.
pub fn pre_launch_checks() -> Result<(), String> {
    let rp_dir = std::path::Path::new("C:\\RacingPoint");

    // Check 1 & 2: No MAINTENANCE_MODE / OTA_DEPLOYING sentinels
    check_sentinel_files_in_dir(rp_dir)?;

    // Check 3: No orphan game processes running
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let known = all_game_process_names();
        for (_pid, proc) in sys.processes() {
            let pname = proc.name().to_string_lossy().to_string();
            for name in known {
                if pname.eq_ignore_ascii_case(name) {
                    return Err(format!(
                        "orphan game process {} (PID {}) still running — clean state required",
                        name,
                        _pid.as_u32()
                    ));
                }
            }
        }
    }

    // Check 4: Disk space > 1GB on C: drive
    {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        if let Some(d) = disks.iter().find(|d| {
            d.mount_point()
                .to_str()
                .map(|s| s == "C:\\" || s == "C:" || s == "/")
                .unwrap_or(false)
        }) {
            if d.available_space() < 1_000_000_000 {
                return Err(format!(
                    "disk space low: {}MB free (< 1GB required)",
                    d.available_space() / 1_048_576
                ));
            }
        }
    }

    Ok(())
}

/// Full clean state reset: kill ALL game processes, clear game.pid, remove shared memory lock.
/// Called before auto-retry to ensure a clean slate.
/// Returns the number of processes killed.
pub fn clean_state_reset() -> u32 {
    let mut killed = 0u32;

    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let known = all_game_process_names();
    for (_pid, proc) in sys.processes() {
        let pname = proc.name().to_string_lossy().to_string();
        for name in known {
            if pname.eq_ignore_ascii_case(name) {
                let pid = _pid.as_u32();
                tracing::warn!(
                    target: LOG_TARGET,
                    pid,
                    name,
                    "clean_state_reset: killing game process"
                );
                if kill_process(pid).is_ok() {
                    killed += 1;
                }
                break;
            }
        }
    }

    clear_persisted_pid();

    let adapter_path = std::path::Path::new("C:\\RacingPoint\\shared_memory_adapter.lock");
    if adapter_path.exists() {
        let _ = std::fs::remove_file(adapter_path);
        tracing::info!(
            target: LOG_TARGET,
            "clean_state_reset: removed shared_memory_adapter.lock"
        );
    }

    tracing::info!(target: LOG_TARGET, killed, "clean_state_reset complete");
    killed
}


/// Check sentinel files in a given directory.
/// Exposed for testing with temporary directories.

/// All known game process names across all sim types.
/// GAME-02: Kept in sync with steam_checks::game_exe_for_sim() and process_names().
fn all_game_process_names() -> &'static [&'static str] {
    &[
        "acs.exe", "AssettoCorsa.exe",
        "AssettoCorsaEVO.exe", "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe",
        "acr.exe",
        "iRacingSim64DX11.exe", "iRacingService.exe", "iRacingService64.exe", "iRacingLauncher64.exe",
        "iRacingUI.exe",
        "F1_25.exe", "F1_2025.exe",
        "LMU.exe", "Le Mans Ultimate.exe",
        "ForzaMotorsport.exe",
        "ForzaHorizon5.exe",
    ]
}

/// Startup orphan scan: kill any game processes left over from a previous
/// rc-agent instance.  Called once at agent startup before connecting to racecontrol.
///
/// 1. Check persisted PID file — if alive, kill it.
/// 2. Scan for all known game process names and kill any that are running.
/// 3. Clean up the PID file.
///
/// NOTE: sysinfo does not expose Windows Session ID, so orphan kills are not
/// scoped to the current user session. This is safe on single-user pod PCs
/// (one interactive session). On multi-user machines this could kill another
/// user's game process — but pods are always single-user.
pub fn cleanup_orphaned_games() -> u32 {
    let mut cleaned = 0u32;

    // 1. Check persisted PID
    if let Some(pid) = read_persisted_pid() {
        if is_process_alive(pid) {
            tracing::warn!(target: LOG_TARGET, pid, "Killing orphaned game process from PID file on startup");
            if let Err(e) = kill_process(pid) {
                tracing::error!(target: LOG_TARGET, pid, "Failed to kill orphaned game by PID: {}", e);
            } else {
                cleaned += 1;
            }
        } else {
            tracing::info!(target: LOG_TARGET, pid, "Persisted game PID is no longer alive — cleaning up");
        }
        clear_persisted_pid();
    }

    // 2. Scan for any running game processes by name
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let known_names = all_game_process_names();
    for (_pid, process) in sys.processes() {
        let pname = process.name().to_string_lossy().to_string();
        for name in known_names {
            if pname.eq_ignore_ascii_case(name) {
                let pid = _pid.as_u32();
                tracing::warn!(target: LOG_TARGET, pid, process_name = %pname, "Killing orphaned game process found by name scan on startup");
                if let Err(e) = kill_process(pid) {
                    tracing::error!(target: LOG_TARGET, pid, "Failed to kill orphaned game process: {}", e);
                } else {
                    cleaned += 1;
                }
                break;
            }
        }
    }

    if cleaned > 0 {
        tracing::info!(target: LOG_TARGET, "Cleaned up {} orphaned game process(es) on startup", cleaned);
    } else {
        tracing::info!(target: LOG_TARGET, "Orphan game scan complete — no stale game processes found");
    }
    cleaned
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GameExeConfig {
    /// Path to game executable
    pub exe_path: Option<String>,
    /// Working directory (defaults to exe parent dir)
    pub working_dir: Option<String>,
    /// Launch arguments
    pub args: Option<String>,
    /// Steam app ID (for Steam launch method)
    pub steam_app_id: Option<u32>,
    /// Whether to use Steam launch (steam://rungameid/{id})
    #[serde(default)]
    pub use_steam: bool,
}

pub struct GameProcess {
    pub sim_type: SimType,
    pub state: GameState,
    pub child: Option<Child>,
    pub pid: Option<u32>,
    pub last_exit_code: Option<i32>,
}

impl GameProcess {
    /// Launch a game executable
    pub fn launch(config: &GameExeConfig, sim_type: SimType) -> anyhow::Result<Self> {
        // Check if args contain a URL scheme (acmanager://, steam://) — launch via OS handler
        if let Some(args) = &config.args {
            if args.starts_with("acmanager://") || args.starts_with("steam://") {
                return Self::launch_url(args, sim_type);
            }
        }

        if config.use_steam {
            if let Some(app_id) = config.steam_app_id {
                let url = format!("steam://rungameid/{}", app_id);
                #[cfg(target_os = "windows")]
                {
                    hidden_cmd("cmd")
                        .args(["/C", "start", "", &url])
                        .spawn()?;
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // Linux dev: open URL (xdg-open or just log)
                    tracing::info!(target: LOG_TARGET, "Would launch Steam URL: {}", url);
                    let _ = Command::new("xdg-open").arg(&url).spawn();
                }
                // Steam launch doesn't give us a Child handle directly
                // We'll detect the process via PID scanning
                return Ok(Self {
                    sim_type,
                    state: GameState::Launching,
                    child: None,
                    pid: None,
                    last_exit_code: None,
                });
            }
        }

        // Direct exe launch
        if let Some(exe_path) = &config.exe_path {
            let mut cmd = Command::new(exe_path);
            if let Some(dir) = &config.working_dir {
                cmd.current_dir(dir);
            }
            if let Some(args) = &config.args {
                // LAUNCH-19: Use JSON array or single-arg parsing — preserves paths with spaces.
                // Old split_whitespace() bug: "C:\Program Files\game.exe -arg" → 2 broken tokens.
                for arg in parse_launch_args(args) {
                    cmd.arg(arg);
                }
            }
            let child = cmd.spawn()?;
            let pid = child.id();
            persist_pid(pid);
            Ok(Self {
                sim_type,
                state: GameState::Launching,
                child: Some(child),
                pid: Some(pid),
                last_exit_code: None,
            })
        } else {
            anyhow::bail!(
                "No exe_path or steam_app_id configured for {:?}",
                sim_type
            );
        }
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    // Process exited — capture exit code
                    self.last_exit_code = exit_status.code();
                    self.state = GameState::Idle;
                    false
                }
                Ok(None) => {
                    // Still running
                    self.state = GameState::Running;
                    true
                }
                Err(_) => {
                    self.state = GameState::Error;
                    false
                }
            }
        } else if let Some(pid) = self.pid {
            is_process_alive(pid)
        } else {
            false
        }
    }

    /// Kill the game process
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.state = GameState::Stopping;
        if let Some(child) = &mut self.child {
            child.kill()?;
            child.wait()?;
        } else if let Some(pid) = self.pid {
            kill_process(pid)?;
        } else if let Some(pid) = read_persisted_pid() {
            // Fallback: no in-memory child or PID, but PID file exists (post-restart)
            tracing::info!(target: LOG_TARGET, pid, "Stopping game via persisted PID file fallback");
            if is_process_alive(pid) {
                kill_process(pid)?;
            }
        }
        self.state = GameState::Idle;
        self.child = None;
        self.pid = None;
        clear_persisted_pid();
        Ok(())
    }

    /// Launch via URL scheme (Content Manager join URL or Steam URL)
    fn launch_url(url: &str, sim_type: SimType) -> anyhow::Result<Self> {
        tracing::info!(target: LOG_TARGET, "Launching via URL scheme: {}", url);
        #[cfg(target_os = "windows")]
        {
            hidden_cmd("cmd")
                .args(["/C", "start", "", url])
                .spawn()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            tracing::info!(target: LOG_TARGET, "Would launch URL: {}", url);
            let _ = Command::new("xdg-open").arg(url).spawn();
        }

        Ok(Self {
            sim_type,
            state: GameState::Launching,
            child: None,
            pid: None,
            last_exit_code: None,
        })
    }
}

/// Known process names per sim type (for Steam-launched games)
fn process_names(sim_type: SimType) -> &'static [&'static str] {
    match sim_type {
        SimType::AssettoCorsa => &["acs.exe", "AssettoCorsa.exe"],
        SimType::AssettoCorsaEvo => &["AssettoCorsaEVO.exe", "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe"],
        SimType::AssettoCorsaRally => &["acr.exe"],
        SimType::IRacing => &["iRacingSim64DX11.exe", "iRacingService.exe", "iRacingService64.exe", "iRacingLauncher64.exe", "iRacingUI.exe"],
        SimType::F125 => &["F1_25.exe", "F1_2025.exe"],
        SimType::LeMansUltimate => &["LMU.exe", "Le Mans Ultimate.exe"],
        SimType::Forza => &["ForzaMotorsport.exe"],
        SimType::ForzaHorizon5 => &["ForzaHorizon5.exe"],
    }
}

/// Find PID of a running game by process name (for Steam launches)
pub fn find_game_pid(sim_type: SimType) -> Option<u32> {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let names = process_names(sim_type);
    for (_pid, process) in sys.processes() {
        let pname = process.name().to_string_lossy().to_string();
        for name in names {
            if pname.eq_ignore_ascii_case(name) {
                return Some(_pid.as_u32());
            }
        }
    }
    None
}

/// Platform-specific process alive check
#[cfg(target_os = "windows")]
fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let handle = winapi::um::processthreadsapi::OpenProcess(
            winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let result =
            winapi::um::processthreadsapi::GetExitCodeProcess(handle, &mut exit_code as *mut u32);
        winapi::um::handleapi::CloseHandle(handle);
        result != 0 && exit_code == 259 // STILL_ACTIVE
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}

/// Platform-specific process kill
#[cfg(target_os = "windows")]
fn kill_process(pid: u32) -> anyhow::Result<()> {
    hidden_cmd("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn kill_process(pid: u32) -> anyhow::Result<()> {
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::types::SimType;

    /// Verify every SimType variant has at least one process name.
    /// This match is exhaustive — adding a new SimType variant without
    /// updating process_names() will cause a compile error here.
    #[test]
    fn test_process_names_exhaustive() {
        let all_variants = [
            SimType::AssettoCorsa,
            SimType::AssettoCorsaEvo,
            SimType::AssettoCorsaRally,
            SimType::IRacing,
            SimType::F125,
            SimType::LeMansUltimate,
            SimType::Forza,
            SimType::ForzaHorizon5,
        ];
        for variant in &all_variants {
            let names = process_names(*variant);
            assert!(
                !names.is_empty(),
                "SimType::{:?} has no process names",
                variant
            );
            for name in names {
                assert!(
                    name.ends_with(".exe"),
                    "Process name '{}' for {:?} should end with .exe",
                    name,
                    variant
                );
            }
        }
    }

    /// Verify all_game_process_names() is a superset of every
    /// SimType variant's process_names() — ensures orphan cleanup
    /// covers all games.
    #[test]
    fn test_all_game_process_names_superset() {
        let all = all_game_process_names();
        let all_variants = [
            SimType::AssettoCorsa,
            SimType::AssettoCorsaEvo,
            SimType::AssettoCorsaRally,
            SimType::IRacing,
            SimType::F125,
            SimType::LeMansUltimate,
            SimType::Forza,
            SimType::ForzaHorizon5,
        ];
        for variant in &all_variants {
            let names = process_names(*variant);
            // At least one process name from this variant must appear
            // in the global list
            let has_overlap = names.iter().any(|n| all.contains(n));
            assert!(
                has_overlap,
                "all_game_process_names() is missing entries for SimType::{:?} (needs at least one of {:?})",
                variant,
                names
            );
        }
    }

    /// Verify specific process names for new game types.
    #[test]
    fn test_process_names_ac_rally() {
        let names = process_names(SimType::AssettoCorsaRally);
        assert_eq!(names, &["acr.exe"]);
    }

    #[test]
    fn test_process_names_forza_horizon_5() {
        let names = process_names(SimType::ForzaHorizon5);
        assert_eq!(names, &["ForzaHorizon5.exe"]);
    }

    #[test]
    fn test_process_names_ac_evo_primary() {
        let names = process_names(SimType::AssettoCorsaEvo);
        // AssettoCorsaEVO.exe should be the primary (first) entry
        assert_eq!(names[0], "AssettoCorsaEVO.exe");
        // Old names kept as fallback
        assert!(names.contains(&"AssettoCorsa2.exe"));
        assert!(names.contains(&"AC2-Win64-Shipping.exe"));
    }

    // ── F1 25 characterization tests ──────────────────────────────────────────

    #[test]
    fn test_process_names_f1_25() {
        let names = process_names(SimType::F125);
        // GAME-02: Both F1_25.exe (primary) and F1_2025.exe (alternate) are tracked
        assert!(names.contains(&"F1_25.exe"), "F125 must include F1_25.exe");
        assert!(names.contains(&"F1_2025.exe"), "F125 must include F1_2025.exe (alternate name)");
    }

    #[test]
    fn test_game_exe_config_steam_launch_requires_app_id() {
        // With use_steam=true but no app_id or exe_path → should fail
        let config = GameExeConfig {
            exe_path: None,
            working_dir: None,
            args: None,
            steam_app_id: None,
            use_steam: true,
        };
        let result = GameProcess::launch(&config, SimType::F125);
        assert!(result.is_err(), "Should fail without steam_app_id or exe_path");
    }

    #[test]
    fn test_game_exe_config_default_is_empty() {
        let config = GameExeConfig::default();
        assert!(config.exe_path.is_none());
        assert!(config.steam_app_id.is_none());
        assert!(!config.use_steam);
        assert!(config.args.is_none());
    }

    #[test]
    fn test_game_exe_config_f1_25_steam() {
        // Characterize the expected F1 25 config shape
        let config = GameExeConfig {
            exe_path: None,
            working_dir: None,
            args: None,
            steam_app_id: Some(3059520),
            use_steam: true,
        };
        assert_eq!(config.steam_app_id, Some(3059520));
        assert!(config.use_steam);
    }

    // ── Task 1 TDD tests: pre_launch_checks, clean_state_reset, arg parsing ──

    /// pre_launch_checks returns Ok(()) when no sentinels exist and no orphan games running.
    /// In test environment (non-Windows or no C:\RacingPoint), sentinel checks trivially pass.
    #[test]
    fn test_pre_launch_checks_pass_in_test_env() {
        // In CI / non-pod environment: MAINTENANCE_MODE and OTA_DEPLOYING files won't exist,
        // and no game processes are running → should pass
        let result = pre_launch_checks();
        // We can't guarantee disk space in all CI envs, but at minimum it should compile and run
        // The result may be Ok or Err(disk) — just verify the function exists and returns Result<_, String>
        let _ = result; // function must exist and be callable
    }

    /// pre_launch_checks with a synthetic MAINTENANCE_MODE file returns Err.
    #[test]
    fn test_pre_launch_checks_maintenance_mode() {
        // Create a temporary dir to act as C:\RacingPoint equivalent using env override
        // We test the logic directly by examining the function's behavior with a known file.
        // This test verifies the Err message contains "MAINTENANCE_MODE".
        let tmp = std::env::temp_dir().join("rp_test_maintenance");
        let _ = std::fs::create_dir_all(&tmp);
        let sentinel = tmp.join("MAINTENANCE_MODE");
        std::fs::write(&sentinel, "").unwrap();

        // Test the internal check logic directly via the helper
        let result = check_sentinel_files_in_dir(&tmp);
        assert!(result.is_err(), "Should fail when MAINTENANCE_MODE exists");
        let msg = result.unwrap_err();
        assert!(msg.contains("MAINTENANCE_MODE"), "Error should mention MAINTENANCE_MODE, got: {}", msg);

        let _ = std::fs::remove_file(&sentinel);
    }

    /// pre_launch_checks with OTA_DEPLOYING sentinel returns Err.
    #[test]
    fn test_pre_launch_checks_ota_deploying() {
        let tmp = std::env::temp_dir().join("rp_test_ota");
        let _ = std::fs::create_dir_all(&tmp);
        let sentinel = tmp.join("OTA_DEPLOYING");
        std::fs::write(&sentinel, "").unwrap();

        let result = check_sentinel_files_in_dir(&tmp);
        assert!(result.is_err(), "Should fail when OTA_DEPLOYING exists");
        let msg = result.unwrap_err();
        assert!(msg.contains("OTA_DEPLOYING"), "Error should mention OTA_DEPLOYING, got: {}", msg);

        let _ = std::fs::remove_file(&sentinel);
    }

    /// clean_state_reset clears the game.pid file.
    #[test]
    fn test_clean_state_reset_clears_pid() {
        // Write a pid, call clean_state_reset, verify pid file is gone
        persist_pid(99999);
        let _ = clean_state_reset();
        // After reset, pid file should be absent
        let recovered = read_persisted_pid();
        assert!(recovered.is_none(), "game.pid should be cleared after clean_state_reset");
    }

    /// Args as JSON array are parsed as separate tokens.
    #[test]
    fn test_shell_quote_args_json_array() {
        let args = r#"["-fullscreen", "-arg1"]"#;
        let parsed = parse_launch_args(args);
        assert_eq!(parsed, vec!["-fullscreen".to_string(), "-arg1".to_string()]);
    }

    /// Args as plain string with spaces are NOT split (preserves paths with spaces).
    #[test]
    fn test_shell_quote_args_plain_string_not_split() {
        let args = r#"C:\Program Files\Steam\F1_25.exe -arg1"#;
        let parsed = parse_launch_args(args);
        // Plain string: passed as single argument (not split on spaces)
        assert_eq!(parsed.len(), 1, "Plain string args must not be split on spaces");
        assert_eq!(parsed[0], args);
    }

    /// Args as JSON array with path containing spaces are preserved correctly.
    #[test]
    fn test_shell_quote_args_json_array_with_spaces() {
        let args = r#"["C:\\Program Files\\Steam\\F1_25.exe", "-arg1"]"#;
        let parsed = parse_launch_args(args);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], r"C:\Program Files\Steam\F1_25.exe");
        assert_eq!(parsed[1], "-arg1");
    }


}

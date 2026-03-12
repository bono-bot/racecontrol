use std::path::PathBuf;
use std::process::{Child, Command};

use rc_common::types::{GameState, SimType};
use serde::Deserialize;

/// Directory where the PID file is persisted.
/// Windows: C:\RaceControl\  Linux: /tmp/racecontrol/
fn pid_file_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    { PathBuf::from(r"C:\RaceControl") }
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
        tracing::warn!("Failed to persist game PID {}: {}", pid, e);
    } else {
        tracing::debug!("Persisted game PID {} to {:?}", pid, pid_file_path());
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

/// All known game process names across all sim types.
fn all_game_process_names() -> &'static [&'static str] {
    &[
        "acs.exe", "AssettoCorsa.exe",
        "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe",
        "iRacingSim64DX11.exe", "iRacingService.exe",
        "F1_25.exe",
        "LMU.exe", "Le Mans Ultimate.exe",
        "ForzaMotorsport.exe",
    ]
}

/// Startup orphan scan: kill any game processes left over from a previous
/// rc-agent instance.  Called once at agent startup before connecting to rc-core.
///
/// 1. Check persisted PID file — if alive, kill it.
/// 2. Scan for all known game process names and kill any that are running.
/// 3. Clean up the PID file.
pub fn cleanup_orphaned_games() -> u32 {
    let mut cleaned = 0u32;

    // 1. Check persisted PID
    if let Some(pid) = read_persisted_pid() {
        if is_process_alive(pid) {
            tracing::warn!(pid, "Killing orphaned game process from PID file on startup");
            if let Err(e) = kill_process(pid) {
                tracing::error!(pid, "Failed to kill orphaned game by PID: {}", e);
            } else {
                cleaned += 1;
            }
        } else {
            tracing::info!(pid, "Persisted game PID is no longer alive — cleaning up");
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
                tracing::warn!(pid, process_name = %pname, "Killing orphaned game process found by name scan on startup");
                if let Err(e) = kill_process(pid) {
                    tracing::error!(pid, "Failed to kill orphaned game process: {}", e);
                } else {
                    cleaned += 1;
                }
                break;
            }
        }
    }

    if cleaned > 0 {
        tracing::info!("Cleaned up {} orphaned game processes on startup", cleaned);
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
                    Command::new("cmd")
                        .args(["/C", "start", "", &url])
                        .spawn()?;
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // Linux dev: open URL (xdg-open or just log)
                    tracing::info!("Would launch Steam URL: {}", url);
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
                for arg in args.split_whitespace() {
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
            tracing::info!(pid, "Stopping game via persisted PID file fallback");
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
        tracing::info!("Launching via URL scheme: {}", url);
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            tracing::info!("Would launch URL: {}", url);
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
        SimType::AssettoCorsaEvo => &["AssettoCorsa2.exe", "AC2-Win64-Shipping.exe"],
        SimType::AssettoCorsaRally => &["AssettoCorsa2.exe"], // TODO(Plan 02): confirm AC Rally process name
        SimType::IRacing => &["iRacingSim64DX11.exe", "iRacingService.exe"],
        SimType::F125 => &["F1_25.exe"],
        SimType::LeMansUltimate => &["LMU.exe", "Le Mans Ultimate.exe"],
        SimType::Forza => &["ForzaMotorsport.exe"],
        SimType::ForzaHorizon5 => &["ForzaHorizon5.exe"], // TODO(Plan 02): confirm FH5 process name
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
    Command::new("taskkill")
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

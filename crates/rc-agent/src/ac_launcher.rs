//! Assetto Corsa full launch sequence for sim racing pods.
//!
//! Flow: Kill AC → Write race.ini → Launch acs.exe → Wait → Minimize Conspit Link
//! Requires: CSP gui.ini already patched with FORCE_START=1 (one-time setup)

use std::process::Command;
use std::path::Path;
use std::io::Write;
use anyhow::Result;
use serde::Deserialize;

use crate::lock_screen;

/// Dialog/system processes that must be killed between sessions to ensure a clean kiosk state.
/// Includes crash reporters, settings windows, and system dialogs that can appear after a game crash.
pub const DIALOG_PROCESSES: &[&str] = &[
    "WerFault.exe",
    "WerFaultSecure.exe",
    "ApplicationFrameHost.exe",
    "SystemSettings.exe",
    "msiexec.exe",
];

/// AC launch parameters parsed from the `launch_args` JSON
#[derive(Debug, Clone, Deserialize)]
pub struct AcLaunchParams {
    pub car: String,
    pub track: String,
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default)]
    pub track_config: String,
    #[serde(default = "default_skin")]
    pub skin: String,
    #[serde(default = "default_transmission")]
    pub transmission: String,
    #[serde(default = "default_ffb")]
    pub ffb: String,
    #[serde(default)]
    pub aids: Option<AcAids>,
    #[serde(default)]
    pub conditions: Option<AcConditions>,
    #[serde(default = "default_duration")]
    pub duration_minutes: u32,
    #[serde(default)]
    pub game_mode: String,
    #[serde(default)]
    pub server_ip: String,
    #[serde(default)]
    pub server_port: u16,
    #[serde(default)]
    pub server_http_port: u16,
    #[serde(default)]
    pub server_password: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AcAids {
    #[serde(default = "one")]
    pub abs: u8,
    #[serde(default = "one")]
    pub tc: u8,
    #[serde(default = "one")]
    pub stability: u8,
    #[serde(default = "one")]
    pub autoclutch: u8,
    #[serde(default)]
    pub ideal_line: u8,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AcConditions {
    #[serde(default)]
    pub damage: u8,
}

fn default_driver() -> String { "Driver".to_string() }
fn default_skin() -> String { "00_default".to_string() }
fn default_transmission() -> String { "manual".to_string() }
fn default_ffb() -> String { "medium".to_string() }
fn default_duration() -> u32 { 60 }
fn one() -> u8 { 1 }

/// Result from AC launch — carries PID and optional CM error for debug reporting.
#[derive(Debug)]
pub struct LaunchResult {
    pub pid: u32,
    /// If CM was used (multiplayer) and failed, this contains the error details.
    /// The game may still be running via direct acs.exe fallback.
    pub cm_error: Option<String>,
}

/// Runs the full AC launch sequence. Blocks for ~10 seconds.
pub fn launch_ac(params: &AcLaunchParams) -> Result<LaunchResult> {
    tracing::info!("AC launch: {} @ {} for {}", params.car, params.track, params.driver);

    // Step 1: Kill existing AC
    tracing::info!("[1/4] Killing existing AC...");
    let _ = Command::new("taskkill")
        .args(["/IM", "acs.exe", "/F"])
        .output();
    let _ = Command::new("taskkill")
        .args(["/IM", "AssettoCorsa.exe", "/F"])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Step 2: Write race.ini + assists.ini + apps preset
    tracing::info!("[2/4] Writing race.ini + assists.ini + apps preset...");
    write_race_ini(params)?;
    write_assists_ini(params)?;
    write_apps_preset()?;

    // Step 2b: Set FFB strength
    set_ffb(&params.ffb)?;

    // Step 3: Launch AC
    // - Multiplayer: use Content Manager (handles server join handshake)
    // - Single-player: launch acs.exe directly (race.ini already written above)
    //   CM's acmanager://race/config fails with "Settings are not specified"
    //   if CM's Quick Drive preset was never configured on this pod.
    let mut cm_error: Option<String> = None;

    let pid = if params.game_mode == "multi" && find_cm_exe().is_some() {
        tracing::info!("[3/5] Launching multiplayer via Content Manager...");
        launch_via_cm(params)?;
        match wait_for_ac_process(15) {
            Ok(pid) => pid,
            Err(e) => {
                // CM failed — gather diagnostic info before falling back
                let cm_diag = diagnose_cm_failure();
                let error_detail = format!(
                    "CM multiplayer launch failed: {}. Diagnostics: {}",
                    e, cm_diag
                );
                tracing::error!("[CM_ERROR] {}", error_detail);
                cm_error = Some(error_detail);

                // Fall back to direct acs.exe (race.ini has [REMOTE] ACTIVE=1)
                tracing::warn!("Falling back to direct acs.exe launch for multiplayer...");
                let ac_dir = find_ac_dir()?;
                let child = Command::new(ac_dir.join("acs.exe"))
                    .current_dir(&ac_dir)
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("Failed to launch acs.exe: {}", e))?;
                child.id()
            }
        }
    } else {
        tracing::info!("[3/5] Launching acs.exe directly (race.ini pre-written)...");
        let ac_dir = find_ac_dir()?;
        let child = Command::new(ac_dir.join("acs.exe"))
            .current_dir(&ac_dir)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch acs.exe: {}", e))?;
        child.id()
    };
    tracing::info!("AC launched with PID {}", pid);

    // Step 4: Wait for AC to load, then minimize Conspit Link
    // (Don't kill Conspit Link — it crashes on force-restart. Just minimize it.)
    tracing::info!("[4/5] Waiting 8s for AC to load, then minimizing Conspit Link...");
    std::thread::sleep(std::time::Duration::from_secs(8));
    minimize_conspit_window();

    // Step 5: Minimize background windows and bring game to foreground
    tracing::info!("[5/5] Minimizing background windows and focusing game...");
    std::thread::sleep(std::time::Duration::from_secs(2));
    minimize_background_windows();
    bring_game_to_foreground();

    Ok(LaunchResult { pid, cm_error })
}

/// Update AUTO_SHIFTER in race.ini without restarting AC.
/// Customer can press Ctrl+R or restart from pits for it to take effect.
pub fn set_transmission(transmission: &str) -> Result<()> {
    let race_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("race.ini");

    let content = std::fs::read_to_string(&race_ini_path)
        .map_err(|e| anyhow::anyhow!("Failed to read race.ini: {}", e))?;

    let new_value = if transmission == "auto" || transmission == "automatic" { "1" } else { "0" };
    let updated = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("AUTO_SHIFTER=") {
                format!("AUTO_SHIFTER={}", new_value)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    std::fs::write(&race_ini_path, &updated)?;
    tracing::info!("Updated race.ini AUTO_SHIFTER={} (transmission={})", new_value, transmission);

    // Also update assists.ini to prevent CSP/CM override
    let assists_ini_path = race_ini_path.with_file_name("assists.ini");
    if assists_ini_path.exists() {
        let assists_content = std::fs::read_to_string(&assists_ini_path)
            .map_err(|e| anyhow::anyhow!("Failed to read assists.ini: {}", e))?;
        let assists_updated = assists_content
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("AUTO_SHIFTER=") {
                    format!("AUTO_SHIFTER={}", new_value)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\r\n");
        std::fs::write(&assists_ini_path, &assists_updated)?;
        tracing::info!("Updated assists.ini AUTO_SHIFTER={}", new_value);
    }

    Ok(())
}

/// Update FFB gain in controls.ini. Preset: light=40, medium=70, strong=100.
/// Takes effect on next AC launch (or restart mid-session).
pub fn set_ffb(preset: &str) -> Result<()> {
    let gain = match preset {
        "light" => 40,
        "medium" => 70,
        "strong" => 100,
        _ => 70, // default to medium
    };

    let controls_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("controls.ini");

    let content = std::fs::read_to_string(&controls_ini_path)
        .map_err(|e| anyhow::anyhow!("Failed to read controls.ini: {}", e))?;

    let mut in_ff_section = false;
    let mut found = false;
    let updated: Vec<String> = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_ff_section = trimmed == "[FF]";
            }
            if in_ff_section && trimmed.starts_with("GAIN=") {
                found = true;
                return format!("GAIN={}", gain);
            }
            line.to_string()
        })
        .collect();

    if !found {
        tracing::warn!("No [FF] GAIN= line found in controls.ini, skipping FFB update");
        return Ok(());
    }

    std::fs::write(&controls_ini_path, updated.join("\r\n"))?;
    tracing::info!("Updated controls.ini [FF] GAIN={} (preset={})", gain, preset);
    Ok(())
}

/// Write race.ini with AUTOSPAWN=1 and the given car/track/driver
fn write_race_ini(params: &AcLaunchParams) -> Result<()> {
    let race_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("race.ini");

    let track_config = if params.track_config.is_empty() {
        String::new()
    } else {
        params.track_config.clone()
    };

    let aids = params.aids.clone().unwrap_or_default();
    let damage = params.conditions.as_ref().map(|c| c.damage).unwrap_or(0);
    let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

    let content = format!(r#"[ASSISTS]
ABS={abs}
AUTO_CLUTCH={autoclutch}
AUTO_SHIFTER={auto_shifter}
DAMAGE={damage}
IDEAL_LINE={ideal_line}
STABILITY={stability}
TRACTION_CONTROL={tc}
VISUAL_DAMAGE=0
SLIPSTREAM=1
TYRE_BLANKETS=1
AUTO_BLIP=1
FUEL_RATE=1

[AUTOSPAWN]
ACTIVE=1

[BENCHMARK]
ACTIVE=0

[CAR_0]
SETUP=
SKIN={skin}
MODEL={car}
MODEL_CONFIG=
BALLAST=0
RESTRICTOR=0
DRIVER_NAME={driver}
NATIONALITY=IND
NATION_CODE=IND

[DYNAMIC_TRACK]
LAP_GAIN=0
RANDOMNESS=0
SESSION_START=100
SESSION_TRANSFER=100

[GHOST_CAR]
ENABLED=0
FILE=
LOAD=0
PLAYING=0
RECORDING=0
SECONDS_ADVANTAGE=0

[GROOVE]
VIRTUAL_LAPS=10
MAX_LAPS=30
STARTING_LAPS=0

[HEADER]
VERSION=2

[LAP_INVALIDATOR]
ALLOWED_TYRES_OUT=-1

[LIGHTING]
CLOUD_SPEED=0.200
SUN_ANGLE=16
TIME_MULT=1.0

[OPTIONS]
USE_MPH=0

[RACE]
AI_LEVEL=100
CARS=1
CONFIG_TRACK={track_config}
DRIFT_MODE=0
FIXED_SETUP=0
JUMP_START_PENALTY=0
MODEL={car}
MODEL_CONFIG=
PENALTIES=1
RACE_LAPS=0
SKIN={skin}
TRACK={track}

[REMOTE]
ACTIVE={remote_active}
GUID=
NAME={driver}
PASSWORD={server_password}
SERVER_IP={server_ip}
SERVER_PORT={server_port}
TEAM=

[REPLAY]
ACTIVE=0
FILENAME=

[RESTART]
ACTIVE=0

[SESSION_0]
NAME=Practice
DURATION_MINUTES={duration_minutes}
SPAWN_SET=PIT
TYPE=1
LAPS=0
STARTING_POSITION=1

[TEMPERATURE]
AMBIENT=22
ROAD=28

[WEATHER]
NAME=3_clear

[WIND]
DIRECTION_DEG=0
SPEED_KMH_MAX=0
SPEED_KMH_MIN=0"#,
        abs = aids.abs,
        autoclutch = aids.autoclutch,
        auto_shifter = auto_shifter,
        damage = damage,
        ideal_line = aids.ideal_line,
        stability = aids.stability,
        tc = aids.tc,
        car = params.car,
        track = params.track,
        track_config = track_config,
        driver = params.driver,
        skin = params.skin,
        duration_minutes = params.duration_minutes,
        remote_active = if params.game_mode == "multi" { 1 } else { 0 },
        server_ip = params.server_ip,
        server_port = params.server_port,
        server_password = params.server_password,
    );

    if let Some(parent) = race_ini_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(&race_ini_path)?;
    file.write_all(content.as_bytes())?;
    tracing::info!("Wrote race.ini to {}", race_ini_path.display());
    Ok(())
}

/// Write assists.ini to override Content Manager / CSP cached assists.
/// AC and CSP may read assists from this file instead of race.ini's [ASSISTS].
fn write_assists_ini(params: &AcLaunchParams) -> Result<()> {
    let assists_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("assists.ini");

    let aids = params.aids.clone().unwrap_or_default();
    let damage = params.conditions.as_ref().map(|c| c.damage).unwrap_or(0);
    let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

    let content = format!(
        "[ASSISTS]\r\nABS={abs}\r\nAUTO_CLUTCH={autoclutch}\r\nAUTO_SHIFTER={auto_shifter}\r\nDAMAGE={damage}\r\nIDEAL_LINE={ideal_line}\r\nSTABILITY={stability}\r\nTRACTION_CONTROL={tc}\r\nVISUAL_DAMAGE=0\r\nSLIPSTREAM=1\r\nTYRE_BLANKETS=1\r\nAUTO_BLIP=1\r\nFUEL_RATE=1\r\n",
        abs = aids.abs,
        autoclutch = aids.autoclutch,
        auto_shifter = auto_shifter,
        damage = damage,
        ideal_line = aids.ideal_line,
        stability = aids.stability,
        tc = aids.tc,
    );

    if let Some(parent) = assists_ini_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(&assists_ini_path)?;
    file.write_all(content.as_bytes())?;
    tracing::info!(
        "Wrote assists.ini: DAMAGE={}, AUTO_SHIFTER={} (transmission={})",
        damage, auto_shifter, params.transmission
    );
    Ok(())
}

/// Find Content Manager executable on the pod.
/// Checks common install locations used on our pods.
fn find_cm_exe() -> Option<std::path::PathBuf> {
    let candidates = [
        r"C:\Users\User\Desktop\Content Manager.exe",
        r"C:\Users\User\Desktop\content-manager\Content Manager.exe",
        r"C:\RacingPoint\Content Manager.exe",
        r"C:\Users\bono\Desktop\Content Manager.exe",
    ];
    for path in &candidates {
        let p = Path::new(path);
        if p.exists() {
            tracing::info!("Found Content Manager at {}", path);
            return Some(p.to_path_buf());
        }
    }
    tracing::warn!("Content Manager not found in any known location");
    None
}

/// Launch AC via Content Manager's acmanager:// URI protocol.
/// For single-player: `acmanager://race/config` (uses current race.ini)
/// For multiplayer: `acmanager://race/online?ip=...&httpPort=...&password=...`
fn launch_via_cm(params: &AcLaunchParams) -> Result<()> {
    let uri = if params.game_mode == "multi" {
        let mut uri = format!(
            "acmanager://race/online?ip={}&httpPort={}",
            params.server_ip, params.server_http_port,
        );
        if !params.server_password.is_empty() {
            uri.push_str(&format!("&password={}", params.server_password));
        }
        uri
    } else {
        "acmanager://race/config".to_string()
    };

    tracing::info!("Launching via Content Manager URI: {}", uri);
    Command::new("cmd")
        .args(["/c", "start", "", &uri])
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to open acmanager:// URI: {}", e))?;

    Ok(())
}

/// Poll for acs.exe process to appear (CM launches it as a child process).
/// Returns the PID once found, or an error after timeout.
fn wait_for_ac_process(timeout_secs: u64) -> Result<u32> {
    let poll_interval = std::time::Duration::from_millis(500);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        if let Some(pid) = find_acs_pid() {
            tracing::info!("Found acs.exe with PID {}", pid);
            return Ok(pid);
        }
        std::thread::sleep(poll_interval);
    }

    anyhow::bail!("acs.exe did not appear within {}s after CM launch", timeout_secs)
}

/// Find acs.exe PID via tasklist.
fn find_acs_pid() -> Option<u32> {
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq acs.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // CSV format: "acs.exe","12345","Console","1","123,456 K"
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

/// Find the AC installation directory
fn find_ac_dir() -> Result<std::path::PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa",
        r"C:\Program Files\Steam\steamapps\common\assettocorsa",
        r"D:\SteamLibrary\steamapps\common\assettocorsa",
    ];
    for dir in &candidates {
        let p = Path::new(dir);
        if p.join("acs.exe").exists() {
            return Ok(p.to_path_buf());
        }
    }
    anyhow::bail!("AC installation not found");
}

/// Diagnose why Content Manager failed to launch AC.
/// Checks: CM process state, CM log files, error dialog windows.
fn diagnose_cm_failure() -> String {
    let mut findings = Vec::new();

    // 1. Check if CM process is still running (might be showing error dialog)
    if let Some(cm_info) = check_cm_process() {
        findings.push(cm_info);
    }

    // 2. Check CM log files for recent errors
    if let Some(log_error) = read_cm_log_errors() {
        findings.push(format!("CM log: {}", log_error));
    }

    // 3. Check for WerFault (crash dialog)
    if is_process_running("WerFault.exe") {
        findings.push("WerFault.exe detected (crash dialog showing)".to_string());
    }

    if findings.is_empty() {
        "No specific CM error found — CM may have silently failed or shown a GUI dialog".to_string()
    } else {
        findings.join("; ")
    }
}

/// Check if Content Manager process is running and what state it's in.
fn check_cm_process() -> Option<String> {
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Content Manager.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();

    if trimmed.is_empty() || trimmed.contains("No tasks") {
        Some("CM process not running (may have crashed or was never launched)".to_string())
    } else {
        // CM is running but didn't spawn acs.exe — likely stuck on error dialog
        Some("CM process alive but acs.exe not spawned (probable error dialog)".to_string())
    }
}

/// Read Content Manager log files for recent error messages.
/// CM stores logs in its data directory (next to exe) or %LOCALAPPDATA%.
fn read_cm_log_errors() -> Option<String> {
    let log_paths = build_cm_log_paths();

    for log_path in &log_paths {
        if let Ok(content) = std::fs::read_to_string(log_path) {
            // Look at the last 2000 chars for recent errors
            let tail = if content.len() > 2000 {
                &content[content.len() - 2000..]
            } else {
                &content
            };

            // Search for CM error patterns
            let error_patterns = [
                "Request Cannot be processed",
                "Settings are not specified",
                "Cannot connect",
                "Server is not available",
                "Connection refused",
                "Oops",
                "Exception",
                "Error:",
                "FATAL",
                "failed to join",
                "booking is not available",
            ];

            let mut found_errors = Vec::new();
            for line in tail.lines().rev().take(50) {
                for pattern in &error_patterns {
                    if line.to_lowercase().contains(&pattern.to_lowercase()) {
                        let trimmed = line.trim();
                        if trimmed.len() <= 200 {
                            found_errors.push(trimmed.to_string());
                        } else {
                            found_errors.push(format!("{}...", &trimmed[..200]));
                        }
                        break;
                    }
                }
            }

            if !found_errors.is_empty() {
                found_errors.truncate(3); // Max 3 error lines
                return Some(found_errors.join(" | "));
            }
        }
    }

    None
}

/// Build list of possible CM log file paths to check.
fn build_cm_log_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // Check next to each known CM exe location
    for cm_dir in &[
        r"C:\Users\User\Desktop\content-manager",
        r"C:\Users\User\Desktop",
        r"C:\RacingPoint",
        r"C:\Users\bono\Desktop",
    ] {
        let base = Path::new(cm_dir);
        // CM stores logs in Data/Logs/ or Logs/ subfolder
        paths.push(base.join("Data").join("Logs").join("Main Log.txt"));
        paths.push(base.join("Data").join("Logs").join("log.txt"));
        paths.push(base.join("Logs").join("Main Log.txt"));
        paths.push(base.join("Logs").join("log.txt"));
    }

    // %LOCALAPPDATA% locations
    if let Some(local_app) = dirs_next::data_local_dir() {
        for dir_name in &["AcTools Content Manager", "AcManager", "AcTools"] {
            let base = local_app.join(dir_name);
            paths.push(base.join("Logs").join("Main Log.txt"));
            paths.push(base.join("Logs").join("log.txt"));
            paths.push(base.join("Log.txt"));
        }
    }

    paths
}

/// Check if a process is currently running by image name.
fn is_process_running(name: &str) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", name), "/FO", "CSV", "/NH"])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            !s.trim().is_empty() && !s.contains("No tasks")
        })
        .unwrap_or(false)
}

/// Force-minimize ConspitLink window using Windows API (WPF ignores start /min).
/// Tries multiple window title patterns since the WPF title may differ from the
/// process name, then falls back to PowerShell process enumeration.
fn minimize_conspit_window() {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        fn wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }

        // Try multiple possible window titles (WPF title may have spaces)
        let titles = [
            "Conspit Link 2.0",
            "ConspitLink2.0",
            "Conspit Link",
            "ConspitLink",
        ];
        for title in &titles {
            unsafe {
                let title_wide = wide(title);
                let hwnd = winapi::um::winuser::FindWindowW(std::ptr::null(), title_wide.as_ptr());
                if !hwnd.is_null() {
                    winapi::um::winuser::ShowWindow(hwnd, winapi::um::winuser::SW_MINIMIZE);
                    tracing::info!("Conspit Link minimized via FindWindowW(\"{}\")", title);
                    return;
                }
            }
        }

        // Fallback: use PowerShell to minimize by process name (wildcard for safety)
        let result = Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "Add-Type -Name W -Namespace N -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool ShowWindow(IntPtr h, int c);'; Get-Process -Name ConspitLink* -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | ForEach-Object { [N.W]::ShowWindow($_.MainWindowHandle, 6); Write-Output \"Minimized: $($_.ProcessName)\" }"
            ])
            .output();
        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim().is_empty() {
                    tracing::warn!("Conspit Link: no window found to minimize (not running?)");
                } else {
                    tracing::info!("Conspit Link minimized via PowerShell: {}", stdout.trim());
                }
            }
            Err(e) => tracing::warn!("Conspit Link minimize PowerShell failed: {}", e),
        }
    }
}

/// Check if Conspit Link is running; if not, restart it and minimize after a delay.
/// Called periodically from the main loop as a crash watchdog.
pub fn ensure_conspit_link_running() {
    let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
    if !Path::new(conspit_path).exists() {
        return; // Not installed on this pod
    }

    if is_process_running("ConspitLink2.0.exe") {
        return; // Already running, nothing to do
    }

    tracing::warn!("Conspit Link not running — restarting (crash recovery)...");
    match Command::new("cmd")
        .args(["/c", "start", "", conspit_path])
        .spawn()
    {
        Ok(_) => {
            tracing::info!("Conspit Link restarted, will minimize in 4s (non-blocking)...");
            // Spawn a thread to wait and minimize — don't block the main loop
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(4));
                minimize_conspit_window();
            });
        }
        Err(e) => tracing::error!("Failed to restart Conspit Link: {}", e),
    }
}

/// Write apps preset to enable sector times and essential HUD elements.
/// This writes to the Documents/Assetto Corsa/cfg/ folder.
fn write_apps_preset() -> Result<()> {
    let cfg_dir = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg");

    // Enable sectors app in apps-default.ini (controls which HUD apps are visible)
    let apps_ini_path = cfg_dir.join("apps-default.ini");
    let content = "[SECTORS]
ACTIVE=1
X=400
Y=50
WIDTH=200
HEIGHT=150
VISIBLE=1

[SPEEDOMETER]
ACTIVE=1
X=800
Y=600
WIDTH=200
HEIGHT=200
VISIBLE=1

[LAPTIME]
ACTIVE=1
X=600
Y=50
WIDTH=200
HEIGHT=80
VISIBLE=1
";
    let mut file = std::fs::File::create(&apps_ini_path)?;
    file.write_all(content.as_bytes())?;
    tracing::info!("Wrote apps preset to {}", apps_ini_path.display());
    Ok(())
}

/// Minimize all visible windows except the game, overlay, and essential system processes.
/// Uses an allow-list approach: anything not on the list gets minimized.
pub fn minimize_background_windows() {
    let ps_script = r#"
        Add-Type -Name WinMin -Namespace NativeMin -MemberDefinition '
            [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
            [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
            [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
        '
        # Processes whose windows we must NOT minimize
        $allowList = @(
            'acs', 'AssettoCorsa',                          # Game
            'msedge', 'msedgewebview2',                     # Overlay / Kiosk (Edge)
            'explorer',                                      # Shell (taskbar/desktop)
            'TextInputHost', 'ShellExperienceHost',          # System UI
            'SearchHost', 'StartMenuExperienceHost',         # System UI
            'SecurityHealthSystray', 'ctfmon',               # System tray
            'rc-agent',                                      # Our agent
            'Content Manager'                                # CM monitors game lifecycle
            # ConspitLink2.0 intentionally NOT listed — minimize it so kiosk stays on top
            # (Conspit still captures telemetry while minimized)
        )
        # SW_MINIMIZE = 6
        Get-Process | Where-Object {
            $_.MainWindowHandle -ne [IntPtr]::Zero -and
            $allowList -notcontains $_.ProcessName
        } | ForEach-Object {
            $hWnd = $_.MainWindowHandle
            if ([NativeMin.WinMin]::IsWindowVisible($hWnd) -and -not [NativeMin.WinMin]::IsIconic($hWnd)) {
                [NativeMin.WinMin]::ShowWindow($hWnd, 6) | Out-Null
                Write-Output "Minimized: $($_.ProcessName) (PID $($_.Id))"
            }
        }
    "#;
    match Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                tracing::info!("minimize_background_windows: {}", stdout.trim());
            }
        }
        Err(e) => tracing::warn!("minimize_background_windows failed: {}", e),
    }
}

/// Bring the AC game window to the foreground so it's visible.
/// Must be called after minimize_background_windows() since that may minimize the game.
fn bring_game_to_foreground() {
    #[cfg(windows)]
    {
        use std::ptr;
        unsafe {
            // Try known AC window class/title patterns
            for title in &["Assetto Corsa\0", "AC\0"] {
                let title_wide: Vec<u16> = title.encode_utf16().collect();
                let hwnd = winapi::um::winuser::FindWindowW(ptr::null(), title_wide.as_ptr());
                if !hwnd.is_null() {
                    winapi::um::winuser::ShowWindow(hwnd, winapi::um::winuser::SW_RESTORE);
                    winapi::um::winuser::SetForegroundWindow(hwnd);
                    tracing::info!("Brought AC window to foreground via FindWindowW");
                    return;
                }
            }
        }
        // Fallback: use PowerShell to find acs.exe window and foreground it
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Add-Type -Name WF -Namespace NF -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport(\"user32.dll\")] public static extern bool ShowWindow(IntPtr h, int c);'; \
                 Get-Process acs -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | ForEach-Object { [NF.WF]::ShowWindow($_.MainWindowHandle, 9); [NF.WF]::SetForegroundWindow($_.MainWindowHandle) }"])
            .output();
        tracing::info!("Brought AC window to foreground via PowerShell fallback");
    }
}

/// Full pod cleanup after a session ends.
/// Kills game, dismisses error dialogs, minimizes background windows
/// (including Conspit Link), and ensures the lock screen is in the foreground.
pub fn cleanup_after_session() {
    tracing::info!("[cleanup] Starting post-session cleanup...");

    // 1. Kill AC and Content Manager (Conspit Link stays running — minimized in step 3)
    let _ = Command::new("taskkill").args(["/IM", "acs.exe", "/F"]).output();
    let _ = Command::new("taskkill").args(["/IM", "AssettoCorsa.exe", "/F"]).output();
    let _ = Command::new("taskkill").args(["/IM", "Content Manager.exe", "/F"]).output();
    tracing::info!("[cleanup] Killed AC + Content Manager (Conspit Link kept alive)");

    // 2. Kill error/crash dialogs and system popups
    for proc in DIALOG_PROCESSES {
        let _ = Command::new("taskkill").args(["/IM", proc, "/F"]).output();
    }
    tracing::info!("[cleanup] Dismissed error dialogs and system popups");

    // 3. Minimize all background windows, bring lock screen to foreground
    let ps_script = r#"
        Add-Type -Name Win -Namespace Native -MemberDefinition '
            [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
            [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
        '
        # Minimize Steam, Conspit, Settings, NVIDIA overlay
        Get-Process -Name steam,steamwebhelper,ConspitLink2.0,SystemSettings,ApplicationFrameHost -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
            ForEach-Object { [Native.Win]::ShowWindow($_.MainWindowHandle, 6) }

        # Close Settings windows
        Get-Process -Name SystemSettings,ApplicationFrameHost -ErrorAction SilentlyContinue |
            ForEach-Object { $_.CloseMainWindow() }

        # Bring lock screen browser (msedge "Racing Point") to foreground and maximize
        $edge = Get-Process -Name msedge -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowTitle -match 'Racing Point' } |
            Select-Object -First 1
        if ($edge) {
            [Native.Win]::SetForegroundWindow($edge.MainWindowHandle)
            [Native.Win]::ShowWindow($edge.MainWindowHandle, 3)  # SW_MAXIMIZE
        }
    "#;
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output();
    tracing::info!("[cleanup] Background windows minimized, lock screen foregrounded");
}

/// Enforce a known-good safe state on the pod — the "factory reset" for runtime.
/// Kills ALL game processes (not just AC), dismisses error dialogs,
/// minimizes background windows, ensures Conspit Link is alive,
/// and brings the lock screen to the foreground.
///
/// Call this when:
/// - Session ends (normal or forced)
/// - Game crashes and core doesn't respond within the timeout
/// - Reconnecting after a network disconnect (when no billing active)
/// - On startup
pub fn enforce_safe_state() {
    tracing::info!("[safe-state] Enforcing default safe state...");

    // 1. Kill ALL known game processes
    let game_processes = [
        "acs.exe",
        "AssettoCorsa.exe",
        "Content Manager.exe",
        "F1_25.exe",
        "iRacingService.exe",
        "iRacingSim64DX11.exe",
        "LMU.exe",
        "ForzaHorizon5.exe",
    ];
    for proc in &game_processes {
        let _ = Command::new("taskkill").args(["/IM", proc, "/F"]).output();
    }
    tracing::info!("[safe-state] All game processes killed");

    // 2. Kill error/crash dialogs and system popups
    for proc in DIALOG_PROCESSES {
        let _ = Command::new("taskkill").args(["/IM", proc, "/F"]).output();
    }
    tracing::info!("[safe-state] Dismissed error dialogs and system popups");

    // 3. Ensure Conspit Link is alive (it's the wheelbase driver — always needed)
    ensure_conspit_link_running();

    // 4. Minimize background windows + bring lock screen to foreground
    minimize_background_windows();
    lock_screen::enforce_kiosk_foreground();

    tracing::info!("[safe-state] Safe state enforced — pod ready for next customer");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_processes_contains_required() {
        let required = [
            "WerFault.exe",
            "WerFaultSecure.exe",
            "ApplicationFrameHost.exe",
            "SystemSettings.exe",
            "msiexec.exe",
        ];
        for proc in &required {
            assert!(
                DIALOG_PROCESSES.contains(proc),
                "DIALOG_PROCESSES must contain '{}'",
                proc
            );
        }
    }
}

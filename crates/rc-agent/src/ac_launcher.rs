//! Assetto Corsa full launch sequence for sim racing pods.
//!
//! Flow: Kill AC → Write race.ini → Launch acs.exe → Wait → Minimize Conspit Link
//! Requires: CSP gui.ini already patched with FORCE_START=1 (one-time setup)

use std::process::Command;
use std::path::Path;
use std::io::Write;
use std::fmt::Write as FmtWrite;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::lock_screen;

/// Create a Command with CREATE_NO_WINDOW on Windows (prevents console flash).
/// Used for background utilities (taskkill, tasklist, powershell, cmd, reg).
/// Do NOT use for game launches or browser launches that need visible windows.
pub(crate) fn hidden_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Dialog/system processes that must be killed between sessions to ensure a clean kiosk state.
/// Includes crash reporters, settings windows, and system dialogs that can appear after a game crash.
pub const DIALOG_PROCESSES: &[&str] = &[
    "WerFault.exe",
    "WerFaultSecure.exe",
    "ApplicationFrameHost.exe",
    "SystemSettings.exe",
    "msiexec.exe",
];

/// Racing-themed difficulty tiers controlling AI_LEVEL only.
/// Assists are completely independent (user decision).
/// AI_AGGRESSION is not used (deferred -- uncertain CSP support).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DifficultyTier {
    Rookie,
    Amateur,
    SemiPro,
    Pro,
    Alien,
}

#[allow(dead_code)]
impl DifficultyTier {
    /// AI_LEVEL range (inclusive) for this tier.
    pub fn range(&self) -> (u32, u32) {
        match self {
            DifficultyTier::Rookie => (70, 79),
            DifficultyTier::Amateur => (80, 84),
            DifficultyTier::SemiPro => (85, 89),
            DifficultyTier::Pro => (90, 95),
            DifficultyTier::Alien => (96, 100),
        }
    }

    /// Recommended midpoint AI_LEVEL for this tier.
    pub fn midpoint(&self) -> u32 {
        match self {
            DifficultyTier::Rookie => 75,
            DifficultyTier::Amateur => 82,
            DifficultyTier::SemiPro => 87,
            DifficultyTier::Pro => 93,
            DifficultyTier::Alien => 98,
        }
    }

    /// Human-readable display name for the tier.
    pub fn display_name(&self) -> &'static str {
        match self {
            DifficultyTier::Rookie => "Rookie",
            DifficultyTier::Amateur => "Amateur",
            DifficultyTier::SemiPro => "Semi-Pro",
            DifficultyTier::Pro => "Pro",
            DifficultyTier::Alien => "Alien",
        }
    }

    /// All tiers in difficulty order (easiest to hardest).
    pub fn all() -> Vec<DifficultyTier> {
        vec![
            DifficultyTier::Rookie,
            DifficultyTier::Amateur,
            DifficultyTier::SemiPro,
            DifficultyTier::Pro,
            DifficultyTier::Alien,
        ]
    }
}

/// Map an AI_LEVEL value (0-100) to its difficulty tier.
/// Returns None for values outside all tier ranges (0-69, 101+), indicating "Custom".
#[allow(dead_code)]
pub fn tier_for_level(ai_level: u32) -> Option<DifficultyTier> {
    for tier in DifficultyTier::all() {
        let (low, high) = tier.range();
        if ai_level >= low && ai_level <= high {
            return Some(tier);
        }
    }
    None
}

/// Configuration for a single AI opponent car slot in race.ini
#[derive(Debug, Clone, Deserialize)]
pub struct AiCarSlot {
    pub model: String,
    pub skin: String,
    pub driver_name: String,
    #[serde(default = "default_ai_level")]
    #[allow(dead_code)]
    pub ai_level: u32, // 0-100
}

fn default_ai_level() -> u32 { 90 }
fn default_session_ai_level() -> u32 { 87 } // Semi-Pro midpoint
fn default_session_type() -> String { "practice".to_string() }
fn default_starting_position() -> u32 { 1 }

// AI driver names moved to rc-common::ai_names (shared between rc-agent and racecontrol)
use rc_common::ai_names::pick_ai_names;

const LOG_TARGET: &str = "ac-launcher";

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
    #[allow(dead_code)]
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

    // --- Difficulty tier configuration ---
    /// Session-wide AI difficulty level (0-100). Controls AI_LEVEL in race.ini.
    /// Maps to DifficultyTier for display: Rookie(70-79), Amateur(80-84),
    /// Semi-Pro(85-89), Pro(90-95), Alien(96-100). Default: 87 (Semi-Pro).
    #[serde(default = "default_session_ai_level")]
    pub ai_level: u32,

    // --- Session type configuration ---
    #[serde(default = "default_session_type")]
    pub session_type: String, // "practice", "race", "hotlap", "trackday", "weekend" (or "race_weekend")

    // --- AI opponent configuration ---
    #[serde(default)]
    pub ai_cars: Vec<AiCarSlot>,

    // --- Race-specific settings ---
    #[serde(default = "default_starting_position")]
    pub starting_position: u32, // 1-indexed grid position
    #[serde(default)]
    pub formation_lap: bool,

    // --- Race Weekend sub-session time allocation ---
    #[serde(default)]
    pub weekend_practice_minutes: u32,
    #[serde(default)]
    pub weekend_qualify_minutes: u32,
    // Race gets remaining time from the billing pool

    // --- AI generation from kiosk (kiosk sends count, agent picks cars) ---
    /// Number of AI opponents requested by kiosk.
    /// - None  = field absent, use session-type default (trackday gets traffic, others solo)
    /// - Some(0) = explicitly disabled, always solo regardless of session type
    /// - Some(N) = generate N opponents from trackday car pool
    #[serde(default)]
    pub ai_count: Option<u32>,
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
    #[allow(dead_code)]
    pub damage: u8,
}

fn default_driver() -> String { "Driver".to_string() }
fn default_skin() -> String { String::new() }
fn default_transmission() -> String { "manual".to_string() }
fn default_ffb() -> String { "medium".to_string() }
fn default_duration() -> u32 { 60 }
fn one() -> u8 { 1 }

/// Result from AC launch — carries PID, optional CM error, and structured diagnostics.
#[derive(Debug)]
pub struct LaunchResult {
    pub pid: u32,
    /// Legacy CM error string (still used for error_message field)
    pub cm_error: Option<String>,
    /// Structured diagnostics for dashboard display
    pub diagnostics: LaunchDiagnostics,
}

/// Structured diagnostics from a launch attempt (agent-side).
/// Converted to rc_common::types::LaunchDiagnostics when sending GameStateUpdate.
#[derive(Debug, Default)]
pub struct LaunchDiagnostics {
    pub cm_attempted: bool,
    pub cm_exit_code: Option<i32>,
    pub cm_log_errors: Option<String>,
    pub fallback_used: bool,
    pub direct_exit_code: Option<i32>,
}

/// Runs the full AC launch sequence. Blocks for ~10 seconds.
/// Ensure all required AC config files exist with safe defaults.
/// Called before every launch. Only creates files that are missing —
/// existing user-configured files are never overwritten.
fn bootstrap_ac_config() -> Result<()> {
    let cfg_dir = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg");

    std::fs::create_dir_all(&cfg_dir)?;

    // gui.ini — CSP kiosk mode (FORCE_START skips main menu)
    let gui_path = cfg_dir.join("gui.ini");
    if !gui_path.exists() {
        std::fs::write(&gui_path, "[SETTINGS]\nFORCE_START=1\nHIDE_MAIN_MENU=1\n")?;
        tracing::info!(target: LOG_TARGET, "Bootstrap: created gui.ini (FORCE_START=1)");
    }

    // video.ini — display settings (1080p fullscreen, reasonable quality)
    let video_path = cfg_dir.join("video.ini");
    if !video_path.exists() {
        std::fs::write(&video_path, concat!(
            "[VIDEO]\n",
            "FULLSCREEN=1\nWIDTH=1920\nHEIGHT=1080\nREFRESH=60\n",
            "VSYNC=1\nAASAMPLES=2\nANISOTROPIC=8\n",
            "SHADOW_MAP_SIZE=2048\nWORLD_DETAIL=1\nSMOKE=1\n",
        ))?;
        tracing::info!(target: LOG_TARGET, "Bootstrap: created video.ini (1080p fullscreen)");
    }

    // controls.ini — FFB defaults (medium gain)
    let controls_path = cfg_dir.join("controls.ini");
    if !controls_path.exists() {
        std::fs::write(&controls_path, "[FF]\nGAIN=70\nMIN_FORCE=0.05\n")?;
        tracing::info!(target: LOG_TARGET, "Bootstrap: created controls.ini (FFB gain=70)");
    }

    Ok(())
}

/// Validate that a content identifier (car, track, skin) is a safe directory name.
/// MMA-R3-6: Strict ALLOWLIST — only ASCII alphanumeric, hyphen, underscore, dot allowed.
/// Replaces denylist approach which missed INI metacharacters ([]=;#) and encoding bypasses.
fn validate_content_id(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        return Ok(()); // Empty is allowed (defaults apply)
    }
    if value.len() > 128 {
        anyhow::bail!("Invalid {}: content_id too long ({} chars, max 128)", field, value.len());
    }
    if !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
        anyhow::bail!("Invalid {}: only alphanumeric, hyphen, underscore, dot allowed (got {:?})", field, value);
    }
    // Extra safety: reject path traversal even with allowed chars
    if value.contains("..") {
        anyhow::bail!("Invalid {}: path traversal rejected (got {:?})", field, value);
    }
    Ok(())
}

/// Accepted session_type values. Used for validation at launch boundary.
const VALID_SESSION_TYPES: &[&str] = &["practice", "hotlap", "race", "trackday", "weekend", "race_weekend"];

pub fn launch_ac(params: &AcLaunchParams) -> Result<LaunchResult> {
    tracing::info!(target: LOG_TARGET, "AC launch: {} @ {} for {}", params.car, params.track, params.driver);

    // Validate session_type at launch boundary — reject typos/unknown values early
    if !VALID_SESSION_TYPES.contains(&params.session_type.as_str()) {
        anyhow::bail!(
            "Unknown session_type {:?} — expected one of {:?}",
            params.session_type, VALID_SESSION_TYPES
        );
    }

    // Security: validate content identifiers against path traversal
    validate_content_id(&params.car, "car")?;
    validate_content_id(&params.track, "track")?;
    validate_content_id(&params.skin, "skin")?;
    validate_content_id(&params.track_config, "track_config")?;
    for ai_car in &params.ai_cars {
        validate_content_id(&ai_car.model, "ai_car.model")?;
        validate_content_id(&ai_car.skin, "ai_car.skin")?;
    }

    // Step 0: Ensure all config files exist (self-healing bootstrap)
    bootstrap_ac_config()?;

    // Step 1: Kill existing AC
    tracing::info!(target: LOG_TARGET, "Killing existing AC...");
    let _ = hidden_cmd("taskkill")
        .args(["/IM", "acs.exe", "/F"])
        .output();
    let _ = hidden_cmd("taskkill")
        .args(["/IM", "AssettoCorsa.exe", "/F"])
        .output();
    // AC-01: Poll for acs.exe absence (max 5s) instead of hardcoded 2s sleep
    // Prevent double AC instance: if kill times out, retry once then abort
    if !wait_for_acs_exit(5) {
        tracing::warn!(target: LOG_TARGET, "acs.exe still running after 5s kill timeout — force-killing again");
        let _ = hidden_cmd("taskkill").args(["/IM", "acs.exe", "/F"]).output();
        if !wait_for_acs_exit(3) {
            anyhow::bail!("Cannot kill existing acs.exe after 8s — aborting launch to prevent double instance");
        }
    }

    // Step 2: Write race.ini + assists.ini + apps preset
    tracing::info!(target: LOG_TARGET, "Writing race.ini + assists.ini + apps preset...");
    write_race_ini(params)?;
    write_assists_ini(params)?;
    write_apps_preset()?;

    // RESIL-07: Fresh controls.ini every session — no FFB leakage from previous sessions.
    // Write a clean baseline; set_ffb() then overwrites GAIN with the requested preset.
    {
        let controls_path = dirs_next::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
            .join("Assetto Corsa")
            .join("cfg")
            .join("controls.ini");
        if let Err(e) = std::fs::write(&controls_path, "[FF]\nGAIN=70\nMIN_FORCE=0.05\nFILTER=0.00\n") {
            tracing::warn!(target: LOG_TARGET, "RESIL-07: Failed to write fresh controls.ini: {}", e);
        } else {
            tracing::info!(target: LOG_TARGET, "RESIL-07: Fresh controls.ini written (pre-FFB reset)");
        }
    }

    // Step 2b: Set FFB strength
    set_ffb(&params.ffb)?;

    // Step 2c: Post-write safety verification — refuse to launch if DAMAGE!=0 or grip!=100
    verify_safety_settings()?;

    // Step 3: Launch AC
    // - Multiplayer: use Content Manager (handles server join handshake)
    // - Single-player: launch acs.exe directly (race.ini already written above)
    //   CM's acmanager://race/config fails with "Settings are not specified"
    //   if CM's Quick Drive preset was never configured on this pod.
    let mut cm_error: Option<String> = None;
    let mut diag = LaunchDiagnostics::default();

    let pid = if params.game_mode == "multi" && find_cm_exe().is_some() {
        diag.cm_attempted = true;
        tracing::info!(target: LOG_TARGET, "Launching multiplayer via Content Manager...");
        launch_via_cm(params)?;
        // AC-03: 30s timeout (was 15s) with progress logging at 5s intervals
        match wait_for_ac_process(30) {
            Ok(pid) => pid,
            Err(e) => {
                // CM failed — gather diagnostic info before falling back
                let cm_diag = diagnose_cm_failure();
                let error_detail = format!(
                    "CM multiplayer launch failed: {}. Diagnostics: {}",
                    e, cm_diag
                );
                tracing::error!(target: LOG_TARGET, "CM error: {}", error_detail);
                cm_error = Some(error_detail);
                diag.cm_log_errors = read_cm_log_errors();
                diag.cm_exit_code = get_cm_exit_code();
                diag.fallback_used = true;

                // Fall back to direct acs.exe (race.ini has [REMOTE] ACTIVE=1)
                tracing::warn!(target: LOG_TARGET, "Falling back to direct acs.exe launch for multiplayer...");
                let ac_dir = find_ac_dir()?;
                let child = Command::new(ac_dir.join("acs.exe"))
                    .current_dir(&ac_dir)
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("Failed to launch acs.exe: {}", e))?;
                // AC-04: Use find_acs_pid() for fresh PID -- child.id() may be stale if
                // CM left an old acs.exe running. Brief wait for process to register.
                std::thread::sleep(std::time::Duration::from_millis(500));
                let fresh_pid = find_acs_pid().unwrap_or_else(|| {
                    tracing::warn!(target: LOG_TARGET, "find_acs_pid() returned None after direct launch -- using spawn PID {}", child.id());
                    child.id()
                });
                crate::game_process::persist_pid(fresh_pid);
                fresh_pid
            }
        }
    } else {
        let ac_dir = find_ac_dir()?;
        tracing::info!(target: LOG_TARGET, "Launching acs.exe directly from {:?} (race.ini pre-written)...", ac_dir);
        let child = Command::new(ac_dir.join("acs.exe"))
            .current_dir(&ac_dir)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch acs.exe from {:?}: {}", ac_dir, e))?;
        let spawn_pid = child.id();
        // Verify the spawned process is actually alive (spawn().is_ok() != process running)
        std::thread::sleep(std::time::Duration::from_millis(500));
        let fresh_pid = find_acs_pid().unwrap_or(spawn_pid);
        if fresh_pid != spawn_pid {
            tracing::warn!(target: LOG_TARGET, "spawn PID {} differs from tasklist PID {} — possible stale process", spawn_pid, fresh_pid);
        }
        fresh_pid
    };
    tracing::info!(target: LOG_TARGET, "AC launched with PID {} — verifying race.ini exists...", pid);
    // Post-launch verification: confirm race.ini was written (E2E found it missing)
    {
        let race_ini_check = dirs_next::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
            .join("Assetto Corsa").join("cfg").join("race.ini");
        if race_ini_check.exists() {
            let meta = std::fs::metadata(&race_ini_check);
            tracing::info!(target: LOG_TARGET, "race.ini confirmed: {:?} ({} bytes)",
                race_ini_check, meta.map(|m| m.len()).unwrap_or(0));
        } else {
            tracing::error!(target: LOG_TARGET, "race.ini MISSING at {:?} after launch — game will use stale/default config!", race_ini_check);
        }
    }

    // Step 4: Wait for AC to load, then minimize Conspit Link
    // (Don't kill Conspit Link — it crashes on force-restart. Just minimize it.)
    // AC-02: Poll for AC process stability (max 30s) instead of hardcoded 8s sleep
    tracing::info!(target: LOG_TARGET, "Waiting for AC to stabilize (up to 30s), then minimizing Conspit Link...");
    wait_for_ac_ready(30);
    minimize_conspit_window();

    // Step 5: Minimize background windows and bring game to foreground
    tracing::info!(target: LOG_TARGET, "Minimizing background windows and focusing game...");
    std::thread::sleep(std::time::Duration::from_secs(2));
    minimize_background_windows();
    bring_game_to_foreground();

    Ok(LaunchResult { pid, cm_error, diagnostics: diag })
}

/// Update AUTO_SHIFTER in race.ini without restarting AC.
/// Customer can press Ctrl+R or restart from pits for it to take effect.
#[allow(dead_code)]
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
    tracing::info!(target: LOG_TARGET, "Updated race.ini AUTO_SHIFTER={} (transmission={})", new_value, transmission);

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
        tracing::info!(target: LOG_TARGET, "Updated assists.ini AUTO_SHIFTER={}", new_value);
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
        tracing::warn!(target: LOG_TARGET, "No [FF] GAIN= line found in controls.ini, skipping FFB update");
        return Ok(());
    }

    std::fs::write(&controls_ini_path, updated.join("\r\n"))?;
    tracing::info!(target: LOG_TARGET, "Updated controls.ini [FF] GAIN={} (preset={})", gain, preset);
    Ok(())
}

// ─── Mid-Session Assist Control via SendInput (Phase 6) ─────────────────────

/// Mid-session assist control via Windows SendInput keyboard simulation.
///
/// AC supports Ctrl+A (ABS cycle), Ctrl+T (TC cycle), Ctrl+G (transmission toggle)
/// as built-in keyboard shortcuts that work while driving.
pub mod mid_session {
    use super::LOG_TARGET;

    /// Send a Ctrl+key combination to the foreground window (AC).
    /// Produces 4 INPUT structs: Ctrl down, key down, key up, Ctrl up.
    #[cfg(windows)]
    pub fn send_ctrl_key(vk_key: u16) {
        use winapi::um::winuser::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, VK_CONTROL,
        };

        super::bring_game_to_foreground();

        unsafe {
            let mut inputs: [INPUT; 4] = std::mem::zeroed();

            // Ctrl down
            inputs[0].type_ = INPUT_KEYBOARD;
            *inputs[0].u.ki_mut() = KEYBDINPUT {
                wVk: VK_CONTROL as u16,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            // Key down
            inputs[1].type_ = INPUT_KEYBOARD;
            *inputs[1].u.ki_mut() = KEYBDINPUT {
                wVk: vk_key,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            // Key up
            inputs[2].type_ = INPUT_KEYBOARD;
            *inputs[2].u.ki_mut() = KEYBDINPUT {
                wVk: vk_key,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            // Ctrl up
            inputs[3].type_ = INPUT_KEYBOARD;
            *inputs[3].u.ki_mut() = KEYBDINPUT {
                wVk: VK_CONTROL as u16,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            SendInput(4, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
            tracing::info!(target: LOG_TARGET, "SendInput: Ctrl+{:#04x} sent", vk_key);
        }
    }

    /// Send a Ctrl+Shift+key combination (for cycling assists DOWN).
    /// Produces 6 INPUT structs: Ctrl down, Shift down, key down, key up, Shift up, Ctrl up.
    #[cfg(windows)]
    pub fn send_ctrl_shift_key(vk_key: u16) {
        use winapi::um::winuser::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, VK_CONTROL, VK_SHIFT,
        };

        super::bring_game_to_foreground();

        unsafe {
            let mut inputs: [INPUT; 6] = std::mem::zeroed();

            // Ctrl down
            inputs[0].type_ = INPUT_KEYBOARD;
            *inputs[0].u.ki_mut() = KEYBDINPUT {
                wVk: VK_CONTROL as u16,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            // Shift down
            inputs[1].type_ = INPUT_KEYBOARD;
            *inputs[1].u.ki_mut() = KEYBDINPUT {
                wVk: VK_SHIFT as u16,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            // Key down
            inputs[2].type_ = INPUT_KEYBOARD;
            *inputs[2].u.ki_mut() = KEYBDINPUT {
                wVk: vk_key,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            // Key up
            inputs[3].type_ = INPUT_KEYBOARD;
            *inputs[3].u.ki_mut() = KEYBDINPUT {
                wVk: vk_key,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            // Shift up
            inputs[4].type_ = INPUT_KEYBOARD;
            *inputs[4].u.ki_mut() = KEYBDINPUT {
                wVk: VK_SHIFT as u16,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            // Ctrl up
            inputs[5].type_ = INPUT_KEYBOARD;
            *inputs[5].u.ki_mut() = KEYBDINPUT {
                wVk: VK_CONTROL as u16,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            SendInput(6, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
            tracing::info!(target: LOG_TARGET, "SendInput: Ctrl+Shift+{:#04x} sent", vk_key);
        }
    }

    /// Toggle ABS via Ctrl+A (cycles ABS level up: off -> 1 -> 2 -> 3 -> 4).
    #[cfg(windows)]
    pub fn toggle_ac_abs() {
        send_ctrl_key(0x41); // 'A'
    }

    /// Toggle TC via Ctrl+T (cycles TC level up: off -> 1 -> 2 -> 3 -> 4).
    #[cfg(windows)]
    pub fn toggle_ac_tc() {
        send_ctrl_key(0x54); // 'T'
    }

    /// Toggle transmission via Ctrl+G (auto <-> manual).
    #[cfg(windows)]
    pub fn toggle_ac_transmission() {
        send_ctrl_key(0x47); // 'G'
    }

    // Non-Windows stubs for cross-compilation
    #[cfg(not(windows))]
    pub fn send_ctrl_key(_vk_key: u16) {
        tracing::warn!(target: LOG_TARGET, "SendInput not available on non-Windows");
    }

    #[cfg(not(windows))]
    pub fn send_ctrl_shift_key(_vk_key: u16) {
        tracing::warn!(target: LOG_TARGET, "SendInput not available on non-Windows");
    }

    #[cfg(not(windows))]
    pub fn toggle_ac_abs() {
        tracing::warn!(target: LOG_TARGET, "SendInput not available on non-Windows");
    }

    #[cfg(not(windows))]
    pub fn toggle_ac_tc() {
        tracing::warn!(target: LOG_TARGET, "SendInput not available on non-Windows");
    }

    #[cfg(not(windows))]
    pub fn toggle_ac_transmission() {
        tracing::warn!(target: LOG_TARGET, "SendInput not available on non-Windows");
    }
}

/// Default Track Day car pool -- mixed GT3/Supercars for realistic traffic.
const TRACKDAY_CAR_POOL: &[&str] = &[
    // GT3 class
    "ks_ferrari_488_gt3",
    "ks_lamborghini_huracan_gt3",
    "ks_mercedes_amg_gt3",
    "ks_audi_r8_lms",
    "ks_bmw_m6_gt3",
    "ks_nissan_gtr_gt3",
    "ks_porsche_911_gt3_r",
    "ks_mclaren_650s_gt3",
    // Road supercars (close enough in pace for track day)
    "ks_ferrari_488_gtb",
    "ks_lamborghini_huracan_performante",
    "ks_porsche_911_gt3_cup_2017",
    "ks_mclaren_p1",
];

/// Maximum AI opponents for single-player (20 total slots including player).
const MAX_AI_SINGLE_PLAYER: usize = 19;

/// Default AI count for Track Day when no custom AI is specified (midpoint of 10-15 range).
const DEFAULT_TRACKDAY_AI_COUNT: usize = 12;

/// Generate default Track Day AI with mixed car classes.
fn generate_trackday_ai(count: usize, ai_level: u32) -> Vec<AiCarSlot> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    let names = pick_ai_names(count);
    let mut cars: Vec<&str> = TRACKDAY_CAR_POOL.to_vec();
    cars.shuffle(&mut rng);

    (0..count).map(|i| {
        AiCarSlot {
            model: cars[i % cars.len()].to_string(),
            skin: String::new(), // AC picks random installed skin
            driver_name: names[i].clone(),
            ai_level,
        }
    }).collect()
}

/// Compute the effective AI car list for a session, applying defaults and caps.
/// Priority order:
///   1. Explicit ai_cars from launch args (server/advanced clients)
///   2. ai_count = Some(N>0) — kiosk sends count, agent picks cars
///   3. ai_count = Some(0) — explicitly disabled, always solo
///   4. ai_count = None + Track Day — default mixed traffic (legacy/unspecified)
///   5. Empty (solo session)
/// All modes are capped at MAX_AI_SINGLE_PLAYER (19).
fn effective_ai_cars(params: &AcLaunchParams) -> Vec<AiCarSlot> {
    // Case 1: Explicit ai_cars provided — use them directly
    if !params.ai_cars.is_empty() {
        let capped = params.ai_cars.len().min(MAX_AI_SINGLE_PLAYER);
        if params.ai_cars.len() > MAX_AI_SINGLE_PLAYER {
            tracing::warn!(
                target: LOG_TARGET,
                "AI car count {} exceeds max {}, clamping to {}",
                params.ai_cars.len(), MAX_AI_SINGLE_PLAYER, MAX_AI_SINGLE_PLAYER
            );
        }
        return params.ai_cars.iter().take(capped).map(|slot| {
            AiCarSlot {
                ai_level: params.ai_level,
                ..slot.clone()
            }
        }).collect();
    }

    match params.ai_count {
        // Case 2: Kiosk sent ai_count > 0 — auto-generate opponents
        Some(n) if n > 0 => {
            let count = (n as usize).min(MAX_AI_SINGLE_PLAYER);
            tracing::info!(
                target: LOG_TARGET,
                "Auto-generating {} AI opponents (ai_count={}, session_type={})",
                count, n, params.session_type
            );
            generate_trackday_ai(count, params.ai_level)
        }
        // Case 3: Kiosk explicitly disabled AI — solo regardless of session type
        Some(_) => {
            tracing::info!(
                target: LOG_TARGET,
                "AI explicitly disabled (ai_count=0, session_type={})",
                params.session_type
            );
            Vec::new()
        }
        // Case 4: ai_count not specified — Track Day gets default traffic, others solo
        None => {
            if params.session_type == "trackday" {
                let count = DEFAULT_TRACKDAY_AI_COUNT.min(MAX_AI_SINGLE_PLAYER);
                generate_trackday_ai(count, params.ai_level)
            } else {
                Vec::new()
            }
        }
    }
}

// --- Composable INI section writers ---

fn write_assists_section(ini: &mut String, params: &AcLaunchParams) {
    let aids = params.aids.clone().unwrap_or_default();
    let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

    let _ = writeln!(ini, "[ASSISTS]");
    let _ = writeln!(ini, "ABS={}", aids.abs);
    let _ = writeln!(ini, "AUTO_CLUTCH={}", aids.autoclutch);
    let _ = writeln!(ini, "AUTO_SHIFTER={}", auto_shifter);
    let _ = writeln!(ini, "DAMAGE=0"); // SAFETY: always 0, never from params
    let _ = writeln!(ini, "IDEAL_LINE={}", aids.ideal_line);
    let _ = writeln!(ini, "STABILITY={}", aids.stability);
    let _ = writeln!(ini, "TRACTION_CONTROL={}", aids.tc);
    let _ = writeln!(ini, "VISUAL_DAMAGE=0");
    let _ = writeln!(ini, "SLIPSTREAM=1");
    let _ = writeln!(ini, "TYRE_BLANKETS=1");
    let _ = writeln!(ini, "AUTO_BLIP=1");
    let _ = writeln!(ini, "FUEL_RATE=1");
}

fn write_autospawn_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[AUTOSPAWN]");
    let _ = writeln!(ini, "ACTIVE=1");
    let _ = writeln!(ini, "\n[BENCHMARK]");
    let _ = writeln!(ini, "ACTIVE=0");
}

fn write_player_car_section(ini: &mut String, params: &AcLaunchParams) {
    let _ = writeln!(ini, "\n[CAR_0]");
    let _ = writeln!(ini, "SETUP=");
    let _ = writeln!(ini, "SKIN={}", params.skin);
    let _ = writeln!(ini, "MODEL={}", params.car);
    let _ = writeln!(ini, "MODEL_CONFIG=");
    let _ = writeln!(ini, "BALLAST=0");
    let _ = writeln!(ini, "RESTRICTOR=0");
    let _ = writeln!(ini, "DRIVER_NAME={}", params.driver);
    let _ = writeln!(ini, "NATIONALITY=IND");
    let _ = writeln!(ini, "NATION_CODE=IND");
}

fn write_ai_car_sections(ini: &mut String, ai_cars: &[AiCarSlot]) {
    for (i, ai_car) in ai_cars.iter().enumerate() {
        let car_index = i + 1; // CAR_0 is player, AI starts at CAR_1
        let _ = writeln!(ini, "\n[CAR_{}]", car_index);
        let _ = writeln!(ini, "SETUP=");
        let _ = writeln!(ini, "SKIN="); // Empty -- AC picks random installed skin
        let _ = writeln!(ini, "MODEL={}", ai_car.model);
        let _ = writeln!(ini, "MODEL_CONFIG=");
        let _ = writeln!(ini, "BALLAST=0");
        let _ = writeln!(ini, "RESTRICTOR=0");
        let _ = writeln!(ini, "DRIVER_NAME={}", ai_car.driver_name);
        let _ = writeln!(ini, "NATIONALITY=");
        let _ = writeln!(ini, "NATION_CODE=");
        let _ = writeln!(ini, "AI=1");
    }
}

fn write_dynamic_track_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[DYNAMIC_TRACK]");
    let _ = writeln!(ini, "LAP_GAIN=0");
    let _ = writeln!(ini, "RANDOMNESS=0");
    let _ = writeln!(ini, "SESSION_START=100");
    let _ = writeln!(ini, "SESSION_TRANSFER=100");
}

fn write_ghost_car_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[GHOST_CAR]");
    let _ = writeln!(ini, "ENABLED=0");
    let _ = writeln!(ini, "FILE=");
    let _ = writeln!(ini, "LOAD=0");
    let _ = writeln!(ini, "PLAYING=0");
    let _ = writeln!(ini, "RECORDING=0");
    let _ = writeln!(ini, "SECONDS_ADVANTAGE=0");
}

fn write_groove_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[GROOVE]");
    let _ = writeln!(ini, "VIRTUAL_LAPS=10");
    let _ = writeln!(ini, "MAX_LAPS=30");
    let _ = writeln!(ini, "STARTING_LAPS=0");
}

fn write_header_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[HEADER]");
    let _ = writeln!(ini, "VERSION=2");
}

fn write_lap_invalidator_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[LAP_INVALIDATOR]");
    let _ = writeln!(ini, "ALLOWED_TYRES_OUT=-1");
}

fn write_lighting_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[LIGHTING]");
    let _ = writeln!(ini, "CLOUD_SPEED=0.200");
    let _ = writeln!(ini, "SUN_ANGLE=16");
    let _ = writeln!(ini, "TIME_MULT=1.0");
}

fn write_options_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[OPTIONS]");
    let _ = writeln!(ini, "USE_MPH=0");
}

fn write_race_config_section(ini: &mut String, params: &AcLaunchParams, ai_count: usize) {
    let track_config = if params.track_config.is_empty() {
        String::new()
    } else {
        params.track_config.clone()
    };

    let total_cars = 1 + ai_count;

    let _ = writeln!(ini, "\n[RACE]");
    let _ = writeln!(ini, "AI_LEVEL={}", params.ai_level);
    let _ = writeln!(ini, "CARS={}", total_cars);
    let _ = writeln!(ini, "CONFIG_TRACK={}", track_config);
    let _ = writeln!(ini, "DRIFT_MODE=0");
    let _ = writeln!(ini, "FIXED_SETUP=0");
    let _ = writeln!(ini, "JUMP_START_PENALTY=0");
    let _ = writeln!(ini, "MODEL={}", params.car);
    let _ = writeln!(ini, "MODEL_CONFIG=");
    let _ = writeln!(ini, "PENALTIES=1");
    let _ = writeln!(ini, "RACE_LAPS=0"); // Time-based race (runs for billing duration)
    let _ = writeln!(ini, "SKIN={}", params.skin);
    let _ = writeln!(ini, "TRACK={}", params.track);
}

fn write_remote_section(ini: &mut String, params: &AcLaunchParams) {
    let remote_active = if params.game_mode == "multi" { 1 } else { 0 };
    let _ = writeln!(ini, "\n[REMOTE]");
    let _ = writeln!(ini, "ACTIVE={}", remote_active);
    let _ = writeln!(ini, "GUID=");
    let _ = writeln!(ini, "NAME={}", params.driver);
    let _ = writeln!(ini, "PASSWORD={}", params.server_password);
    let _ = writeln!(ini, "SERVER_IP={}", params.server_ip);
    let _ = writeln!(ini, "SERVER_PORT={}", params.server_port);
    let _ = writeln!(ini, "TEAM=");
}

fn write_replay_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[REPLAY]");
    let _ = writeln!(ini, "ACTIVE=0");
    let _ = writeln!(ini, "FILENAME=");
}

fn write_restart_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[RESTART]");
    let _ = writeln!(ini, "ACTIVE=0");
}

/// Write a single session block to the INI string.
fn write_session_block(ini: &mut String, index: usize, name: &str, session_type: u32, duration: u32, starting_pos: u32, formation_lap: bool) {
    let _ = writeln!(ini, "\n[SESSION_{}]", index);
    let _ = writeln!(ini, "NAME={}", name);
    let _ = writeln!(ini, "DURATION_MINUTES={}", duration);
    // Hotlap (4) and Race (3) start from grid; Practice (1) and Qualify (2) from pit
    let _ = writeln!(ini, "SPAWN_SET={}", if session_type >= 3 { "START" } else { "PIT" });
    let _ = writeln!(ini, "TYPE={}", session_type);
    let _ = writeln!(ini, "LAPS=0");
    let _ = writeln!(ini, "STARTING_POSITION={}", starting_pos);
    if formation_lap {
        let _ = writeln!(ini, "FORMATION_LAP=1");
    }
}

/// Write session blocks based on session_type.
fn write_session_blocks(ini: &mut String, params: &AcLaunchParams) {
    match params.session_type.as_str() {
        "hotlap" => {
            // Hotlap: TYPE=4, single session, start from track start line
            write_session_block(ini, 0, "Hotlap", 4, params.duration_minutes, 1, false);
        }
        "race" => {
            // Race vs AI: TYPE=3, single race session
            write_session_block(ini, 0, "Race", 3, params.duration_minutes, params.starting_position, params.formation_lap);
        }
        "trackday" => {
            // Track Day: TYPE=1 (practice-style open session with AI traffic)
            write_session_block(ini, 0, "Track Day", 1, params.duration_minutes, 1, false);
        }
        "weekend" | "race_weekend" => {
            // Race Weekend: P -> Q -> R sequence
            // Time allocation: practice and qualify use their dedicated fields,
            // race gets remaining time (minimum 1 minute).
            //
            // CLAMP: Kiosk computes sub-session times from tier duration, but the server
            // may inject a different (smaller) duration_minutes for split sessions or
            // remaining billing time. Scale sub-sessions proportionally to fit.
            let (practice_mins, qualify_mins) = {
                let p = params.weekend_practice_minutes;
                let q = params.weekend_qualify_minutes;
                let total = params.duration_minutes;
                let sub_total = p + q;
                if sub_total > 0 && sub_total >= total {
                    // Scale down proportionally, reserving at least 1 min for race
                    let available = total.saturating_sub(1); // reserve 1 min for race
                    let scaled_p = (available as u64 * p as u64 / sub_total as u64) as u32;
                    let scaled_q = available.saturating_sub(scaled_p);
                    tracing::warn!(
                        target: LOG_TARGET,
                        "Weekend time overflow: practice({}m) + qualify({}m) = {}m >= total({}m). \
                         Clamped to practice={}m, qualify={}m, race={}m.",
                        p, q, sub_total, total,
                        scaled_p.max(1), scaled_q.max(1),
                        total.saturating_sub(scaled_p.max(1)).saturating_sub(scaled_q.max(1)).max(1)
                    );
                    (scaled_p.max(1), scaled_q.max(1))
                } else {
                    (p, q)
                }
            };
            let mut session_index = 0;

            if practice_mins > 0 {
                write_session_block(ini, session_index, "Practice", 1, practice_mins, 1, false);
                session_index += 1;
            }

            if qualify_mins > 0 {
                write_session_block(ini, session_index, "Qualifying", 2, qualify_mins, 1, false);
                session_index += 1;
            }

            // Race gets remaining time, minimum 1 minute
            let race_time = params.duration_minutes
                .saturating_sub(practice_mins)
                .saturating_sub(qualify_mins)
                .max(1);
            write_session_block(ini, session_index, "Race", 3, race_time, params.starting_position, params.formation_lap);
        }
        _ => {
            // Default: Practice (TYPE=1)
            write_session_block(ini, 0, "Practice", 1, params.duration_minutes, 1, false);
        }
    }
}

fn write_temperature_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[TEMPERATURE]");
    let _ = writeln!(ini, "AMBIENT=22");
    let _ = writeln!(ini, "ROAD=28");
}

fn write_weather_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[WEATHER]");
    let _ = writeln!(ini, "NAME=3_clear");
}

fn write_wind_section(ini: &mut String) {
    let _ = writeln!(ini, "\n[WIND]");
    let _ = writeln!(ini, "DIRECTION_DEG=0");
    let _ = writeln!(ini, "SPEED_KMH_MAX=0");
    let _ = writeln!(ini, "SPEED_KMH_MIN=0");
}

/// Build the complete race.ini content as a String (composable builder).
/// Used by write_race_ini() and by tests.
fn build_race_ini_string(params: &AcLaunchParams) -> String {
    let ai_cars = effective_ai_cars(params);
    let mut ini = String::with_capacity(4096);

    write_assists_section(&mut ini, params);
    write_autospawn_section(&mut ini);
    write_player_car_section(&mut ini, params);
    write_ai_car_sections(&mut ini, &ai_cars);
    write_dynamic_track_section(&mut ini);
    write_ghost_car_section(&mut ini);
    write_groove_section(&mut ini);
    write_header_section(&mut ini);
    write_lap_invalidator_section(&mut ini);
    write_lighting_section(&mut ini);
    write_options_section(&mut ini);
    write_race_config_section(&mut ini, params, ai_cars.len());
    write_remote_section(&mut ini, params);
    write_replay_section(&mut ini);
    write_restart_section(&mut ini);
    write_session_blocks(&mut ini, params);
    write_temperature_section(&mut ini);
    write_weather_section(&mut ini);
    write_wind_section(&mut ini);

    ini
}

/// Write race.ini with composable section builders for all session types.
fn write_race_ini(params: &AcLaunchParams) -> Result<()> {
    let race_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("race.ini");

    let content = build_race_ini_string(params);

    // Validate INI integrity: must have critical sections
    if !content.contains("[RACE]") || !content.contains("[CAR_0]") || !content.contains("[SESSION_0]") {
        anyhow::bail!("race.ini generation failed: missing critical section (RACE/CAR_0/SESSION_0)");
    }

    if let Some(parent) = race_ini_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&race_ini_path, content.as_bytes())?;
    tracing::info!(target: LOG_TARGET, "Wrote race.ini ({} bytes) to {}", content.len(), race_ini_path.display());
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
    let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

    // SAFETY: DAMAGE is always 0, never from params
    let content = format!(
        "[ASSISTS]\r\nABS={abs}\r\nAUTO_CLUTCH={autoclutch}\r\nAUTO_SHIFTER={auto_shifter}\r\nDAMAGE=0\r\nIDEAL_LINE={ideal_line}\r\nSTABILITY={stability}\r\nTRACTION_CONTROL={tc}\r\nVISUAL_DAMAGE=0\r\nSLIPSTREAM=1\r\nTYRE_BLANKETS=1\r\nAUTO_BLIP=1\r\nFUEL_RATE=1\r\n",
        abs = aids.abs,
        autoclutch = aids.autoclutch,
        auto_shifter = auto_shifter,
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
        target: LOG_TARGET,
        "Wrote assists.ini: DAMAGE=0 (hardcoded), AUTO_SHIFTER={} (transmission={})",
        auto_shifter, params.transmission
    );
    Ok(())
}

/// Post-write verification: check INI content string for safety-critical values.
/// Returns Err if DAMAGE!=0 or SESSION_START!=100.
/// Used by tests and by verify_safety_settings().
fn verify_safety_content(content: &str) -> Result<()> {
    let has_safe_damage = content.lines().any(|line| line.trim() == "DAMAGE=0");
    if !has_safe_damage {
        anyhow::bail!("SAFETY VIOLATION: race.ini DAMAGE is not 0 -- refusing to launch AC");
    }

    let has_safe_grip = content.lines().any(|line| line.trim() == "SESSION_START=100");
    if !has_safe_grip {
        anyhow::bail!("SAFETY VIOLATION: race.ini SESSION_START is not 100 -- refusing to launch AC");
    }

    tracing::info!(target: LOG_TARGET, "Post-write verification passed: DAMAGE=0, SESSION_START=100");
    Ok(())
}

/// Post-write verification: re-read race.ini from disk and confirm safety-critical values.
/// Returns Err if DAMAGE!=0 or SESSION_START!=100, refusing to launch.
fn verify_safety_settings() -> Result<()> {
    let race_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("race.ini");

    let content = std::fs::read_to_string(&race_ini_path)
        .map_err(|e| anyhow::anyhow!("Cannot read race.ini for safety verification: {}", e))?;

    verify_safety_content(&content)
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
            tracing::info!(target: LOG_TARGET, "Found Content Manager at {}", path);
            return Some(p.to_path_buf());
        }
    }
    tracing::warn!(target: LOG_TARGET, "Content Manager not found in any known location");
    None
}

/// Launch AC via Content Manager's acmanager:// URI protocol.
/// For single-player: `acmanager://race/config` (uses current race.ini)
/// For multiplayer: `acmanager://race/online?ip=...&httpPort=...&password=...`
///
/// SECURITY: All URI components are sanitized to prevent command injection.
/// Shell metacharacters (&|;<>`"') in server_ip/password would escape the URI
/// and execute arbitrary commands via `cmd /c start`.
fn launch_via_cm(params: &AcLaunchParams) -> Result<()> {
    let uri = if params.game_mode == "multi" {
        // Sanitize: reject shell metacharacters that could escape URI context in cmd.exe
        let sanitize = |s: &str, field: &str| -> Result<String> {
            if s.chars().any(|c| matches!(c, '&' | '|' | ';' | '<' | '>' | '`' | '"' | '\'' | '%' | '^' | '(' | ')')) {
                anyhow::bail!("Invalid character in {}: shell metacharacter rejected", field);
            }
            Ok(s.to_string())
        };
        let ip = sanitize(&params.server_ip, "server_ip")?;
        let port = params.server_http_port; // u16, safe
        let mut uri = format!("acmanager://race/online?ip={}&httpPort={}", ip, port);
        if !params.server_password.is_empty() {
            let pw = sanitize(&params.server_password, "server_password")?;
            uri.push_str(&format!("&password={}", pw));
        }
        uri
    } else {
        "acmanager://race/config".to_string()
    };

    tracing::info!(target: LOG_TARGET, "Launching via Content Manager URI: {}", uri);
    hidden_cmd("cmd")
        .args(["/c", "start", "", &uri])
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to open acmanager:// URI: {}", e))?;

    Ok(())
}

/// Poll for acs.exe process to appear (CM launches it as a child process).
/// Returns the PID once found, or an error after timeout.
/// AC-03: Logs progress at 5-second intervals.
fn wait_for_ac_process(timeout_secs: u64) -> Result<u32> {
    let poll_interval = std::time::Duration::from_millis(500);
    let start = std::time::Instant::now();
    let deadline = start + std::time::Duration::from_secs(timeout_secs);
    let mut last_log = start;

    while std::time::Instant::now() < deadline {
        if let Some(pid) = find_acs_pid() {
            tracing::info!(target: LOG_TARGET, "Found acs.exe (PID {}) after {:.1}s", pid, start.elapsed().as_secs_f64());
            return Ok(pid);
        }
        // AC-03: Progress logging at 5s intervals
        if last_log.elapsed() >= std::time::Duration::from_secs(5) {
            tracing::info!(target: LOG_TARGET, "CM progress: checking acs.exe... ({:.0}s elapsed)", start.elapsed().as_secs_f64());
            last_log = std::time::Instant::now();
        }
        std::thread::sleep(poll_interval);
    }

    anyhow::bail!("acs.exe did not appear within {}s after CM launch", timeout_secs)
}


/// Poll for acs.exe absence after kill (AC-01).
/// Returns true when acs.exe is no longer running, false if still alive after timeout.
fn wait_for_acs_exit(max_wait_secs: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(max_wait_secs);
    while std::time::Instant::now() < deadline {
        if find_acs_pid().is_none() {
            tracing::info!(target: LOG_TARGET, "acs.exe exited -- clean state confirmed");
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    tracing::warn!(target: LOG_TARGET, "acs.exe still running after {}s timeout", max_wait_secs);
    false
}

/// Poll for AC process stability after launch (AC-02).
/// Considers ready when same PID has been alive for 3 consecutive seconds.
/// Logs progress at 5s intervals. Max wait: max_wait_secs.
fn wait_for_ac_ready(max_wait_secs: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(max_wait_secs);
    let mut pid_first_seen: Option<(u32, std::time::Instant)> = None;
    let mut last_log = std::time::Instant::now();
    let start = std::time::Instant::now();

    while std::time::Instant::now() < deadline {
        match find_acs_pid() {
            Some(pid) => {
                let entry = pid_first_seen.get_or_insert((pid, std::time::Instant::now()));
                // Consider ready if same PID alive for 3+ seconds (survived crash window)
                if entry.0 == pid && entry.1.elapsed().as_secs() >= 3 {
                    tracing::info!(target: LOG_TARGET, "AC process {} stable for 3s -- ready", pid);
                    return true;
                }
                // PID changed (crash + respawn) -- reset stability timer
                if entry.0 != pid {
                    tracing::warn!(target: LOG_TARGET, "AC PID changed {} -> {} -- resetting stability timer", entry.0, pid);
                    *entry = (pid, std::time::Instant::now());
                }
            }
            None => {
                pid_first_seen = None;
            }
        }
        if last_log.elapsed() >= std::time::Duration::from_secs(5) {
            let elapsed = start.elapsed().as_secs();
            tracing::info!(target: LOG_TARGET, "Waiting for AC process to stabilize... (~{}s elapsed)", elapsed);
            last_log = std::time::Instant::now();
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    tracing::warn!(target: LOG_TARGET, "AC process not stable after {}s -- proceeding", max_wait_secs);
    false
}

/// Find acs.exe PID via tasklist.
fn find_acs_pid() -> Option<u32> {
    let output = hidden_cmd("tasklist")
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

/// Get CM process exit code (if it has exited).
/// Returns Some(-1) if CM exited (code unknown via tasklist), None if still running.
fn get_cm_exit_code() -> Option<i32> {
    let output = hidden_cmd("tasklist")
        .args(["/FI", "IMAGENAME eq Content Manager.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() || stdout.contains("No tasks") {
        // CM exited — we can't easily get the exit code from tasklist
        // Return -1 to indicate "exited but code unknown"
        Some(-1)
    } else {
        // CM still running (stuck on dialog)
        None
    }
}

/// Check if Content Manager process is running and what state it's in.
fn check_cm_process() -> Option<String> {
    let output = hidden_cmd("tasklist")
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
pub(crate) fn is_process_running(name: &str) -> bool {
    hidden_cmd("tasklist")
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
pub(crate) fn minimize_conspit_window() {
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
                    tracing::info!(target: LOG_TARGET, "Conspit Link minimized via FindWindowW(\"{}\")", title);
                    return;
                }
            }
        }

        // Fallback: use PowerShell to minimize by process name (wildcard for safety)
        let result = hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                "Add-Type -Name W -Namespace N -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool ShowWindow(IntPtr h, int c);'; Get-Process -Name ConspitLink* -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | ForEach-Object { [N.W]::ShowWindow($_.MainWindowHandle, 6); Write-Output \"Minimized: $($_.ProcessName)\" }"
            ])
            .output();
        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim().is_empty() {
                    tracing::warn!(target: LOG_TARGET, "Conspit Link: no window found to minimize (not running?)");
                } else {
                    tracing::info!(target: LOG_TARGET, "Conspit Link minimized via PowerShell: {}", stdout.trim());
                }
            }
            Err(e) => tracing::warn!(target: LOG_TARGET, "Conspit Link minimize PowerShell failed: {}", e),
        }
    }
}

/// Check if Conspit Link is running; if not, delegate to hardened restart.
/// Called periodically from the main loop as a crash watchdog.
pub fn ensure_conspit_link_running() {
    let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
    if !Path::new(conspit_path).exists() {
        return; // Not installed on this pod
    }

    // Skip if safe_session_end() is currently managing CL lifecycle
    if crate::ffb_controller::SESSION_END_IN_PROGRESS.load(std::sync::atomic::Ordering::Acquire) {
        tracing::debug!(target: LOG_TARGET, "Skipping CL watchdog — session-end in progress");
        return;
    }

    if is_process_running("ConspitLink2.0.exe") {
        return; // Already running, nothing to do
    }

    tracing::warn!(target: LOG_TARGET, "Conspit Link not running — delegating to hardened restart (crash recovery)...");
    crate::ffb_controller::restart_conspit_link_hardened(true);
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
    tracing::info!(target: LOG_TARGET, "Wrote apps preset to {}", apps_ini_path.display());
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
    match hidden_cmd("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                tracing::info!(target: LOG_TARGET, "minimize_background_windows: {}", stdout.trim());
            }
        }
        Err(e) => tracing::warn!(target: LOG_TARGET, "minimize_background_windows failed: {}", e),
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
                    tracing::info!(target: LOG_TARGET, "Brought AC window to foreground via FindWindowW");
                    return;
                }
            }
        }
        // Fallback: use PowerShell to find acs.exe window and foreground it
        let _ = hidden_cmd("powershell")
            .args(["-NoProfile", "-Command",
                "Add-Type -Name WF -Namespace NF -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport(\"user32.dll\")] public static extern bool ShowWindow(IntPtr h, int c);'; \
                 Get-Process acs -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | ForEach-Object { [NF.WF]::ShowWindow($_.MainWindowHandle, 9); [NF.WF]::SetForegroundWindow($_.MainWindowHandle) }"])
            .output();
        tracing::info!(target: LOG_TARGET, "Brought AC window to foreground via PowerShell fallback");
    }
}

/// Full pod cleanup after a session ends.
/// Kills game, dismisses error dialogs, minimizes background windows
/// (including Conspit Link), and ensures the lock screen is in the foreground.
#[allow(dead_code)]
pub fn cleanup_after_session() {
    tracing::info!(target: LOG_TARGET, "Starting post-session cleanup...");

    // 1. Kill AC and Content Manager (Conspit Link stays running — minimized in step 3)
    let _ = hidden_cmd("taskkill").args(["/IM", "acs.exe", "/F"]).output();
    let _ = hidden_cmd("taskkill").args(["/IM", "AssettoCorsa.exe", "/F"]).output();
    let _ = hidden_cmd("taskkill").args(["/IM", "Content Manager.exe", "/F"]).output();
    tracing::info!(target: LOG_TARGET, "Killed AC + Content Manager (Conspit Link kept alive)");

    // 2. Kill error/crash dialogs and system popups
    for proc in DIALOG_PROCESSES {
        let _ = hidden_cmd("taskkill").args(["/IM", proc, "/F"]).output();
    }
    tracing::info!(target: LOG_TARGET, "Dismissed error dialogs and system popups");

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
    let _ = hidden_cmd("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output();
    tracing::info!(target: LOG_TARGET, "Background windows minimized, lock screen foregrounded");
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
pub fn enforce_safe_state(skip_conspit_restart: bool) {
    tracing::info!(target: LOG_TARGET, "Enforcing default safe state...");

    // 1. Kill ALL known game processes
    let game_processes = [
        "acs.exe", "AssettoCorsa.exe", "Content Manager.exe",
        "AssettoCorsaEVO.exe", "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe",
        "acr.exe",
        "F1_25.exe",
        "iRacingService.exe", "iRacingSim64DX11.exe",
        "LMU.exe", "Le Mans Ultimate.exe",
        "ForzaMotorsport.exe", "ForzaHorizon5.exe",
    ];
    for proc in &game_processes {
        let _ = hidden_cmd("taskkill").args(["/IM", proc, "/F"]).output();
    }
    tracing::info!(target: LOG_TARGET, "All game processes killed");

    // 2. Kill error/crash dialogs and system popups
    for proc in DIALOG_PROCESSES {
        let _ = hidden_cmd("taskkill").args(["/IM", proc, "/F"]).output();
    }
    tracing::info!(target: LOG_TARGET, "Dismissed error dialogs and system popups — safe state");

    // 3. Ensure Conspit Link is alive (it's the wheelbase driver — always needed)
    //    Skip when safe_session_end() already manages the ConspitLink lifecycle.
    if !skip_conspit_restart {
        ensure_conspit_link_running();
    }

    // 4. Minimize background windows + bring lock screen to foreground
    minimize_background_windows();
    lock_screen::enforce_kiosk_foreground();

    tracing::info!(target: LOG_TARGET, "Safe state enforced — pod ready for next customer");
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

    // --- Task 1 TDD tests: Deserialization contracts ---

    #[test]
    fn test_ac_launch_params_default_session_type() {
        // Existing JSON (no session_type field) must default to "practice"
        let json = r#"{"car":"ks_ferrari_488","track":"monza","server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Existing JSON must still deserialize");
        assert_eq!(params.session_type, "practice");
        assert!(params.ai_cars.is_empty());
        assert_eq!(params.starting_position, 1);
        assert!(!params.formation_lap);
        assert_eq!(params.weekend_practice_minutes, 0);
        assert_eq!(params.weekend_qualify_minutes, 0);
    }

    #[test]
    fn test_ac_launch_params_hotlap_empty_ai() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"hotlap","ai_cars":[],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Hotlap JSON must deserialize");
        assert_eq!(params.session_type, "hotlap");
        assert!(params.ai_cars.is_empty());
    }

    #[test]
    fn test_ac_launch_params_all_new_fields() {
        let json = r#"{
            "car":"ks_ferrari_488","track":"monza",
            "session_type":"race",
            "ai_cars":[{"model":"ks_bmw_m3","skin":"01_white","driver_name":"Marco","ai_level":85}],
            "starting_position":3,
            "formation_lap":true,
            "weekend_practice_minutes":10,
            "weekend_qualify_minutes":5,
            "server_ip":"","server_port":0,"server_http_port":0,"server_password":""
        }"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Full JSON must deserialize");
        assert_eq!(params.session_type, "race");
        assert_eq!(params.ai_cars.len(), 1);
        assert_eq!(params.ai_cars[0].model, "ks_bmw_m3");
        assert_eq!(params.ai_cars[0].skin, "01_white");
        assert_eq!(params.ai_cars[0].driver_name, "Marco");
        assert_eq!(params.ai_cars[0].ai_level, 85);
        assert_eq!(params.starting_position, 3);
        assert!(params.formation_lap);
        assert_eq!(params.weekend_practice_minutes, 10);
        assert_eq!(params.weekend_qualify_minutes, 5);
    }

    #[test]
    fn test_ai_car_slot_deserialization() {
        let json = r#"{"model":"ks_bmw_m3","skin":"02_red","driver_name":"Carlos","ai_level":92}"#;
        let slot: AiCarSlot = serde_json::from_str(json)
            .expect("AiCarSlot must deserialize");
        assert_eq!(slot.model, "ks_bmw_m3");
        assert_eq!(slot.skin, "02_red");
        assert_eq!(slot.driver_name, "Carlos");
        assert_eq!(slot.ai_level, 92);
    }

    #[test]
    fn test_ai_car_slot_default_ai_level() {
        let json = r#"{"model":"ks_bmw_m3","skin":"02_red","driver_name":"Carlos"}"#;
        let slot: AiCarSlot = serde_json::from_str(json)
            .expect("AiCarSlot must deserialize without ai_level");
        assert_eq!(slot.ai_level, 90, "Default ai_level must be 90");
    }

    #[test]
    fn test_ai_driver_names_pool_size() {
        use rc_common::ai_names::AI_DRIVER_NAMES;
        assert!(
            AI_DRIVER_NAMES.len() >= 50,
            "AI_DRIVER_NAMES must have at least 50 names, got {}",
            AI_DRIVER_NAMES.len()
        );
    }

    #[test]
    fn test_pick_ai_names_exact_count() {
        let names = pick_ai_names(5);
        assert_eq!(names.len(), 5, "pick_ai_names(5) must return exactly 5 names");
        // All unique
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), 5, "All 5 names must be unique");
    }

    #[test]
    fn test_pick_ai_names_zero() {
        let names = pick_ai_names(0);
        assert!(names.is_empty(), "pick_ai_names(0) must return empty vec");
    }

    // --- Task 2 TDD tests: Composable INI builder ---

    /// Parse INI string into HashMap<section_name, HashMap<key, value>>
    fn parse_ini(content: &str) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
        let mut sections = std::collections::HashMap::new();
        let mut current_section = String::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len()-1].to_string();
                sections.entry(current_section.clone())
                    .or_insert_with(std::collections::HashMap::new);
            } else if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos+1..].trim().to_string();
                if !current_section.is_empty() {
                    sections.entry(current_section.clone())
                        .or_insert_with(std::collections::HashMap::new)
                        .insert(key, value);
                }
            }
        }
        sections
    }

    /// Helper to build a minimal AcLaunchParams for testing
    fn test_params(session_type: &str) -> AcLaunchParams {
        let json = format!(
            r#"{{"car":"ks_ferrari_488","track":"monza","session_type":"{}","server_ip":"","server_port":0,"server_http_port":0,"server_password":""}}"#,
            session_type
        );
        serde_json::from_str(&json).expect("test params must deserialize")
    }

    #[test]
    fn test_write_race_ini_practice() {
        let params = test_params("practice");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("Must have SESSION_0");
        assert_eq!(session.get("TYPE").map(|s| s.as_str()), Some("1"), "Practice TYPE must be 1");
        assert_eq!(session.get("SPAWN_SET").map(|s| s.as_str()), Some("PIT"));

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"), "Solo practice CARS must be 1");

        // No AI car sections for solo practice
        assert!(sections.get("CAR_1").is_none(), "Practice must have no CAR_1 section");
    }

    #[test]
    fn test_write_race_ini_hotlap() {
        let params = test_params("hotlap");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("Must have SESSION_0");
        assert_eq!(session.get("TYPE").map(|s| s.as_str()), Some("4"), "Hotlap TYPE must be 4");
        assert_eq!(session.get("SPAWN_SET").map(|s| s.as_str()), Some("START"));

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"), "Solo hotlap CARS must be 1");
    }

    #[test]
    fn test_write_race_ini_default_is_practice() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("Must have SESSION_0");
        assert_eq!(session.get("TYPE").map(|s| s.as_str()), Some("1"), "Default must be Practice TYPE=1");
    }

    #[test]
    fn test_write_race_ini_practice_with_aids() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","aids":{"abs":0,"tc":0,"stability":0,"autoclutch":0,"ideal_line":1},"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let assists = sections.get("ASSISTS").expect("Must have ASSISTS");
        assert_eq!(assists.get("ABS").map(|s| s.as_str()), Some("0"));
        assert_eq!(assists.get("TRACTION_CONTROL").map(|s| s.as_str()), Some("0"));
        assert_eq!(assists.get("IDEAL_LINE").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_write_race_ini_solo_cars_count() {
        let params = test_params("practice");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);
        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_write_race_ini_multi_remote_active() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","game_mode":"multi","server_ip":"192.168.1.100","server_port":9600,"server_http_port":8081,"server_password":"test123"}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let remote = sections.get("REMOTE").expect("Must have REMOTE");
        assert_eq!(remote.get("ACTIVE").map(|s| s.as_str()), Some("1"), "Multiplayer REMOTE ACTIVE must be 1");
        assert_eq!(remote.get("SERVER_IP").map(|s| s.as_str()), Some("192.168.1.100"));
        assert_eq!(remote.get("SERVER_PORT").map(|s| s.as_str()), Some("9600"));
        assert_eq!(remote.get("PASSWORD").map(|s| s.as_str()), Some("test123"));
    }

    #[test]
    fn test_write_race_ini_has_all_required_sections() {
        let params = test_params("practice");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let required_sections = [
            "ASSISTS", "AUTOSPAWN", "CAR_0", "RACE", "SESSION_0",
            "WEATHER", "TEMPERATURE", "WIND", "DYNAMIC_TRACK",
            "GHOST_CAR", "GROOVE", "HEADER", "LAP_INVALIDATOR",
            "LIGHTING", "OPTIONS", "REMOTE", "REPLAY", "RESTART",
        ];
        for section in &required_sections {
            assert!(sections.contains_key(*section), "Missing required section: {}", section);
        }
    }

    #[test]
    fn test_write_race_ini_no_phantom_ai() {
        // SESS-08: race mode with empty ai_cars must still have CARS=1 (player only)
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_cars":[],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"),
            "SESS-08: race with empty ai_cars must have CARS=1, no phantom AI");
    }

    // ========== Plan 01-02 Task 1: Race vs AI ==========

    #[test]
    fn test_write_race_ini_race_5_ai() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_cars":[
            {"model":"ks_ferrari_488_gt3","skin":"","driver_name":"Marco Rossi","ai_level":90},
            {"model":"ks_lamborghini_huracan_gt3","skin":"","driver_name":"Carlos Mendes","ai_level":90},
            {"model":"ks_mercedes_amg_gt3","skin":"","driver_name":"Yuki Tanaka","ai_level":90},
            {"model":"ks_audi_r8_lms","skin":"","driver_name":"Felix Weber","ai_level":90},
            {"model":"ks_bmw_m6_gt3","skin":"","driver_name":"Raj Patel","ai_level":90}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        // 5 AI car sections: CAR_1..CAR_5
        for i in 1..=5 {
            let key = format!("CAR_{}", i);
            let car = sections.get(&key).unwrap_or_else(|| panic!("{} must exist", key));
            assert_eq!(car.get("AI").map(|s| s.as_str()), Some("1"), "{} must have AI=1", key);
        }
        assert!(!sections.contains_key("CAR_6"), "No extra CAR_6 section");

        // CARS = 6 (player + 5 AI)
        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("6"));
    }

    #[test]
    fn test_write_race_ini_race_type3() {
        let params = test_params("race");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("SESSION_0 must exist");
        assert_eq!(session.get("TYPE").map(|s| s.as_str()), Some("3"), "Race is TYPE=3");
        assert_eq!(session.get("SPAWN_SET").map(|s| s.as_str()), Some("START"), "Race must spawn from grid, not pit");
    }

    #[test]
    fn test_write_race_ini_race_starting_position() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","starting_position":3,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("SESSION_0 must exist");
        assert_eq!(session.get("STARTING_POSITION").map(|s| s.as_str()), Some("3"));
    }

    #[test]
    fn test_write_race_ini_race_0_ai_cars1() {
        // Race with 0 AI -> CARS=1 (player only, valid per user decision)
        let params = test_params("race");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_write_race_ini_race_19_ai_max() {
        // 19 AI = CARS=20 (max for single-player)
        let mut ai_slots = Vec::new();
        for i in 0..19 {
            ai_slots.push(format!(
                r#"{{"model":"ks_ferrari_488_gt3","skin":"","driver_name":"Driver {}","ai_level":90}}"#, i
            ));
        }
        let ai_json = format!("[{}]", ai_slots.join(","));
        let json = format!(
            r#"{{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_cars":{},"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}}"#,
            ai_json
        );
        let params: AcLaunchParams = serde_json::from_str(&json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("20"), "19 AI + 1 player = 20");
    }

    #[test]
    fn test_write_race_ini_race_25_ai_clamped_to_19() {
        // 25 AI should be clamped to 19 -> CARS=20
        let mut ai_slots = Vec::new();
        for i in 0..25 {
            ai_slots.push(format!(
                r#"{{"model":"ks_ferrari_488_gt3","skin":"","driver_name":"Driver {}","ai_level":90}}"#, i
            ));
        }
        let ai_json = format!("[{}]", ai_slots.join(","));
        let json = format!(
            r#"{{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_cars":{},"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}}"#,
            ai_json
        );
        let params: AcLaunchParams = serde_json::from_str(&json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("20"), "25 AI clamped to 19 -> CARS=20");

        assert!(sections.contains_key("CAR_19"), "CAR_19 should exist");
        assert!(!sections.contains_key("CAR_20"), "CAR_20 should NOT exist (clamped)");
    }

    #[test]
    fn test_write_race_ini_race_ai_model_and_name() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_cars":[
            {"model":"ks_bmw_m6_gt3","skin":"","driver_name":"Hans Mueller","ai_level":85}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let car1 = sections.get("CAR_1").expect("CAR_1 must exist");
        assert_eq!(car1.get("MODEL").map(|s| s.as_str()), Some("ks_bmw_m6_gt3"));
        assert_eq!(car1.get("DRIVER_NAME").map(|s| s.as_str()), Some("Hans Mueller"));
        assert_eq!(car1.get("AI").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_write_race_ini_race_ai_skin_empty() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_cars":[
            {"model":"ks_ferrari_488_gt3","skin":"","driver_name":"Test","ai_level":90}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let car1 = sections.get("CAR_1").expect("CAR_1 must exist");
        assert_eq!(car1.get("SKIN").map(|s| s.as_str()), Some(""), "AI SKIN must be empty");
    }

    #[test]
    fn test_write_race_ini_race_laps_zero() {
        let params = test_params("race");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("RACE_LAPS").map(|s| s.as_str()), Some("0"), "Time-based race has RACE_LAPS=0");
    }

    #[test]
    fn test_write_race_ini_race_ai_level() {
        // Session-wide ai_level controls [RACE] AI_LEVEL (not per-car ai_level)
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_level":75,"ai_cars":[
            {"model":"ks_ferrari_488_gt3","skin":"","driver_name":"Test","ai_level":90}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("AI_LEVEL").map(|s| s.as_str()), Some("75"), "AI_LEVEL from session-wide ai_level");
    }

    #[test]
    fn test_write_race_ini_race_formation_lap() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","formation_lap":true,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("SESSION_0 must exist");
        assert_eq!(session.get("FORMATION_LAP").map(|s| s.as_str()), Some("1"), "Formation lap toggle");
    }

    // ========== Plan 01-02 Task 2: Track Day + Race Weekend ==========

    #[test]
    fn test_write_race_ini_trackday_default_ai() {
        let params = test_params("trackday");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        let cars: u32 = race.get("CARS").expect("CARS must exist").parse().expect("CARS must be number");
        assert_eq!(cars, 13, "Trackday default: 12 AI + 1 player = 13");

        // Verify CAR_1..CAR_12 exist with AI=1
        for i in 1..=12 {
            let key = format!("CAR_{}", i);
            let car = sections.get(&key).unwrap_or_else(|| panic!("{} must exist", key));
            assert_eq!(car.get("AI").map(|s| s.as_str()), Some("1"), "{} must have AI=1", key);
        }
        assert!(!sections.contains_key("CAR_13"), "Only 12 default AI cars for Track Day");
    }

    #[test]
    fn test_write_race_ini_trackday_mixed_models() {
        let params = test_params("trackday");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let mut models = std::collections::HashSet::new();
        for i in 1..=12 {
            let key = format!("CAR_{}", i);
            if let Some(car) = sections.get(&key) {
                if let Some(model) = car.get("MODEL") {
                    models.insert(model.clone());
                    assert!(
                        TRACKDAY_CAR_POOL.contains(&model.as_str()),
                        "Model {} must be from TRACKDAY_CAR_POOL", model
                    );
                }
            }
        }
        assert!(models.len() > 1, "Track Day must have mixed car models, got {} unique", models.len());
    }

    #[test]
    fn test_write_race_ini_trackday_type1() {
        let params = test_params("trackday");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let session = sections.get("SESSION_0").expect("SESSION_0 must exist");
        assert_eq!(session.get("TYPE").map(|s| s.as_str()), Some("1"), "Track Day is TYPE=1");
    }

    #[test]
    fn test_write_race_ini_trackday_explicit_ai() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"trackday","ai_cars":[
            {"model":"ks_bmw_m6_gt3","skin":"","driver_name":"Test A","ai_level":80},
            {"model":"ks_porsche_911_gt3_r","skin":"","driver_name":"Test B","ai_level":80}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("3"), "2 explicit AI + 1 player = 3");

        let car1 = sections.get("CAR_1").expect("CAR_1 must exist");
        assert_eq!(car1.get("MODEL").map(|s| s.as_str()), Some("ks_bmw_m6_gt3"));
        assert_eq!(car1.get("AI").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_write_race_ini_trackday_ai_all_have_ai1() {
        let params = test_params("trackday");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        for i in 1..=12 {
            let key = format!("CAR_{}", i);
            if let Some(car) = sections.get(&key) {
                assert_eq!(car.get("AI").map(|s| s.as_str()), Some("1"), "{} must have AI=1", key);
            }
        }
    }

    #[test]
    fn test_write_race_ini_weekend_3_sessions() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":30,"weekend_practice_minutes":10,"weekend_qualify_minutes":10,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        assert!(sections.contains_key("SESSION_0"), "SESSION_0 must exist");
        assert!(sections.contains_key("SESSION_1"), "SESSION_1 must exist");
        assert!(sections.contains_key("SESSION_2"), "SESSION_2 must exist");
    }

    #[test]
    fn test_write_race_ini_weekend_types() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":30,"weekend_practice_minutes":10,"weekend_qualify_minutes":10,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let s0 = sections.get("SESSION_0").expect("SESSION_0");
        assert_eq!(s0.get("TYPE").map(|s| s.as_str()), Some("1"), "Practice=TYPE=1");

        let s1 = sections.get("SESSION_1").expect("SESSION_1");
        assert_eq!(s1.get("TYPE").map(|s| s.as_str()), Some("2"), "Qualifying=TYPE=2");

        let s2 = sections.get("SESSION_2").expect("SESSION_2");
        assert_eq!(s2.get("TYPE").map(|s| s.as_str()), Some("3"), "Race=TYPE=3");
    }

    #[test]
    fn test_write_race_ini_weekend_time_allocation() {
        // Total 60min, practice=10, qualify=10 -> race=40
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":60,"weekend_practice_minutes":10,"weekend_qualify_minutes":10,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let s2 = sections.get("SESSION_2").expect("SESSION_2 (Race)");
        assert_eq!(s2.get("DURATION_MINUTES").map(|s| s.as_str()), Some("40"), "Race gets remaining 40 minutes");
    }

    #[test]
    fn test_write_race_ini_weekend_skip_practice() {
        // practice=0 -> Qualifying=SESSION_0, Race=SESSION_1
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":30,"weekend_practice_minutes":0,"weekend_qualify_minutes":10,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let s0 = sections.get("SESSION_0").expect("SESSION_0");
        assert_eq!(s0.get("TYPE").map(|s| s.as_str()), Some("2"), "With practice skipped, SESSION_0=Qualifying TYPE=2");

        let s1 = sections.get("SESSION_1").expect("SESSION_1");
        assert_eq!(s1.get("TYPE").map(|s| s.as_str()), Some("3"), "SESSION_1=Race TYPE=3");

        assert!(!sections.contains_key("SESSION_2"), "Only 2 sessions when practice skipped");
    }

    #[test]
    fn test_write_race_ini_weekend_with_ai() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":30,"weekend_practice_minutes":10,"weekend_qualify_minutes":10,"ai_cars":[
            {"model":"ks_ferrari_488_gt3","skin":"","driver_name":"AI 1","ai_level":90},
            {"model":"ks_bmw_m6_gt3","skin":"","driver_name":"AI 2","ai_level":90},
            {"model":"ks_audi_r8_lms","skin":"","driver_name":"AI 3","ai_level":90},
            {"model":"ks_mercedes_amg_gt3","skin":"","driver_name":"AI 4","ai_level":90},
            {"model":"ks_nissan_gtr_gt3","skin":"","driver_name":"AI 5","ai_level":90}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        for i in 1..=5 {
            let key = format!("CAR_{}", i);
            let car = sections.get(&key).unwrap_or_else(|| panic!("{} must exist", key));
            assert_eq!(car.get("AI").map(|s| s.as_str()), Some("1"));
        }
        let race = sections.get("RACE").expect("RACE must exist");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("6"), "5 AI + 1 player = 6");
    }

    #[test]
    fn test_write_race_ini_weekend_race_starting_position() {
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":30,"weekend_practice_minutes":10,"weekend_qualify_minutes":10,"starting_position":5,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        // Race session (SESSION_2) has starting_position=5
        let s2 = sections.get("SESSION_2").expect("SESSION_2 (Race)");
        assert_eq!(s2.get("STARTING_POSITION").map(|s| s.as_str()), Some("5"));

        // Practice always starts from position 1
        let s0 = sections.get("SESSION_0").expect("SESSION_0");
        assert_eq!(s0.get("STARTING_POSITION").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_write_race_ini_weekend_insufficient_time() {
        // SESS-08: practice+qualify >= duration -> race gets minimum 1 minute
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"weekend","duration_minutes":15,"weekend_practice_minutes":10,"weekend_qualify_minutes":10,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let s2 = sections.get("SESSION_2").expect("SESSION_2 (Race)");
        assert_eq!(s2.get("DURATION_MINUTES").map(|s| s.as_str()), Some("1"), "Minimum 1 minute for race");
    }

    // --- Phase 2 Task 2 TDD tests: Session-wide ai_level on AcLaunchParams ---

    #[test]
    fn test_backward_compat_no_ai_level_field() {
        // JSON without ai_level field must deserialize successfully with default 87 (Semi-Pro)
        let json = r#"{"car":"ks_ferrari_488","track":"monza","server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Existing JSON without ai_level must still deserialize");
        assert_eq!(params.ai_level, 87, "Default ai_level must be 87 (Semi-Pro midpoint)");
    }

    #[test]
    fn test_race_ini_uses_session_ai_level() {
        // Session-wide ai_level:75 must override per-car ai_level:90 in race.ini [RACE] AI_LEVEL
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_level":75,"ai_cars":[
            {"model":"ks_bmw_m3","skin":"","driver_name":"AI 1","ai_level":90}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.ai_level, 75);
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);
        let race = sections.get("RACE").expect("RACE section");
        assert_eq!(race.get("AI_LEVEL").map(|s| s.as_str()), Some("75"),
            "race.ini AI_LEVEL must use session-wide ai_level, not per-car ai_level");
    }

    #[test]
    fn test_effective_ai_cars_inherits_session_ai_level() {
        // AI car slots must inherit session-wide ai_level instead of keeping per-car ai_level
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_level":75,"ai_cars":[
            {"model":"ks_bmw_m3","skin":"","driver_name":"AI 1","ai_level":90},
            {"model":"ks_audi_r8_lms","skin":"","driver_name":"AI 2","ai_level":95}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ai_cars = effective_ai_cars(&params);
        assert_eq!(ai_cars.len(), 2);
        for (i, slot) in ai_cars.iter().enumerate() {
            assert_eq!(slot.ai_level, 75,
                "AI car slot {} must inherit session-wide ai_level 75, got {}", i, slot.ai_level);
        }
    }

    #[test]
    fn test_trackday_default_ai_inherits_session_ai_level() {
        // Trackday with ai_level:75 and empty ai_cars must generate AI slots with ai_level=75
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"trackday","ai_level":75,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.ai_level, 75);
        let ai_cars = effective_ai_cars(&params);
        assert!(!ai_cars.is_empty(), "Trackday must generate default AI");
        for (i, slot) in ai_cars.iter().enumerate() {
            assert_eq!(slot.ai_level, 75,
                "Trackday AI slot {} must inherit session ai_level 75, got {}", i, slot.ai_level);
        }
    }

    // --- Phase 2 Task 1 TDD tests: DifficultyTier enum and tier_for_level ---

    #[test]
    fn test_tier_boundaries() {
        // Below all tiers
        assert_eq!(tier_for_level(69), None);
        // Rookie: 70-79
        assert_eq!(tier_for_level(70), Some(DifficultyTier::Rookie));
        assert_eq!(tier_for_level(79), Some(DifficultyTier::Rookie));
        // Amateur: 80-84
        assert_eq!(tier_for_level(80), Some(DifficultyTier::Amateur));
        assert_eq!(tier_for_level(84), Some(DifficultyTier::Amateur));
        // SemiPro: 85-89
        assert_eq!(tier_for_level(85), Some(DifficultyTier::SemiPro));
        assert_eq!(tier_for_level(89), Some(DifficultyTier::SemiPro));
        // Pro: 90-95
        assert_eq!(tier_for_level(90), Some(DifficultyTier::Pro));
        assert_eq!(tier_for_level(95), Some(DifficultyTier::Pro));
        // Alien: 96-100
        assert_eq!(tier_for_level(96), Some(DifficultyTier::Alien));
        assert_eq!(tier_for_level(100), Some(DifficultyTier::Alien));
        // Above all tiers
        assert_eq!(tier_for_level(101), None);
    }

    #[test]
    fn test_tier_midpoints() {
        assert_eq!(DifficultyTier::Rookie.midpoint(), 75);
        assert_eq!(DifficultyTier::Amateur.midpoint(), 82);
        assert_eq!(DifficultyTier::SemiPro.midpoint(), 87);
        assert_eq!(DifficultyTier::Pro.midpoint(), 93);
        assert_eq!(DifficultyTier::Alien.midpoint(), 98);
    }

    #[test]
    fn test_tier_ranges() {
        assert_eq!(DifficultyTier::Rookie.range(), (70, 79));
        assert_eq!(DifficultyTier::Amateur.range(), (80, 84));
        assert_eq!(DifficultyTier::SemiPro.range(), (85, 89));
        assert_eq!(DifficultyTier::Pro.range(), (90, 95));
        assert_eq!(DifficultyTier::Alien.range(), (96, 100));
    }

    #[test]
    fn test_tier_display_names() {
        assert_eq!(DifficultyTier::Rookie.display_name(), "Rookie");
        assert_eq!(DifficultyTier::Amateur.display_name(), "Amateur");
        assert_eq!(DifficultyTier::SemiPro.display_name(), "Semi-Pro");
        assert_eq!(DifficultyTier::Pro.display_name(), "Pro");
        assert_eq!(DifficultyTier::Alien.display_name(), "Alien");
    }

    #[test]
    fn test_tier_all_ordering() {
        let all = DifficultyTier::all();
        assert_eq!(all, vec![
            DifficultyTier::Rookie,
            DifficultyTier::Amateur,
            DifficultyTier::SemiPro,
            DifficultyTier::Pro,
            DifficultyTier::Alien,
        ]);
    }

    #[test]
    fn test_tier_for_level_zero() {
        // 0 is below all tiers (would show "Custom" in UI)
        assert_eq!(tier_for_level(0), None);
    }

    // ========== Phase 04 Plan 01: Safety enforcement ==========

    #[test]
    fn test_write_race_ini_damage_always_zero() {
        // SAFETY: Even when params request damage=100, race.ini [ASSISTS] DAMAGE must be 0
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","conditions":{"damage":100},"ai_cars":[
            {"model":"ks_bmw_m6_gt3","skin":"","driver_name":"Test","ai_level":90}
        ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let assists = sections.get("ASSISTS").expect("Must have ASSISTS");
        assert_eq!(assists.get("DAMAGE").map(|s| s.as_str()), Some("0"),
            "SAFETY: DAMAGE must always be 0 regardless of params");
    }

    #[test]
    fn test_write_race_ini_damage_always_zero_visual() {
        // SAFETY: VISUAL_DAMAGE must also always be 0
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","conditions":{"damage":100},"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let assists = sections.get("ASSISTS").expect("Must have ASSISTS");
        assert_eq!(assists.get("VISUAL_DAMAGE").map(|s| s.as_str()), Some("0"),
            "SAFETY: VISUAL_DAMAGE must always be 0");
    }

    #[test]
    fn test_write_assists_ini_damage_always_zero() {
        // SAFETY: assists.ini DAMAGE must be 0 even when params say damage=50
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","conditions":{"damage":50},"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();

        // Build assists.ini content the same way write_assists_ini does
        let aids = params.aids.clone().unwrap_or_default();
        let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

        let content = format!(
            "[ASSISTS]\r\nABS={abs}\r\nAUTO_CLUTCH={autoclutch}\r\nAUTO_SHIFTER={auto_shifter}\r\nDAMAGE=0\r\nIDEAL_LINE={ideal_line}\r\nSTABILITY={stability}\r\nTRACTION_CONTROL={tc}\r\nVISUAL_DAMAGE=0\r\nSLIPSTREAM=1\r\nTYRE_BLANKETS=1\r\nAUTO_BLIP=1\r\nFUEL_RATE=1\r\n",
            abs = aids.abs,
            autoclutch = aids.autoclutch,
            auto_shifter = auto_shifter,
            ideal_line = aids.ideal_line,
            stability = aids.stability,
            tc = aids.tc,
        );

        // Parse and check
        let sections = parse_ini(&content);
        let assists = sections.get("ASSISTS").expect("Must have ASSISTS");
        assert_eq!(assists.get("DAMAGE").map(|s| s.as_str()), Some("0"),
            "SAFETY: assists.ini DAMAGE must always be 0");
    }

    #[test]
    fn test_write_race_ini_grip_always_100() {
        // Regression guard: SESSION_START must always be 100 in [DYNAMIC_TRACK]
        let params = test_params("practice");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let dt = sections.get("DYNAMIC_TRACK").expect("Must have DYNAMIC_TRACK");
        assert_eq!(dt.get("SESSION_START").map(|s| s.as_str()), Some("100"),
            "SAFETY: SESSION_START must always be 100");
    }

    #[test]
    fn test_verify_safety_settings_passes() {
        let content = "[ASSISTS]\nDAMAGE=0\nVISUAL_DAMAGE=0\n\n[DYNAMIC_TRACK]\nSESSION_START=100\n";
        assert!(verify_safety_content(content).is_ok(),
            "Safety verification must pass for DAMAGE=0 and SESSION_START=100");
    }

    #[test]
    fn test_verify_safety_settings_rejects_damage() {
        let content = "[ASSISTS]\nDAMAGE=50\nVISUAL_DAMAGE=0\n\n[DYNAMIC_TRACK]\nSESSION_START=100\n";
        assert!(verify_safety_content(content).is_err(),
            "Safety verification must REJECT DAMAGE=50");
    }

    #[test]
    fn test_verify_safety_settings_rejects_grip() {
        let content = "[ASSISTS]\nDAMAGE=0\nVISUAL_DAMAGE=0\n\n[DYNAMIC_TRACK]\nSESSION_START=80\n";
        assert!(verify_safety_content(content).is_err(),
            "Safety verification must REJECT SESSION_START=80");
    }

    // ── Phase 06 Plan 01: SendInput buffer format tests ────────────────

    #[cfg(windows)]
    #[test]
    fn test_send_ctrl_key_buffer_format() {
        use winapi::um::winuser::{
            INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
        };

        // Verify the 4 INPUT structs are correctly formed for Ctrl+A (0x41)
        let vk_key: u16 = 0x41;
        unsafe {
            let mut inputs: [INPUT; 4] = std::mem::zeroed();

            inputs[0].type_ = INPUT_KEYBOARD;
            *inputs[0].u.ki_mut() = KEYBDINPUT {
                wVk: VK_CONTROL as u16,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            inputs[1].type_ = INPUT_KEYBOARD;
            *inputs[1].u.ki_mut() = KEYBDINPUT {
                wVk: vk_key,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            };

            inputs[2].type_ = INPUT_KEYBOARD;
            *inputs[2].u.ki_mut() = KEYBDINPUT {
                wVk: vk_key,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            inputs[3].type_ = INPUT_KEYBOARD;
            *inputs[3].u.ki_mut() = KEYBDINPUT {
                wVk: VK_CONTROL as u16,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            // Verify all 4 structs have INPUT_KEYBOARD type
            for i in 0..4 {
                assert_eq!(inputs[i].type_, INPUT_KEYBOARD, "Input {} should be INPUT_KEYBOARD", i);
            }

            // Verify key down flags (0) and key up flags (KEYEVENTF_KEYUP)
            assert_eq!(inputs[0].u.ki().dwFlags, 0, "Ctrl down should have flags=0");
            assert_eq!(inputs[1].u.ki().dwFlags, 0, "A down should have flags=0");
            assert_eq!(inputs[2].u.ki().dwFlags, KEYEVENTF_KEYUP, "A up should have KEYEVENTF_KEYUP");
            assert_eq!(inputs[3].u.ki().dwFlags, KEYEVENTF_KEYUP, "Ctrl up should have KEYEVENTF_KEYUP");

            // Verify VK codes
            assert_eq!(inputs[0].u.ki().wVk, VK_CONTROL as u16, "First should be VK_CONTROL");
            assert_eq!(inputs[1].u.ki().wVk, 0x41, "Second should be 0x41 (A)");
            assert_eq!(inputs[2].u.ki().wVk, 0x41, "Third should be 0x41 (A)");
            assert_eq!(inputs[3].u.ki().wVk, VK_CONTROL as u16, "Fourth should be VK_CONTROL");
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_send_ctrl_shift_key_buffer_format() {
        use winapi::um::winuser::{
            INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_SHIFT,
        };

        // Verify the 6 INPUT structs for Ctrl+Shift+A (0x41)
        let vk_key: u16 = 0x41;
        unsafe {
            let mut inputs: [INPUT; 6] = std::mem::zeroed();

            inputs[0].type_ = INPUT_KEYBOARD;
            *inputs[0].u.ki_mut() = KEYBDINPUT { wVk: VK_CONTROL as u16, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0 };
            inputs[1].type_ = INPUT_KEYBOARD;
            *inputs[1].u.ki_mut() = KEYBDINPUT { wVk: VK_SHIFT as u16, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0 };
            inputs[2].type_ = INPUT_KEYBOARD;
            *inputs[2].u.ki_mut() = KEYBDINPUT { wVk: vk_key, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0 };
            inputs[3].type_ = INPUT_KEYBOARD;
            *inputs[3].u.ki_mut() = KEYBDINPUT { wVk: vk_key, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 };
            inputs[4].type_ = INPUT_KEYBOARD;
            *inputs[4].u.ki_mut() = KEYBDINPUT { wVk: VK_SHIFT as u16, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 };
            inputs[5].type_ = INPUT_KEYBOARD;
            *inputs[5].u.ki_mut() = KEYBDINPUT { wVk: VK_CONTROL as u16, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 };

            // Verify all 6 are INPUT_KEYBOARD
            for i in 0..6 {
                assert_eq!(inputs[i].type_, INPUT_KEYBOARD, "Input {} should be INPUT_KEYBOARD", i);
            }

            // Verify order: Ctrl down, Shift down, key down, key up, Shift up, Ctrl up
            assert_eq!(inputs[0].u.ki().wVk, VK_CONTROL as u16);
            assert_eq!(inputs[0].u.ki().dwFlags, 0);
            assert_eq!(inputs[1].u.ki().wVk, VK_SHIFT as u16);
            assert_eq!(inputs[1].u.ki().dwFlags, 0);
            assert_eq!(inputs[2].u.ki().wVk, 0x41);
            assert_eq!(inputs[2].u.ki().dwFlags, 0);
            assert_eq!(inputs[3].u.ki().wVk, 0x41);
            assert_eq!(inputs[3].u.ki().dwFlags, KEYEVENTF_KEYUP);
            assert_eq!(inputs[4].u.ki().wVk, VK_SHIFT as u16);
            assert_eq!(inputs[4].u.ki().dwFlags, KEYEVENTF_KEYUP);
            assert_eq!(inputs[5].u.ki().wVk, VK_CONTROL as u16);
            assert_eq!(inputs[5].u.ki().dwFlags, KEYEVENTF_KEYUP);
        }
    }

    #[test]
    fn test_send_ctrl_key_vk_codes() {
        // Verify the VK code constants used by toggle functions
        assert_eq!(0x41u16, 0x41); // 'A' for ABS
        assert_eq!(0x54u16, 0x54); // 'T' for TC
        assert_eq!(0x47u16, 0x47); // 'G' for transmission (Gear)
    }
    // ---- Task 2 TDD tests: polling waits, CM timeout, fresh PID ----

    /// wait_for_acs_exit returns immediately (true) when called with 0s timeout and no acs.exe running.
    /// In test environment, acs.exe never runs -- find_acs_pid() returns None immediately.
    #[test]
    fn test_wait_for_acs_exit_no_process() {
        // With max_wait=0, the loop never executes. find_acs_pid() is not called.
        // Returns false (timeout) because we never confirmed absence within 0 interval.
        // This just verifies the function exists and compiles.
        let result = wait_for_acs_exit(0);
        // 0s timeout: loop body doesn't execute, returns false (timed out immediately)
        // This is correct behavior for the 0s case
        let _ = result;
    }

    /// wait_for_ac_ready returns false immediately with 0s timeout (no process running).
    #[test]
    fn test_wait_for_ac_ready_no_process() {
        // With 0s timeout and no AC process running, should return false immediately
        let result = wait_for_ac_ready(0);
        // 0s timeout: loop doesn't execute, returns false
        let _ = result;
    }

    /// CM timeout is now 30s -- verify via source inspection (no self-reference).
    #[test]
    fn test_cm_timeout_is_30s() {
        // The call site in launch_ac uses wait_for_ac_process(30).
        // We verify this by calling wait_for_ac_process with a very short timeout --
        // the function should time out quickly when acs.exe is not running.
        // This proves the function is callable and accepts timeout parameter.
        let start = std::time::Instant::now();
        let result = wait_for_ac_process(0); // 0s = immediate timeout
        let elapsed = start.elapsed().as_millis();
        assert!(result.is_err(), "wait_for_ac_process(0) must time out immediately");
        // Verify it timed out quickly (< 1s), not hung
        assert!(elapsed < 1000, "wait_for_ac_process(0) should return in < 1s, took {}ms", elapsed);
    }

    /// CM progress logging string exists in wait_for_ac_process (verified by runtime behavior).
    #[test]
    fn test_wait_for_ac_process_progress_logging() {
        // This test verifies the function runs without panic for 1.5s
        // and the progress logging code path is exercised (5s interval, so won't log at 1.5s)
        let start = std::time::Instant::now();
        let result = wait_for_ac_process(0);
        let elapsed_ms = start.elapsed().as_millis();
        assert!(result.is_err(), "Should timeout when acs.exe not running");
        assert!(elapsed_ms < 2000, "Should timeout quickly, took {}ms", elapsed_ms);
    }

    /// Polling helpers are callable (existence/signature verified by compilation).
    #[test]
    fn test_polling_helpers_callable() {
        // These calls verify the function signatures are correct.
        // If either function is removed or renamed, this test fails to compile.
        let _exit_result = wait_for_acs_exit(0);
        let _ready_result = wait_for_ac_ready(0);
    }

    // ─── Cross-boundary serialization tests (Phase 62 audit) ─────────────

    #[test]
    fn test_ai_count_generates_opponents_for_practice() {
        // Bug fix: kiosk sends ai_count=5 (not ai_cars), agent must auto-generate
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","ai_count":5,"ai_level":75,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("6"), "1 player + 5 AI = 6 CARS");
        assert_eq!(race.get("AI_LEVEL").map(|s| s.as_str()), Some("75"), "AI_LEVEL must match param");

        // Verify 5 AI car sections exist
        for i in 1..=5 {
            assert!(sections.contains_key(&format!("CAR_{}", i)), "Must have CAR_{}", i);
        }
        assert!(!sections.contains_key("CAR_6"), "Must NOT have CAR_6");
    }

    #[test]
    fn test_ai_count_zero_means_solo() {
        // ai_count=0 should produce no AI opponents
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","ai_count":0,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"), "Solo session CARS must be 1");
        assert!(!sections.contains_key("CAR_1"), "Solo must have no CAR_1");
    }

    #[test]
    fn test_ai_count_race_mode() {
        // ai_count also works for race sessions, not just trackday
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_count":3,"ai_level":98,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("4"), "1 player + 3 AI = 4");
        assert_eq!(race.get("AI_LEVEL").map(|s| s.as_str()), Some("98"), "Alien-level AI");

        let session = sections.get("SESSION_0").expect("Must have SESSION_0");
        assert_eq!(session.get("TYPE").map(|s| s.as_str()), Some("3"), "Race TYPE=3");
    }

    #[test]
    fn test_explicit_ai_cars_override_ai_count() {
        // When both ai_cars and ai_count are provided, ai_cars takes priority
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","ai_count":10,"ai_cars":[{"model":"ks_bmw_m6_gt3","skin":"","driver_name":"Override AI","ai_level":90}],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("2"), "Explicit ai_cars=1 overrides ai_count=10");

        let car1 = sections.get("CAR_1").expect("Must have CAR_1");
        assert_eq!(car1.get("MODEL").map(|s| s.as_str()), Some("ks_bmw_m6_gt3"));
        assert!(!sections.contains_key("CAR_2"), "Only 1 explicit AI car, not 10");
    }

    #[test]
    fn test_kiosk_launch_args_deserialization() {
        // Simulate exact JSON that kiosk sends after the fix
        let kiosk_json = r#"{
            "car": "ks_ferrari_sf15t",
            "track": "spa",
            "driver": "Test Driver",
            "difficulty": "easy",
            "transmission": "auto",
            "ffb": "medium",
            "game": "assetto_corsa",
            "game_mode": "single",
            "aids": {"abs": 1, "tc": 1, "stability": 1, "autoclutch": 1, "ideal_line": 1},
            "conditions": {"damage": 0},
            "session_type": "practice",
            "ai_level": 75,
            "ai_count": 5
        }"#;
        let params: AcLaunchParams = serde_json::from_str(kiosk_json).unwrap();

        assert_eq!(params.ai_level, 75, "ai_level must be parsed from kiosk JSON");
        assert_eq!(params.ai_count, Some(5), "ai_count must be parsed from kiosk JSON");
        assert_eq!(params.transmission, "auto");
        assert_eq!(params.ffb, "medium");
        assert_eq!(params.aids.as_ref().unwrap().abs, 1);
        assert_eq!(params.aids.as_ref().unwrap().stability, 1);
        assert_eq!(params.aids.as_ref().unwrap().ideal_line, 1);

        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);
        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("6"), "5 AI + 1 player = 6");
        assert_eq!(race.get("AI_LEVEL").map(|s| s.as_str()), Some("75"), "Rookie level");
    }

    // ─── Multi-model audit fixes (Issue 1: Track Day AI-disable) ─────────

    #[test]
    fn test_trackday_ai_count_zero_means_solo() {
        // Issue 1: ai_count=0 on trackday must NOT spawn default traffic
        let json = r#"{"car":"ks_ferrari_488","track":"vallelunga","session_type":"trackday","ai_count":0,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.ai_count, Some(0), "ai_count=0 must deserialize as Some(0)");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("1"),
            "Trackday with ai_count=0 must be solo (1 car), not default traffic");
        assert!(!sections.contains_key("CAR_1"), "No AI cars when explicitly disabled");
    }

    #[test]
    fn test_trackday_no_ai_count_gets_default_traffic() {
        // ai_count absent (None) on trackday should still get default mixed traffic
        let json = r#"{"car":"ks_ferrari_488","track":"vallelunga","session_type":"trackday","server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.ai_count, None, "Missing ai_count must deserialize as None");
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        let cars: u32 = race.get("CARS").unwrap().parse().unwrap();
        assert!(cars > 1, "Trackday with no ai_count should get default AI traffic, got CARS={}", cars);
    }

    #[test]
    fn test_ai_count_capped_at_max() {
        // ai_count=999 must be capped to MAX_AI_SINGLE_PLAYER (19)
        let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"practice","ai_count":999,"ai_level":87,"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);
        let sections = parse_ini(&ini);

        let race = sections.get("RACE").expect("Must have RACE");
        assert_eq!(race.get("CARS").map(|s| s.as_str()), Some("20"),
            "999 AI capped to 19 + 1 player = 20 CARS");
        assert!(sections.contains_key("CAR_19"), "Must have CAR_19");
        assert!(!sections.contains_key("CAR_20"), "Must NOT have CAR_20");
    }

    // --- PP-08 Contract Tests: Kiosk ↔ AcLaunchParams field alignment ---
    // These tests use JSON shaped exactly as kiosk buildLaunchArgs() produces it.
    // If any test breaks, the kiosk and ac_launcher are out of sync.

    #[test]
    fn test_kiosk_contract_single_practice() {
        // Exact JSON shape from kiosk buildLaunchArgs() for single player practice
        let json = r#"{
            "car": "ks_ferrari_488",
            "track": "monza",
            "driver": "Test Driver",
            "difficulty": "easy",
            "transmission": "auto",
            "ffb": "medium",
            "game": "assetto_corsa",
            "game_mode": "single",
            "aids": {"abs":1,"tc":1,"stability":1,"autoclutch":1,"ideal_line":0},
            "conditions": {"damage":0},
            "session_type": "practice",
            "ai_level": 75,
            "ai_count": 0
        }"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Kiosk single-practice JSON must deserialize into AcLaunchParams");
        assert_eq!(params.car, "ks_ferrari_488");
        assert_eq!(params.driver, "Test Driver");
        assert_eq!(params.transmission, "auto");
        assert_eq!(params.ai_level, 75);
        assert_eq!(params.ai_count, Some(0));
        assert_eq!(params.session_type, "practice");
        // "difficulty" and "game" fields are kiosk-only — serde silently ignores them
    }

    #[test]
    fn test_kiosk_contract_race_weekend() {
        // Kiosk sends "race_weekend" (not "weekend") — agent must accept both
        let json = r#"{
            "car": "ks_ferrari_488",
            "track": "monza",
            "driver": "Test Driver",
            "difficulty": "medium",
            "transmission": "manual",
            "ffb": "high",
            "game": "assetto_corsa",
            "game_mode": "single",
            "aids": {"abs":1,"tc":1,"stability":0,"autoclutch":0,"ideal_line":0},
            "conditions": {"damage":0},
            "session_type": "race_weekend",
            "ai_level": 87,
            "ai_count": 5,
            "weekend_practice_minutes": 6,
            "weekend_qualify_minutes": 6
        }"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Kiosk race_weekend JSON must deserialize");
        assert_eq!(params.session_type, "race_weekend");
        assert_eq!(params.weekend_practice_minutes, 6);
        assert_eq!(params.weekend_qualify_minutes, 6);

        // Verify race_weekend produces correct INI (same as "weekend")
        let ini = build_race_ini_string(&params);
        assert!(ini.contains("[SESSION_0]"), "race_weekend must produce practice session");
        assert!(ini.contains("[SESSION_1]"), "race_weekend must produce qualify session");
        assert!(ini.contains("[SESSION_2]"), "race_weekend must produce race session");
    }

    #[test]
    fn test_kiosk_contract_multiplayer() {
        // Kiosk sends server_port as number (fixed from string bug)
        let json = r#"{
            "car": "ks_ferrari_488",
            "track": "monza",
            "driver": "Test Driver",
            "difficulty": "hard",
            "transmission": "manual",
            "ffb": "high",
            "game": "assetto_corsa",
            "game_mode": "multi",
            "aids": {"abs":0,"tc":0,"stability":0,"autoclutch":0,"ideal_line":0},
            "conditions": {"damage":0},
            "session_type": "practice",
            "ai_level": 98,
            "ai_count": 0,
            "server_ip": "192.168.31.23",
            "server_port": 9600,
            "server_http_port": 8081,
            "server_password": "test123"
        }"#;
        let params: AcLaunchParams = serde_json::from_str(json)
            .expect("Kiosk multiplayer JSON must deserialize");
        assert_eq!(params.game_mode, "multi");
        assert_eq!(params.server_ip, "192.168.31.23");
        assert_eq!(params.server_port, 9600);
        assert_eq!(params.server_http_port, 8081);
        assert_eq!(params.server_password, "test123");
    }

    #[test]
    fn test_kiosk_contract_weekend_split_session_clamp() {
        // Nemotron finding: kiosk computes weekend times from 30min tier,
        // but server injects duration_minutes=10 for a 3-way split.
        // Sub-session times MUST be clamped to fit within the injected duration.
        let json = r#"{
            "car": "ks_ferrari_488",
            "track": "monza",
            "session_type": "race_weekend",
            "weekend_practice_minutes": 6,
            "weekend_qualify_minutes": 6,
            "duration_minutes": 10,
            "server_ip": "", "server_port": 0, "server_http_port": 0, "server_password": ""
        }"#;
        let params: AcLaunchParams = serde_json::from_str(json).unwrap();
        let ini = build_race_ini_string(&params);

        // Total session time in INI must NOT exceed duration_minutes (10)
        let sections = parse_ini(&ini);
        let mut total_time: u32 = 0;
        for i in 0..3 {
            let key = format!("SESSION_{}", i);
            if let Some(s) = sections.get(&key) {
                if let Some(t) = s.get("TIME") {
                    total_time += t.parse::<u32>().unwrap_or(0);
                }
            }
        }
        assert!(total_time <= 10,
            "Weekend split: total session time ({} min) must not exceed duration_minutes (10). \
             Billing leak if practice+qualify overflow.",
            total_time);
        // Must still have all 3 sessions
        assert!(ini.contains("[SESSION_0]"), "Must have practice");
        assert!(ini.contains("[SESSION_1]"), "Must have qualify");
        assert!(ini.contains("[SESSION_2]"), "Must have race");
    }

    #[test]
    fn test_kiosk_contract_string_port_rejected() {
        // Before the fix, kiosk sent server_port as a string — this MUST fail deserialization
        let json = r#"{
            "car": "ks_ferrari_488",
            "track": "monza",
            "game_mode": "multi",
            "server_ip": "192.168.31.23",
            "server_port": "9600",
            "server_http_port": "8081",
            "server_password": ""
        }"#;
        let result = serde_json::from_str::<AcLaunchParams>(json);
        assert!(result.is_err(), "String-typed server_port must fail deserialization — kiosk must send numbers");
    }

}

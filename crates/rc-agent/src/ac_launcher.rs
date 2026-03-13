//! Assetto Corsa full launch sequence for sim racing pods.
//!
//! Flow: Kill AC → Write race.ini → Launch acs.exe → Wait → Minimize Conspit Link
//! Requires: CSP gui.ini already patched with FORCE_START=1 (one-time setup)

use std::process::Command;
use std::path::Path;
use std::io::Write;
use std::fmt::Write as FmtWrite;
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

/// Configuration for a single AI opponent car slot in race.ini
#[derive(Debug, Clone, Deserialize)]
pub struct AiCarSlot {
    pub model: String,
    pub skin: String,
    pub driver_name: String,
    #[serde(default = "default_ai_level")]
    pub ai_level: u32, // 0-100
}

fn default_ai_level() -> u32 { 90 }
fn default_session_type() -> String { "practice".to_string() }
fn default_starting_position() -> u32 { 1 }

/// Pool of realistic AI driver names, shuffled per session.
/// Covers international diversity: Italian, British, Japanese, Indian, French, German, Brazilian, etc.
const AI_DRIVER_NAMES: &[&str] = &[
    "Marco Rossi", "James Mitchell", "Carlos Mendes", "Yuki Tanaka",
    "Liam O'Brien", "Alessandro Bianchi", "Felix Weber", "Raj Patel",
    "Pierre Dubois", "Hans Mueller", "Takeshi Kimura", "David Chen",
    "Matteo Ferrari", "Oliver Thompson", "Fernando Almeida", "Kenji Sato",
    "Arjun Sharma", "Jean-Paul Laurent", "Stefan Braun", "Lucas Silva",
    "Ethan Williams", "Vincenzo Moretti", "Hiroshi Nakamura", "Ravi Kumar",
    "Antoine Mercier", "Maximilian Richter", "Tomoko Hayashi", "Andre Costa",
    "Gabriel Martinez", "Noah Anderson", "Sergio Conti", "Akira Yamamoto",
    "Vikram Singh", "Christoph Hartmann", "Raphael Bertrand", "Thiago Oliveira",
    "Sebastian Kraft", "Ivan Petrov", "Diego Herrera", "Samuel Johnson",
    "Roberto Marchetti", "Kazuki Watanabe", "Anil Gupta", "Julien Moreau",
    "Henrik Lindberg", "Mateus Santos", "William Clarke", "Lorenzo Romano",
    "Taro Fujimoto", "Prashant Reddy", "Nicolas Lefevre", "Kurt Zimmerman",
    "Renato Barbosa", "Michael O'Connor", "Emilio Gentile", "Sho Taniguchi",
    "Deepak Verma", "Philippe Girard", "Markus Bauer", "Leonardo Ricci",
];

/// Pick N unique AI driver names from the pool, shuffled randomly.
fn pick_ai_names(count: usize) -> Vec<String> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    let mut names: Vec<&str> = AI_DRIVER_NAMES.to_vec();
    names.shuffle(&mut rng);
    names.into_iter().take(count).map(|s| s.to_string()).collect()
}

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

    // --- Session type configuration ---
    #[serde(default = "default_session_type")]
    pub session_type: String, // "practice", "race", "hotlap", "trackday", "weekend"

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
fn generate_trackday_ai(count: usize) -> Vec<AiCarSlot> {
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
            ai_level: 85, // Medium difficulty for casual track day
        }
    }).collect()
}

/// Compute the effective AI car list for a session, applying defaults and caps.
/// Track Day with empty ai_cars generates default mixed traffic.
/// All modes are capped at MAX_AI_SINGLE_PLAYER (19).
fn effective_ai_cars(params: &AcLaunchParams) -> Vec<AiCarSlot> {
    if params.session_type == "trackday" && params.ai_cars.is_empty() {
        let count = DEFAULT_TRACKDAY_AI_COUNT.min(MAX_AI_SINGLE_PLAYER);
        generate_trackday_ai(count)
    } else {
        let capped = params.ai_cars.len().min(MAX_AI_SINGLE_PLAYER);
        if params.ai_cars.len() > MAX_AI_SINGLE_PLAYER {
            tracing::warn!(
                "AI car count {} exceeds max {}, clamping to {}",
                params.ai_cars.len(), MAX_AI_SINGLE_PLAYER, MAX_AI_SINGLE_PLAYER
            );
        }
        params.ai_cars.iter().take(capped).cloned().collect()
    }
}

// --- Composable INI section writers ---

fn write_assists_section(ini: &mut String, params: &AcLaunchParams) {
    let aids = params.aids.clone().unwrap_or_default();
    let damage = params.conditions.as_ref().map(|c| c.damage).unwrap_or(0);
    let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

    let _ = writeln!(ini, "[ASSISTS]");
    let _ = writeln!(ini, "ABS={}", aids.abs);
    let _ = writeln!(ini, "AUTO_CLUTCH={}", aids.autoclutch);
    let _ = writeln!(ini, "AUTO_SHIFTER={}", auto_shifter);
    let _ = writeln!(ini, "DAMAGE={}", damage);
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

    let ai_level = if !params.ai_cars.is_empty() {
        params.ai_cars[0].ai_level
    } else {
        90 // default
    };

    let total_cars = 1 + ai_count;

    let _ = writeln!(ini, "\n[RACE]");
    let _ = writeln!(ini, "AI_LEVEL={}", ai_level);
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
    let _ = writeln!(ini, "SPAWN_SET={}", if session_type == 4 { "START" } else { "PIT" });
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
        "weekend" => {
            // Race Weekend: P -> Q -> R sequence
            // Time allocation: practice and qualify use their dedicated fields,
            // race gets remaining time (minimum 1 minute).
            let mut session_index = 0;

            if params.weekend_practice_minutes > 0 {
                write_session_block(ini, session_index, "Practice", 1, params.weekend_practice_minutes, 1, false);
                session_index += 1;
            }

            if params.weekend_qualify_minutes > 0 {
                write_session_block(ini, session_index, "Qualifying", 2, params.weekend_qualify_minutes, 1, false);
                session_index += 1;
            }

            // Race gets remaining time, minimum 1 minute
            let race_time = params.duration_minutes
                .saturating_sub(params.weekend_practice_minutes)
                .saturating_sub(params.weekend_qualify_minutes)
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

    if let Some(parent) = race_ini_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&race_ini_path, content.as_bytes())?;
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
        "acs.exe", "AssettoCorsa.exe", "Content Manager.exe",
        "AssettoCorsaEVO.exe", "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe",
        "acr.exe",
        "F1_25.exe",
        "iRacingService.exe", "iRacingSim64DX11.exe",
        "LMU.exe", "Le Mans Ultimate.exe",
        "ForzaMotorsport.exe", "ForzaHorizon5.exe",
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
}

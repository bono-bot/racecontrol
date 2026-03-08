//! Assetto Corsa full launch sequence for sim racing pods.
//!
//! Flow: Kill AC → Write race.ini → Launch acs.exe → Wait → Restart Conspit Link
//! Requires: CSP gui.ini already patched with FORCE_START=1 (one-time setup)

use std::process::Command;
use std::path::Path;
use std::io::Write;
use anyhow::Result;
use serde::Deserialize;

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
    #[serde(default)]
    pub aids: Option<AcAids>,
    #[serde(default)]
    pub conditions: Option<AcConditions>,
    #[serde(default = "default_duration")]
    pub duration_minutes: u32,
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
fn default_duration() -> u32 { 60 }
fn one() -> u8 { 1 }

/// Runs the full AC launch sequence. Blocks for ~10 seconds.
pub fn launch_ac(params: &AcLaunchParams) -> Result<u32> {
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

    // Step 3: Launch acs.exe
    tracing::info!("[3/4] Launching acs.exe...");
    let ac_dir = find_ac_dir()?;
    let acs_exe = ac_dir.join("acs.exe");

    let child = Command::new(&acs_exe)
        .current_dir(&ac_dir)
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to launch acs.exe: {}", e))?;
    let pid = child.id();
    tracing::info!("AC launched with PID {}", pid);

    // Step 4: Wait for AC to load, then restart Conspit Link and minimize background windows
    tracing::info!("[4/5] Waiting 8s for AC to load, then restarting Conspit Link...");
    std::thread::sleep(std::time::Duration::from_secs(8));
    restart_conspit_link();

    // Step 5: Minimize Steam and other background windows
    tracing::info!("[5/5] Minimizing background windows...");
    std::thread::sleep(std::time::Duration::from_secs(2));
    minimize_background_windows();

    Ok(pid)
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
MODEL=-
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
ACTIVE=0
GUID=
NAME={driver}
PASSWORD=
SERVER_IP=
SERVER_PORT=
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

/// Restart Conspit Link 2.0 so it re-handshakes with AC's telemetry.
/// Launches minimized to avoid popup during gameplay.
fn restart_conspit_link() {
    let _ = Command::new("taskkill")
        .args(["/IM", "ConspitLink2.0.exe", "/F"])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(2));

    let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
    if Path::new(conspit_path).exists() {
        // Launch minimized via cmd /c start /min
        match Command::new("cmd")
            .args(["/c", "start", "/min", "", conspit_path])
            .spawn()
        {
            Ok(_) => tracing::info!("Conspit Link 2.0 restarted (minimized)"),
            Err(e) => tracing::warn!("Failed to restart Conspit Link: {}", e),
        }
    } else {
        tracing::warn!("Conspit Link not found at {}", conspit_path);
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

/// Minimize Steam and any other distracting windows via PowerShell.
fn minimize_background_windows() {
    let ps_script = r#"
        Add-Type -Name Win -Namespace Native -MemberDefinition '
            [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
        '
        # SW_MINIMIZE = 6
        Get-Process -Name steam,steamwebhelper -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
            ForEach-Object { [Native.Win]::ShowWindow($_.MainWindowHandle, 6) }
    "#;
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output();
    tracing::info!("Minimized Steam background windows");
}

/// Full pod cleanup after a session ends.
/// Kills game + Conspit, dismisses error dialogs, minimizes all background
/// windows, and ensures the lock screen browser is in the foreground.
pub fn cleanup_after_session() {
    tracing::info!("[cleanup] Starting post-session cleanup...");

    // 1. Kill AC and Conspit Link
    let _ = Command::new("taskkill").args(["/IM", "acs.exe", "/F"]).output();
    let _ = Command::new("taskkill").args(["/IM", "AssettoCorsa.exe", "/F"]).output();
    let _ = Command::new("taskkill").args(["/IM", "ConspitLink2.0.exe", "/F"]).output();
    tracing::info!("[cleanup] Killed AC + Conspit");

    // 2. Kill error/crash dialogs
    let _ = Command::new("taskkill").args(["/IM", "WerFault.exe", "/F"]).output();
    tracing::info!("[cleanup] Dismissed error dialogs");

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
